//! The idle queue's alarm (docs/agent-control.md, 「An idle queue is told to the owner」): approved work in the
//! job service that a free worker could take, and no job started for the idle minutes, told to the owner once
//! for each idle stretch through the receiver's owner notice; and an approved item waiting a minute on its own
//! free worker, told to the chat agent once for that item. The owner's switch silences both until the next
//! job starts.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use policy::fill_marks;
use policy::queue_idle::QUEUE_IDLE;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use store::{all, one};

use crate::Refusal;
use crate::job_end::{address, instruct, token};
use crate::queue::settings;

/// The settings row the owner silences the alarm with, `off`, until the next job starts.
pub const SWITCH_KEY: &str = "queue_idle_alarm";
/// The settings row the moment the owner was last told stands in.
pub const TOLD_KEY: &str = "queue_idle_told";
/// The settings row the owner's own idle minutes stand in.
pub const MINUTES_KEY: &str = "queue_idle_minutes";
/// The settings rows, one an item by its number, of when an item was first seen waiting on its own free worker.
pub const SEEN_PREFIX: &str = "queue_item_waiting:";
/// The settings rows, one an item by its number, of when the chat agent was told the item awaits a launch.
pub const LAUNCH_TOLD_PREFIX: &str = "queue_item_told:";

// How long either call waits.
const ANSWER_WITHIN: Duration = Duration::from_secs(10);

/// An open item as the alarm reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
  pub id: i64,
  pub title: String,
  /// The worker it is to go to, where it names one.
  pub assignee: Option<String>,
}

/// What the queue stands at as one pass reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Standing {
  /// The open items in the queues that take leases.
  pub open: Vec<Open>,
  /// The workers not resting with no job at work.
  pub free: Vec<String>,
  /// When the last job started, in epoch milliseconds, where one has.
  pub last_start: Option<i64>,
  /// When the owner was last told, in epoch milliseconds, where he was.
  pub told: Option<i64>,
  /// Whether the owner's switch silences the alarm.
  pub silenced: bool,
  /// The idle minutes that hold.
  pub minutes: i64,
}

/// The open items a free worker could take: those going to a free worker, and those naming no worker
/// while one is free.
pub fn waiting(standing: &Standing) -> Vec<&Open> {
  standing
    .open
    .iter()
    .filter(|item| match &item.assignee {
      Some(worker) => standing.free.contains(worker),
      None => !standing.free.is_empty(),
    })
    .collect()
}

/// The open items whose own assignee is a free worker.
pub fn awaiting_launch(standing: &Standing) -> Vec<&Open> {
  standing
    .open
    .iter()
    .filter(|item| item.assignee.as_ref().is_some_and(|worker| standing.free.contains(worker)))
    .collect()
}

/// The items the chat agent is to be told of at `now`: each awaiting a launch since `seen` names it for the
/// launch seconds and more, not told yet, and the switch on.
pub fn launches_due<'a>(
  standing: &'a Standing,
  seen: &BTreeMap<i64, i64>,
  told: &BTreeSet<i64>,
  now: i64,
) -> Vec<&'a Open> {
  if standing.silenced {
    return vec![];
  }

  awaiting_launch(standing)
    .into_iter()
    .filter(|item| !told.contains(&item.id))
    .filter(|item| seen.get(&item.id).is_some_and(|since| now - since >= QUEUE_IDLE.launch_seconds * 1000))
    .collect()
}

/// The instruction an item awaiting a launch is queued with for the chat agent, in the owner's private room.
pub fn launch_instruction(item: &Open) -> Value {
  let number = item.id.to_string();
  let order = fill_marks(
    &QUEUE_IDLE.launch_order,
    &[
      ("item", &number),
      ("title", &item.title),
      ("worker", item.assignee.as_deref().unwrap_or_default()),
    ],
  );

  json!({
    "room": Value::Null,
    "said": { "order": order },
    "sort": "notice",
    "kind": format!("item-awaits-launch:{}", item.id),
    "summary": format!("Item #{} awaits a launch", item.id),
    "events": [format!("item-awaits-launch:#{}", item.id)],
    "run": Value::Null,
  })
}

// The item numbers a family of rows is kept for, each with its moment.
async fn marks(db: &DatabaseConnection, prefix: &str) -> Result<BTreeMap<i64, i64>, Refusal> {
  let mut held = BTreeMap::new();

  for name in settings::named(db, prefix).await? {
    if let (Ok(id), Some(at)) = (name.parse::<i64>(), setting_number(db, &format!("{prefix}{name}")).await?)
    {
      held.insert(id, at);
    }
  }

  Ok(held)
}

