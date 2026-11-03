//! The idle queue's alarm (docs/agent-control.md, 「An idle queue is told to the owner」): open work in the
//! job service that a free worker could take, and no job started for the idle minutes, told to the owner once
//! for each idle stretch through the receiver's owner notice; and an open item waiting a minute on its own
//! free worker, told to the supervisor once for that item. The owner's switch silences both until the next
//! job starts. A draft awaiting approval is told to the supervisor once too, in the room of whoever ordered
//! it: its own room, else its parent's up the `raised-by` chain.

use std::collections::{BTreeMap, BTreeSet};

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
/// The settings rows, one an item by its number, of when the supervisor was told the item awaits a launch.
pub const LAUNCH_TOLD_PREFIX: &str = "queue_item_told:";

/// The settings rows, one an item by its number, of when the supervisor was told the draft awaits approval.
pub const DRAFT_TOLD_PREFIX: &str = "queue_draft_told:";
/// The label naming the item whose job raised an item, by its number.
pub const RAISED_BY: &str = "raised-by";

/// An open item as the alarm reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
  pub id: i64,
  pub title: String,
  /// The worker it is to go to, where it names one.
  pub assignee: Option<String>,
  /// The room it was asked in, where it names one: its `roomId`, else the room its `scope` names.
  pub room: Option<i64>,
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

/// The items the supervisor is to be told of at `now`: each awaiting a launch since `seen` names it for the
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

/// The instruction an item awaiting a launch is queued with for the supervisor, in the room the item was asked
/// in, or the owner's private room for an item that names none.
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
    "room": item.room,
    "said": { "order": order },
    "sort": "notice",
    "kind": format!("item-awaits-launch:{}", item.id),
    "summary": format!("Item #{} awaits a launch", item.id),
    "events": [format!("item-awaits-launch:#{}", item.id)],
    "run": Value::Null,
  })
}

/// A draft as the pass reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
  pub id: i64,
  pub title: String,
  /// Who raised it: the item its `raised-by` label names, else its requester.
  pub raised_by: Option<String>,
  /// The item whose job raised it, where its `raised-by` label names one.
  pub parent: Option<i64>,
  /// The repository its work is in, its `assigneeName`.
  pub repository: Option<String>,
  pub brief: String,
  /// The room it was asked in, where it names one itself.
  pub room: Option<i64>,
}

/// The item an item's `raised-by` label names, by its number.
pub fn parent_of(item: &Value) -> Option<i64> {
  let label = &item["labels"][RAISED_BY];

  label
    .as_i64()
    .or_else(|| label.as_str().and_then(|number| number.trim().trim_start_matches('#').parse().ok()))
    .filter(|parent| *parent > 0)
}

/// A job service item read as a draft; none for a member that carries no number.
pub fn draft_of(item: &Value) -> Option<Draft> {
  let parent = parent_of(item);
  let requester = item["requesterName"].as_str().map(str::trim).filter(|name| !name.is_empty());

  Some(Draft {
    id: item["id"].as_i64()?,
    title: item["title"].as_str().unwrap_or_default().to_owned(),
    raised_by: parent.map(|parent| format!("item #{parent}")).or(requester.map(str::to_owned)),
    parent,
    repository: item["assigneeName"].as_str().map(str::to_owned),
    brief: item["brief"].as_str().unwrap_or_default().to_owned(),
    room: room_of(item),
  })
}

/// The drafts the supervisor is yet to be told of.
pub fn drafts_due<'a>(drafts: &'a [Draft], told: &BTreeSet<i64>) -> Vec<&'a Draft> {
  drafts.iter().filter(|draft| !told.contains(&draft.id)).collect()
}

/// The first lines of a brief the supervisor is told, blank ones left out, cut at the policy's characters.
pub fn brief_head(brief: &str) -> String {
  let head = brief
    .lines()
    .map(str::trim_end)
    .filter(|line| !line.trim().is_empty())
    .take(QUEUE_IDLE.draft_brief_lines)
    .collect::<Vec<_>>()
    .join("\n");

  if head.chars().count() <= QUEUE_IDLE.draft_brief_chars {
    return head;
  }

  let mut cut: String = head.chars().take(QUEUE_IDLE.draft_brief_chars).collect();

  cut.push('…');
  cut
}

