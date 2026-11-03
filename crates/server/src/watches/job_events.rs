//! The job service's events an item's orderer and the owner are told of (docs/agent-control.md, 「An item's
//! change is told to its orderer」): read as a page past the cursor agency keeps, each event told once — an item
//! closed by somebody other than its orderer, a run's question asked, a question past its due time, and a
//! settling reply that withdraws the question's instruction. A pass with no cursor, or one whose cursor left
//! the service's window, reads to the end and tells nothing of what it passed.

use std::collections::BTreeMap;

use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::job_notices::{self, Receiver};
use crate::queue::orderer::{JobService, Orderer};
use crate::queue::settings;

/// The settings row the number of the last event read stands in.
pub const CURSOR_KEY: &str = "job_events_after";
/// The settings rows, one a question by its number, of the receiver's id of the instruction it was told with.
pub const QUESTION_TOLD_PREFIX: &str = "question_told:";
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

/// One pass over the job service's events past the cursor, each told once; nothing where the deployment names
/// no job service or no receiver.
pub async fn pass(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let (Some(service), Some(receiver)) = (JobService::of(vars), Receiver::of(vars)) else {
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
      if telling && let Err(why) = told(db, &service, &receiver, event).await {
        eprintln!("queue: event {} was not told: {why}", event["id"]);
      }
    }

    if let Some(last) = events.last().and_then(|event| event["id"].as_i64()) {
      after = Some(last);
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

// Whether the item's own orderer closed it: its changer is the requester of the item the orderer was found on,
// or a supervisor agency holds.
async fn by_orderer(db: &DatabaseConnection, by: &str, orderer: &Orderer) -> bool {
  orderer.requester.as_deref() == Some(by)
    || crate::supervisors::named(db, by).await.ok().flatten().is_some()
}

// One event told to whom it calls for.
async fn told(
  db: &DatabaseConnection,
  service: &JobService,
  receiver: &Receiver,
  event: &Value,
) -> Result<(), String> {
  let data = &event["data"];

  match event["kind"].as_str().unwrap_or_default() {
    "item.changed" => {
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

      receiver.instruct(&job_notices::closed(data, by, &orderer)).await?;
      println!("queue: item #{} closed by {by}, told to its orderer", data["id"]);
    }
    "question.created" => {
      let Some(id) = data["id"].as_i64() else { return Ok(()) };
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

      if let Some(instruction) = receiver.instruct(&job_notices::question(data, &orderer)).await? {
        settings::set(db, &format!("{QUESTION_TOLD_PREFIX}{id}"), &instruction.to_string())
          .await
          .map_err(|why| why.detail)?;
      }

      println!("queue: question #{id} told to its orderer");
    }
    "question.due" => {
      receiver.notify_owner(&job_notices::question_due(data)).await?;
      println!("queue: question #{} past its due time, told to the owner", data["id"]);
    }
    "reply.created" => {
      let settles = data["settles"].as_bool().unwrap_or(false);
      let Some(question) = data["questionId"].as_i64().filter(|_| settles) else {
        return Ok(());
      };
      let key = format!("{QUESTION_TOLD_PREFIX}{question}");

      if let Some(instruction) = settings::get(db, &key)
        .await
        .map_err(|why| why.detail)?
        .and_then(|value| value.trim().parse::<i64>().ok())
      {
        receiver.withdraw(instruction).await?;
        println!("queue: question #{question} answered, its instruction {instruction} withdrawn");
      }

      settings::forget(db, &key).await.map_err(|why| why.detail)?;
    }
    _ => {}
  }

  Ok(())
}
