//! The job service's events an item's orderer and the owner are told of (docs/agent-control.md, 「An item's
//! change is told to its orderer」): read as a page past the cursor agency keeps, each event's notice owed in
//! agency's outbox — a close that handed its commit over and waits on its deployment, standing until it no longer
//! waits, an item closed by somebody other than its orderer, a run's question asked, a question past its due
//! time, and a settling reply that withdraws the question's instruction. The cursor advances only past events
//! whose notices are owed. A pass with no cursor, or one whose cursor left the service's window, reads to the end
//! and tells nothing of what it passed.

use std::collections::BTreeMap;

use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::job_notices;
use crate::outbox::{self, Keep, Notice, Receiver};
use crate::queue::orderer::{JobService, Orderer};
use crate::queue::settings;

/// The settings row the number of the last event read stands in.
pub const CURSOR_KEY: &str = "job_events_after";
/// The prefix of the key a run's question is owed under, its number after it.
pub const QUESTION_OWED: &str = "question-awaits-answer:";
/// The prefix of the key a commit waiting to go live is owed under, its item's number after it.
pub const DEPLOY_OWED: &str = "item-awaits-deploy:";
/// The kinds read.
pub const KINDS: &str = "item.changed,question.created,question.due,reply.created";

// A page of events past `after`, and whether more stand; none where the cursor left the service's window.
async fn page(service: &JobService, after: Option<i64>) -> Result<Option<(Vec<Value>, bool)>, String> {
  let mut query = vec![("kinds", KINDS.to_owned()), ("limit", "500".to_owned())];

  // A cursor of nothing reads from the oldest event the window holds.
  if let Some(after) = after.filter(|after| *after > 0) {
    query.push(("after", after.to_string()));
  }

  let answer = reqwest::Client::new()
    .get(format!("{}/events", service.url))
    .header(reqwest::header::ACCEPT, "application/json")
    .query(&query)
    .bearer_auth(&service.token)
    .send()
    .await
    .map_err(|why| why.to_string())?;

  if answer.status() == reqwest::StatusCode::GONE {
    return Ok(None);
  }

  if !answer.status().is_success() {
    return Err(format!("the job service answered {} to /events", answer.status().as_u16()));
  }

  let page: Value = answer.json().await.map_err(|why| why.to_string())?;

  Ok(Some((page["items"].as_array().cloned().unwrap_or_default(), page["more"] == true)))
}

// The cursor kept, so neither a restart nor the next pass reads an event twice.
async fn keep(db: &DatabaseConnection, at: i64) -> Result<(), String> {
  settings::set(db, CURSOR_KEY, &at.to_string()).await.map_err(|why| why.detail)
}

/// One pass over the job service's events past the cursor, each one's notice owed; nothing where the deployment
/// names no job service or no receiver. An event whose notice could not be owed ends the pass there, the cursor
/// kept at the event before it, so the next pass takes it again.
pub async fn pass(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let (Some(service), Some(_)) = (JobService::of(vars), Receiver::of(vars)) else {
    return Ok(());
  };
  let kept = settings::get(db, CURSOR_KEY)
    .await
    .map_err(|why| why.detail)?
    .and_then(|value| value.trim().parse::<i64>().ok());
  let mut telling = kept.is_some();
  let mut after = kept;

  loop {
    let Some((events, more)) = page(&service, after).await? else {
      if after.is_none_or(|after| after <= 0) {
        return Err("the job service answered 410 to a read from its oldest event".to_owned());
      }

      // The cursor left the window: read from the oldest again, telling nothing of what is passed.
      telling = false;
      after = None;
      continue;
    };

    for event in &events {
      let Some(id) = event["id"].as_i64() else { continue };

      if telling && let Err(why) = told(db, vars, &service, event).await {
        if let Some(last) = after {
          keep(db, last).await?;
        }

        return Err(format!("event {id} was not owed, so the cursor stays before it: {why}"));
      }

      after = Some(id);
    }

    if let Some(last) = after {
      keep(db, last).await?;
    }

    if !more {
      break;
    }
  }

  if after.is_none() {
    // No event stands yet: the cursor starts at nothing, so the next pass tells what comes.
    keep(db, 0).await?;
  }

  Ok(())
}

/// The commits owed as waiting to go live let go where no job of agency's on their item is still starting or
/// running, so its close no longer waits on the deployment: what the sender's clock runs before it sends.
pub async fn deploys_owed(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let owed = outbox::owed(db, DEPLOY_OWED).await?;

  if owed.is_empty() {
    return Ok(());
  }

  let waiting: std::collections::BTreeSet<String> = store::all(
    db,
    r#"SELECT DISTINCT "item_id" FROM "jobs" WHERE "state" IN ('starting', 'running')"#,
    vec![],
  )
  .await
  .map_err(|why| why.detail)?
  .iter()
  .filter_map(|row| row.try_get::<i64>("", "item_id").ok())
  .map(|item| item.to_string())
  .collect();

  for (item, row) in owed.iter().filter(|(item, _)| !waiting.contains(*item)) {
    outbox::let_go_as_read(db, vars, &format!("{DEPLOY_OWED}{item}"), row).await?;
    println!("queue: item #{item}'s commit owed as waiting to go live let go, its job no longer waiting");
  }

  Ok(())
}