/// The instruction a draft awaiting approval is queued with for the supervisor that ordered it, in `room`, or
/// the owner's private room where none was found. It is an order under the sort `job`, as a job's own notices
/// are, so it reaches the room's supervisor.
pub fn approval_instruction(draft: &Draft, room: Option<i64>) -> Value {
  let number = draft.id.to_string();
  let brief = brief_head(&draft.brief);
  let order = fill_marks(
    &QUEUE_IDLE.draft_order,
    &[
      ("item", &number),
      ("title", &draft.title),
      ("raised_by", draft.raised_by.as_deref().unwrap_or("no one named")),
      ("repository", draft.repository.as_deref().unwrap_or("none, host work")),
      ("brief", if brief.is_empty() { "(no brief)" } else { &brief }),
    ],
  );

  json!({
    "room": room,
    "said": { "order": order },
    "agentName": Value::Null,
    "sort": "job",
    "kind": format!("item-awaits-approval:{}", draft.id),
    "summary": format!("Item #{} awaits approval", draft.id),
    "events": [format!("item-awaits-approval:#{}", draft.id)],
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

/// The items awaiting a launch told to the supervisor once each, and the rows kept for them brought up to
/// date: a waiting item first seen now marked, one no longer waiting let go, and a told item that is no
/// longer open let go.
pub async fn tell_launches(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  standing: &Standing,
  now: i64,
) -> Result<(), String> {
  // A deployment that carries no receiver has no supervisor to tell.
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
      "queue: item #{} waited on its free worker {} for a launch, told to the supervisor",
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
  // An item a job of agency's is still at work on waits on nothing, its lease run out or not.
  let working: Vec<i64> =
    all(db, r#"SELECT "item_id" FROM "jobs" WHERE "state" IN ('starting', 'running')"#, vec![])
      .await?
      .iter()
      .filter_map(|row| row.try_get::<i64>("", "item_id").ok())
      .collect();
  let open = open.into_iter().filter(|item| !working.contains(&item.id)).collect();
  let busy: Vec<String> = all(
    db,
    r#"SELECT DISTINCT "worker_name" FROM "jobs" WHERE "state" IN ('starting', 'running')"#,
    vec![],
  )
  .await?
  .iter()
  .filter_map(|row| row.try_get::<String>("", "worker_name").ok())
  .collect();
  let free = crate::queue::workers::workers(db, std::path::Path::new(""))
    .await?
    .into_iter()
    .filter(|worker| !worker.builtin && !busy.contains(&worker.name))
    .map(|worker| worker.name)
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
    .send()
    .await
    .map_err(|why| why.to_string())?;

  if !answer.status().is_success() {
    return Err(format!("the job service answered {} to {path}", answer.status().as_u16()));
  }

  answer.json().await.map_err(|why| why.to_string())
}

// The job service's open items in the queues that take leases — open, holding nothing; none where the
// deployment names no job service. An item stands open only once an approver approved it or it was raised
// with an unattended label, so its state is the whole of whether it may be launched.
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
          room: room_of(item),
        })
      })
      .collect(),
  ))
}

// One item of the job service by its number; none where it answers that it holds no such item.
async fn read_item(url: &str, token: &str, id: i64) -> Result<Option<Value>, String> {
  let answer = reqwest::Client::new()
    .get(format!("{url}/items/{id}"))
    .bearer_auth(token)
    .send()
    .await
    .map_err(|why| why.to_string())?;

  if answer.status() == reqwest::StatusCode::NOT_FOUND {
    return Ok(None);
  }

  if !answer.status().is_success() {
    return Err(format!("the job service answered {} to /items/{id}", answer.status().as_u16()));
  }

  answer.json().await.map(Some).map_err(|why| why.to_string())
}