/// The items awaiting a launch told to the chat agent once each, and the rows kept for them brought up to
/// date: a waiting item first seen now marked, one no longer waiting let go, and a told item that is no
/// longer open and approved let go.
pub async fn tell_launches(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  standing: &Standing,
  now: i64,
) -> Result<(), String> {
  // A deployment that carries no receiver has no chat agent to tell.
  if address(vars, "BMO_RECEIVER_URL").is_none() {
    return Ok(());
  }

  let fault = |why: Refusal| why.detail;
  let mut seen = marks(db, SEEN_PREFIX).await.map_err(fault)?;
  let told = marks(db, LAUNCH_TOLD_PREFIX).await.map_err(fault)?;
  let awaiting: BTreeSet<i64> = awaiting_launch(standing).iter().map(|item| item.id).collect();
  let open: BTreeSet<i64> = standing.open.iter().map(|item| item.id).collect();

  for id in &awaiting {
    if !seen.contains_key(id) {
      settings::set(db, &format!("{SEEN_PREFIX}{id}"), &now.to_string())
        .await
        .map_err(fault)?;
      seen.insert(*id, now);
    }
  }

  for id in seen.keys().filter(|id| !awaiting.contains(id)) {
    settings::forget(db, &format!("{SEEN_PREFIX}{id}")).await.map_err(fault)?;
  }

  for id in told.keys().filter(|id| !open.contains(id)) {
    settings::forget(db, &format!("{LAUNCH_TOLD_PREFIX}{id}")).await.map_err(fault)?;
  }

  let told: BTreeSet<i64> = told.into_keys().collect();

  for item in launches_due(standing, &seen, &told, now) {
    instruct(vars, &launch_instruction(item)).await?;
    settings::set(db, &format!("{LAUNCH_TOLD_PREFIX}{}", item.id), &now.to_string())
      .await
      .map_err(fault)?;
    println!(
      "queue: item #{} waited on its free worker {} for a launch, told to the chat agent",
      item.id,
      item.assignee.as_deref().unwrap_or_default()
    );
  }

  Ok(())
}

/// Whether the queue is idle at `now` and not yet told for this stretch: work a free worker could take, no
/// job started for the idle minutes, no telling since the last job started, and the switch on.
pub fn idle(standing: &Standing, now: i64) -> bool {
  let quiet_since = standing.last_start.unwrap_or(0);

  !standing.silenced
    && !waiting(standing).is_empty()
    && now - quiet_since >= standing.minutes * 60_000
    && standing.told.is_none_or(|told| told < quiet_since)
}

/// The owner notice an idle queue is told with.
pub fn notice(standing: &Standing) -> Value {
  let items: Vec<Value> = waiting(standing)
    .iter()
    .map(|item| json!({ "id": item.id, "title": item.title, "assignee": item.assignee }))
    .collect();

  json!({
    "code": QUEUE_IDLE.code,
    "openItems": items,
    "freeWorkers": standing.free,
    "idleMinutes": standing.minutes,
    "lastJobStartedAt": standing.last_start,
    "ask": QUEUE_IDLE.ask,
  })
}

async fn setting_number(db: &DatabaseConnection, key: &str) -> Result<Option<i64>, Refusal> {
  Ok(
    settings::get(db, key)
      .await?
      .and_then(|value| value.trim().parse::<i64>().ok())
      .filter(|value| *value > 0),
  )
}

