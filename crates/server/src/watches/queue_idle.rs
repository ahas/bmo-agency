//! The idle queue's alarm (docs/agent-control.md, 「An idle queue is told to the owner」): approved work in the
//! job service that a free worker could take, and no job started for the idle minutes, told to the owner once
//! for each idle stretch through the receiver's owner notice, unless the owner's switch silences it until the
//! next job starts.

use std::collections::BTreeMap;
use std::time::Duration;

use policy::queue_idle::QUEUE_IDLE;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use store::{all, one};

use crate::Refusal;
use crate::job_end::{address, token};
use crate::queue::settings;

/// The settings row the owner silences the alarm with, `off`, until the next job starts.
pub const SWITCH_KEY: &str = "queue_idle_alarm";
/// The settings row the moment the owner was last told stands in.
pub const TOLD_KEY: &str = "queue_idle_told";
/// The settings row the owner's own idle minutes stand in.
pub const MINUTES_KEY: &str = "queue_idle_minutes";

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

// The job service's open items in the queues that take leases — open, holding nothing; none where the
// deployment names no job service.
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
  let page = read_job(&url, &token, "/items", &[("state", "open"), ("limit", "100")]).await?;

  Ok(Some(
    page["items"]
      .as_array()
      .into_iter()
      .flatten()
      .filter(|item| item["queueName"].as_str().is_some_and(|queue| taking.contains(&queue)))
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

/// One pass of the alarm: the queue read, and an idle one told to the owner once, the moment kept.
pub async fn pass(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let Some(open) = open_items(vars).await? else { return Ok(()) };
  let standing = held(db, open).await.map_err(|why| why.detail)?;
  let now = crate::now_ms();

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