// Whether the item's own orderer closed it: its changer is the requester of the item the orderer was found on,
// or a supervisor agency holds.
async fn by_orderer(db: &DatabaseConnection, by: &str, orderer: &Orderer) -> bool {
  orderer.requester.as_deref() == Some(by)
    || crate::supervisors::named(db, by).await.ok().flatten().is_some()
}

// A close's commit waiting to go live owed to the item's orderer, standing, once for that commit; and let go once
// the item no longer reads so.
async fn deploy_told(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  service: &JobService,
  data: &Value,
) -> Result<(), String> {
  let Some(id) = data["id"].as_i64() else { return Ok(()) };
  let key = format!("{DEPLOY_OWED}{id}");
  let owed = outbox::owed(db, &key).await?.remove("");
  // A close that merged its commit leaves the item waiting for its repository's batch deployment; an older
  // close leaves it waiting to go live. Either is owed to the orderer, who puts the repository live.
  let phase = &data["labels"]["close-phase"];
  let waiting = ((phase == "waiting-batch" || phase == "waiting-live") && data["state"] != "done")
    .then(|| data["labels"]["close-commit"].as_str().filter(|commit| !commit.is_empty()))
    .flatten();

  match (waiting, owed) {
    (Some(commit), owed) if owed.as_ref().is_none_or(|owed| owed.mark != commit) => {
      let orderer = service.orderer_of(data).await?;
      let notice = Notice::instruction(job_notices::awaits_deploy(data, commit, &orderer), Keep::Standing);

      outbox::owe(db, vars, notice.marked(commit)).await?;
      println!("queue: item #{id}'s commit {commit} waits to go live, owed to its orderer");
    }
    (None, Some(owed)) => outbox::let_go_as_read(db, vars, &key, &owed).await?,
    _ => {}
  }

  Ok(())
}

// One event's notice owed to whom it calls for.
async fn told(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  service: &JobService,
  event: &Value,
) -> Result<(), String> {
  let data = &event["data"];

  match event["kind"].as_str().unwrap_or_default() {
    "item.changed" => {
      deploy_told(db, vars, service, data).await?;

      let moved_to_done =
        data["state"] == "done" && data["previousState"].as_str().is_some_and(|was| was != "done");

      if !moved_to_done {
        return Ok(());
      }

      let by = data["changedBy"].as_str().unwrap_or("the shared token");
      let orderer = service.orderer_of(data).await?;

      if by_orderer(db, by, &orderer).await {
        return Ok(());
      }

      outbox::owe(db, vars, Notice::instruction(job_notices::closed(data, by, &orderer), Keep::Once))
        .await?;
      println!("queue: item #{} closed by {by}, owed to its orderer", data["id"]);
    }
    "question.created" => {
      let Some(id) = data["id"].as_i64() else { return Ok(()) };

      // An asker that tells of its question itself has already put it, or closed it unsent.
      if data["toldByAsker"].as_bool().unwrap_or(false) {
        return Ok(());
      }

      let orderer = match data["itemId"].as_i64() {
        Some(item) => service.orderer_of_id(item).await?.1,
        None => Orderer {
          room: crate::queue::orderer::room_of(data),
          requester: None,
        },
      };
      let orderer = Orderer {
        room: orderer.room.or_else(|| crate::queue::orderer::room_of(data)),
        ..orderer
      };

      outbox::owe(db, vars, Notice::instruction(job_notices::question(data, &orderer), Keep::Kept)).await?;
      println!("queue: question #{id} owed to its orderer");
    }
    "question.due" => {
      let Some(question) = data["id"].as_i64() else { return Ok(()) };

      crate::job_review::question_due(db, vars, question).await;
      outbox::owe(db, vars, Notice::owner(question, job_notices::question_due(data))).await?;
      println!("queue: question #{question} past its due time, owed to the owner");
    }
    "reply.created" => {
      let settles = data["settles"].as_bool().unwrap_or(false);
      let Some(question) = data["questionId"].as_i64().filter(|_| settles) else {
        return Ok(());
      };

      crate::job_review::ruled(db, vars, data).await;
      outbox::let_go(db, vars, &format!("{QUESTION_OWED}{question}")).await?;
      println!("queue: question #{question} answered, its instruction let go");
    }
    _ => {}
  }

  Ok(())
}