/// What agency itself holds of the queue's standing: its workers, its jobs and the alarm's own rows.
pub async fn held(db: &DatabaseConnection, open: Vec<Open>) -> Result<Standing, Refusal> {
  let free = all(
    db,
    r#"SELECT a."name" FROM "agents" a
       WHERE a."kind" = 'worker' AND a."resting_since" IS NULL
         AND NOT EXISTS (SELECT 1 FROM "jobs" j WHERE j."worker_name" = a."name" AND j."state" IN ('starting', 'running'))
       ORDER BY a."name""#,
    vec![],
  )
  .await?
  .iter()
  .filter_map(|row| row.try_get::<String>("", "name").ok())
  .collect();
  let last_start = one(db, r#"SELECT MAX("started_at") AS "at" FROM "jobs""#, vec![])
    .await?
    .and_then(|row| row.try_get::<Option<i64>>("", "at").ok().flatten());

  Ok(Standing {
    open,
    free,
    last_start,
    told: setting_number(db, TOLD_KEY).await?,
    silenced: settings::get(db, SWITCH_KEY).await?.is_some_and(|value| value.trim() == "off"),
    minutes: setting_number(db, MINUTES_KEY).await?.unwrap_or(QUEUE_IDLE.idle_minutes),
  })
}

// One read of the job service, its answer's body.
async fn read_job(url: &str, token: &str, path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
  let answer = reqwest::Client::new()
    .get(format!("{url}{path}"))
    .query(query)
    .bearer_auth(token)
    .timeout(ANSWER_WITHIN)
    .send()
    .await
    .map_err(|why| why.to_string())?;

  if !answer.status().is_success() {
    return Err(format!("the job service answered {} to {path}", answer.status().as_u16()));
  }

  answer.json().await.map_err(|why| why.to_string())
}

// The job service's approved open items in the queues that take leases — open, holding nothing; none
// where the deployment names no job service. Approved is the port's `approved`, an approval of the kind
// `approval`, which an item's state does not say.
async fn open_items(vars: &BTreeMap<String, String>) -> Result<Option<Vec<Open>>, String> {
  let (Some(url), Some(token)) =
    (address(vars, "BMO_JOB_URL"), token(vars, "BMO_JOB_TOKEN", "BMO_JOB_TOKEN_FILE"))
  else {
    return Ok(None);
  };
  let queues = read_job(&url, &token, "/queues", &[]).await?;
  let taking: Vec<&str> = queues["items"]
    .as_array()
    .into_iter()
    .flatten()
    .filter(|queue| {
      queue["state"] == "open" && queue["holds"].as_array().is_none_or(|holds| holds.is_empty())
    })
    .filter_map(|queue| queue["name"].as_str())
    .collect();
  let port = read_job(&url, &token, "/items", &[("open", "1")]).await?;
  let approved: Vec<i64> = port["items"]
    .as_array()
    .into_iter()
    .flatten()
    .filter(|item| item["approved"] == true)
    .filter_map(|item| item["id"].as_i64())
    .collect();
  let page = read_job(&url, &token, "/items", &[("state", "open"), ("limit", "100")]).await?;

  Ok(Some(
    page["items"]
      .as_array()
      .into_iter()
      .flatten()
      .filter(|item| item["queueName"].as_str().is_some_and(|queue| taking.contains(&queue)))
      .filter(|item| item["id"].as_i64().is_some_and(|id| approved.contains(&id)))
      .filter_map(|item| {
        Some(Open {
          id: item["id"].as_i64()?,
          title: item["title"].as_str().unwrap_or_default().to_owned(),
          assignee: item["assigneeName"].as_str().map(str::to_owned),
        })
      })
      .collect(),
  ))
}

// The owner notice queued with the receiver; nothing where the deployment names no receiver.
async fn tell(vars: &BTreeMap<String, String>, body: &Value) -> Result<bool, String> {
  let (Some(url), Some(token)) = (
    address(vars, "BMO_RECEIVER_URL"),
    token(vars, "BMO_RECEIVER_TOKEN", "BMO_RECEIVER_TOKEN_FILE"),
  ) else {
    return Ok(false);
  };
  let answer = reqwest::Client::new()
    .post(format!("{url}/owner/notices"))
    .bearer_auth(token)
    .json(body)
    .timeout(ANSWER_WITHIN)
    .send()
    .await
    .map_err(|why| why.to_string())?;

  if answer.status().is_success() {
    Ok(true)
  } else {
    Err(format!("the receiver answered {}", answer.status().as_u16()))
  }
}

/// One pass of the alarm: the queue read, each item awaiting a launch on its own free worker told to the chat
/// agent once, and an idle queue told to the owner once, the moments kept.
pub async fn pass(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let Some(open) = open_items(vars).await? else { return Ok(()) };
  let standing = held(db, open).await.map_err(|why| why.detail)?;
  let now = crate::now_ms();

  if let Err(why) = tell_launches(db, vars, &standing, now).await {
    eprintln!("queue: an item awaiting a launch was not told: {why}");
  }

  if !idle(&standing, now) {
    return Ok(());
  }

  if tell(vars, &notice(&standing)).await? {
    settings::set(db, TOLD_KEY, &now.to_string()).await.map_err(|why| why.detail)?;
    println!(
      "queue: idle for {} minutes with {} items a free worker could take, told to the owner",
      standing.minutes,
      waiting(&standing).len()
    );
  }

  Ok(())
}

/// A job started: the owner's switch on again.
pub async fn job_started(db: &DatabaseConnection) {
  if let Err(why) = settings::forget(db, SWITCH_KEY).await {
    eprintln!("queue: the idle alarm's switch was not turned on again: {}", why.detail);
  }
}