// The job service's drafts, every page of them.
async fn drafts(url: &str, token: &str) -> Result<Vec<Draft>, String> {
  let mut held = Vec::new();
  let mut after: Option<String> = None;

  loop {
    let mut query = vec![("state", "draft"), ("limit", "500")];

    if let Some(after) = &after {
      query.push(("after", after));
    }

    let page = read_job(url, token, "/items", &query).await?;
    let members = page["items"].as_array().cloned().unwrap_or_default();

    held.extend(members.iter().filter_map(draft_of));

    match members.last().and_then(|last| last["id"].as_i64()) {
      Some(last) if page["more"] == true => after = Some(last.to_string()),
      _ => return Ok(held),
    }
  }
}

// The room of the supervisor that ordered a draft: its own room, else the room of the item its `raised-by`
// label names, followed up the chain while each names none of its own. None where the chain reaches no room,
// an item the job service no longer holds, or an item already passed.
async fn orderer_room(url: &str, token: &str, draft: &Draft) -> Result<Option<i64>, String> {
  if draft.room.is_some() {
    return Ok(draft.room);
  }

  let mut passed = BTreeSet::from([draft.id]);
  let mut next = draft.parent;

  while let Some(id) = next.filter(|id| passed.insert(*id)) {
    let Some(item) = read_item(url, token, id).await? else { return Ok(None) };

    if let Some(room) = room_of(&item) {
      return Ok(Some(room));
    }

    next = parent_of(&item);
  }

  Ok(None)
}

/// Each draft awaiting approval told to the supervisor that ordered it, once: the told rows of items no longer
/// a draft let go, and a row kept for each told now.
pub async fn tell_drafts(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let (Some(url), Some(token), Some(_)) = (
    address(vars, "BMO_JOB_URL"),
    token(vars, "BMO_JOB_TOKEN", "BMO_JOB_TOKEN_FILE"),
    address(vars, "BMO_RECEIVER_URL"),
  ) else {
    return Ok(());
  };
  let fault = |why: Refusal| why.detail;
  let drafts = drafts(&url, &token).await?;
  let standing: BTreeSet<i64> = drafts.iter().map(|draft| draft.id).collect();
  let told = marks(db, DRAFT_TOLD_PREFIX).await.map_err(fault)?;

  for id in told.keys().filter(|id| !standing.contains(id)) {
    settings::forget(db, &format!("{DRAFT_TOLD_PREFIX}{id}")).await.map_err(fault)?;
  }

  let told: BTreeSet<i64> = told.into_keys().collect();

  for draft in drafts_due(&drafts, &told) {
    let room = orderer_room(&url, &token, draft).await?;

    instruct(vars, &approval_instruction(draft, room)).await?;
    settings::set(db, &format!("{DRAFT_TOLD_PREFIX}{}", draft.id), &crate::now_ms().to_string())
      .await
      .map_err(fault)?;
    println!("queue: draft #{} awaits approval, told to the supervisor", draft.id);
  }

  Ok(())
}

/// The room an item was asked in: its `roomId`, else the room its `scope` names; none for an item that names
/// neither, which the owner's private room answers for.
pub fn room_of(item: &Value) -> Option<i64> {
  item["roomId"]
    .as_i64()
    .or_else(|| item["scope"].as_str().and_then(|scope| scope.trim().parse().ok()))
    .or_else(|| item["scope"].as_i64())
    .filter(|room| *room != 0)
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
    .send()
    .await
    .map_err(|why| why.to_string())?;

  if answer.status().is_success() {
    Ok(true)
  } else {
    Err(format!("the receiver answered {}", answer.status().as_u16()))
  }
}

/// One pass of the alarm: the queue read, each item awaiting a launch on its own free worker and each draft
/// awaiting approval told to the supervisor once, and an idle queue told to the owner once, the moments kept.
pub async fn pass(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let Some(open) = open_items(vars).await? else { return Ok(()) };
  let standing = held(db, open).await.map_err(|why| why.detail)?;
  let now = crate::now_ms();

  if let Err(why) = tell_launches(db, vars, &standing, now).await {
    eprintln!("queue: an item awaiting a launch was not told: {why}");
  }

  if let Err(why) = tell_drafts(db, vars).await {
    eprintln!("queue: a draft awaiting approval was not told: {why}");
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
