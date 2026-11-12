//! The idle queue's alarm (docs/agent-control.md, 「An idle queue is told to the owner」): open work in the
//! job service that a free checkout could take, and no job started for the idle minutes, told to the owner once
//! for each idle stretch through the receiver's owner notice. The same minute pass tells each item's orderer
//! what calls for its judgement (「An item's change is told to its orderer」): an open item waiting a minute on
//! a free checkout, an open item its paused queue holds, an open item whose assignee no checkout is of, and a
//! draft awaiting approval, each once; and it reads the job service's events for the rest. The owner's switch
//! silences the idle notice and the launch notice until the next job starts.

use std::collections::{BTreeMap, BTreeSet};

use policy::fill_marks;
use policy::job_notices::JOB_NOTICES;
use policy::queue_idle::QUEUE_IDLE;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use store::{all, one};

use crate::Refusal;
use crate::job_notices::{self, Receiver};
use crate::queue::orderer::{JobService, Orderer};
use crate::queue::settings;

pub use crate::queue::orderer::{RAISED_BY, parent_of, room_of};

/// The settings row the owner silences the alarm with, `off`, until the next job starts.
pub const SWITCH_KEY: &str = "queue_idle_alarm";
/// The settings row the moment the owner was last told stands in.
pub const TOLD_KEY: &str = "queue_idle_told";
/// The settings row the owner's own idle minutes stand in.
pub const MINUTES_KEY: &str = "queue_idle_minutes";
/// The settings rows, one an item by its number, of when an item was first seen waiting on a free checkout.
pub const SEEN_PREFIX: &str = "queue_item_waiting:";
/// The settings rows, one an item by its number, of when the orderer was told the item awaits a launch.
pub const LAUNCH_TOLD_PREFIX: &str = "queue_item_told:";
/// The settings rows, one an item by its number, of when the orderer was told the item waits on its paused
/// queue.
pub const PAUSED_TOLD_PREFIX: &str = "queue_paused_told:";
/// The settings rows, one an item by its number, of when the orderer was told the item names no checkout.
pub const UNLAUNCHABLE_TOLD_PREFIX: &str = "queue_unlaunchable_told:";
/// The settings rows, one an item by its number, of the `agent` label whose refused launch the orderer was
/// told of.
pub const AGENT_TOLD_PREFIX: &str = "queue_agent_told:";
/// The settings rows, one an item by its number, of when the orderer was told the draft awaits approval.
pub const DRAFT_TOLD_PREFIX: &str = "queue_draft_told:";

/// An open item as the alarm reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Open {
  pub id: i64,
  pub title: String,
  /// The checkout it is to go to, where it names one.
  pub assignee: Option<String>,
  /// The room it was asked in, where it names one: its `roomId`, else the room its `scope` names.
  pub room: Option<i64>,
  /// The item it was raised from, where it names one.
  pub parent: Option<i64>,
  /// The queue it waits in.
  pub queue: String,
  /// Whether its queue is paused or under a hold, and so gives no lease.
  pub paused: bool,
}

impl Open {
  /// The item as the orderer's resolver reads it.
  pub fn as_item(&self) -> Value {
    json!({ "id": self.id, "roomId": self.room, "raisedBy": self.parent })
  }
}

/// What the queue stands at as one pass reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Standing {
  /// The open items in the queues that take leases.
  pub open: Vec<Open>,
  /// The open items in the queues that give no lease.
  pub paused: Vec<Open>,
  /// The checkouts not resting with no job at work.
  pub free: Vec<String>,
  /// Every checkout agency holds, free or not, the built-in worker among them.
  pub known: Vec<String>,
  /// When the last job started, in epoch milliseconds, where one has.
  pub last_start: Option<i64>,
  /// When the owner was last told, in epoch milliseconds, where he was.
  pub told: Option<i64>,
  /// Whether the owner's switch silences the alarm.
  pub silenced: bool,
  /// The idle minutes that hold.
  pub minutes: i64,
}

/// The open items a free checkout could take: those going to a free checkout, and those naming none while one
/// is free.
pub fn waiting(standing: &Standing) -> Vec<&Open> {
  awaiting_launch(standing)
}

/// The open items awaiting a launch: those whose own assignee is a free checkout, and those naming none while
/// any checkout is free.
pub fn awaiting_launch(standing: &Standing) -> Vec<&Open> {
  standing
    .open
    .iter()
    .filter(|item| match &item.assignee {
      Some(worker) => standing.free.contains(worker),
      None => !standing.free.is_empty(),
    })
    .collect()
}

/// The open items whose assignee is no checkout agency holds, so no launch will ever take them.
pub fn unlaunchable(standing: &Standing) -> Vec<&Open> {
  standing
    .open
    .iter()
    .filter(|item| item.assignee.as_ref().is_some_and(|worker| !standing.known.contains(worker)))
    .collect()
}

/// The items the orderer is to be told of at `now`: each awaiting a launch since `seen` names it for the launch
/// seconds and more, not told yet, and the switch on.
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

/// The instruction an item awaiting a launch is queued with for its orderer.
pub fn launch_instruction(item: &Open, orderer: &Orderer) -> Value {
  let number = item.id.to_string();
  let order = fill_marks(
    &QUEUE_IDLE.launch_order,
    &[
      ("item", &number),
      ("title", &item.title),
      ("worker", item.assignee.as_deref().unwrap_or(&JOB_NOTICES.any_worker)),
    ],
  );

  job_notices::instruction(
    "item-awaits-launch",
    item.id,
    orderer,
    format!("Item #{} awaits a launch", item.id),
    order,
  )
}

/// A draft as the pass reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
  pub id: i64,
  pub title: String,
  /// Who raised it: the item it was raised from, else its requester.
  pub raised_by: Option<String>,
  /// The item it was raised from, where it names one: its `raisedBy`, else its `raised-by` label.
  pub parent: Option<i64>,
  /// The repository its work is in, its `assigneeName`.
  pub repository: Option<String>,
  pub brief: String,
  /// The room it was asked in, where it names one itself.
  pub room: Option<i64>,
}

impl Draft {
  /// The draft as the orderer's resolver reads it.
  pub fn as_item(&self) -> Value {
    json!({ "id": self.id, "roomId": self.room, "raisedBy": self.parent })
  }
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

/// The drafts the orderer is yet to be told of.
pub fn drafts_due<'a>(drafts: &'a [Draft], told: &BTreeSet<i64>) -> Vec<&'a Draft> {
  drafts.iter().filter(|draft| !told.contains(&draft.id)).collect()
}

/// The first lines of a brief the orderer is told, blank ones left out, cut at the policy's characters.
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

/// The instruction a draft awaiting approval is queued with for its orderer.
pub fn approval_instruction(draft: &Draft, orderer: &Orderer) -> Value {
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

  job_notices::instruction(
    "item-awaits-approval",
    draft.id,
    orderer,
    format!("Item #{} awaits approval", draft.id),
    order,
  )
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

// The told rows of a family let go for every item no longer among `standing`; the items still told answered.
async fn kept_marks(
  db: &DatabaseConnection,
  prefix: &str,
  standing: &BTreeSet<i64>,
) -> Result<BTreeSet<i64>, String> {
  let fault = |why: Refusal| why.detail;
  let told = marks(db, prefix).await.map_err(fault)?;

  for id in told.keys().filter(|id| !standing.contains(id)) {
    settings::forget(db, &format!("{prefix}{id}")).await.map_err(fault)?;
  }

  Ok(told.into_keys().filter(|id| standing.contains(id)).collect())
}

// The orderer of an item as the pass finds it: through the job service where the deployment names one, else
// the room the item names itself.
async fn orderer(service: Option<&JobService>, item: &Value) -> Result<Orderer, String> {
  match service {
    Some(service) => service.orderer_of(item).await,
    None => Ok(Orderer {
      room: room_of(item),
      requester: None,
    }),
  }
}

/// The items awaiting a launch told to their orderer once each, and the rows kept for them brought up to date:
/// a waiting item first seen now marked, one no longer waiting let go, and a told item that is no longer open
/// let go.
pub async fn tell_launches(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  standing: &Standing,
  now: i64,
) -> Result<(), String> {
  // A deployment that carries no receiver has no supervisor to tell.
  let Some(receiver) = Receiver::of(vars) else { return Ok(()) };
  let service = JobService::of(vars);
  let fault = |why: Refusal| why.detail;
  let mut seen = marks(db, SEEN_PREFIX).await.map_err(fault)?;
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

  let told = kept_marks(db, LAUNCH_TOLD_PREFIX, &open).await?;

  for item in launches_due(standing, &seen, &told, now) {
    let orderer = orderer(service.as_ref(), &item.as_item()).await?;

    receiver.instruct(&launch_instruction(item, &orderer)).await?;
    settings::set(db, &format!("{LAUNCH_TOLD_PREFIX}{}", item.id), &now.to_string())
      .await
      .map_err(fault)?;
    println!(
      "queue: item #{} waited on {} for a launch, told to its orderer",
      item.id,
      item.assignee.as_deref().unwrap_or(&JOB_NOTICES.any_worker)
    );
  }

  Ok(())
}

/// The open items that cannot go told to their orderer once each: those its paused queue holds, and those
/// whose assignee no checkout is of; each row let go once its item no longer stands so.
pub async fn tell_stuck(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  standing: &Standing,
  now: i64,
) -> Result<(), String> {
  let Some(receiver) = Receiver::of(vars) else { return Ok(()) };
  let service = JobService::of(vars);
  let fault = |why: Refusal| why.detail;
  let paused: BTreeSet<i64> = standing.paused.iter().map(|item| item.id).collect();
  let told = kept_marks(db, PAUSED_TOLD_PREFIX, &paused).await?;

  for item in standing.paused.iter().filter(|item| !told.contains(&item.id)) {
    let orderer = orderer(service.as_ref(), &item.as_item()).await?;

    receiver
      .instruct(&job_notices::paused(item.id, &item.title, &item.queue, &orderer))
      .await?;
    settings::set(db, &format!("{PAUSED_TOLD_PREFIX}{}", item.id), &now.to_string())
      .await
      .map_err(fault)?;
    println!("queue: item #{} waits on its paused queue {}, told to its orderer", item.id, item.queue);
  }

  let stuck = unlaunchable(standing);
  let standing_ids: BTreeSet<i64> = stuck.iter().map(|item| item.id).collect();
  let told = kept_marks(db, UNLAUNCHABLE_TOLD_PREFIX, &standing_ids).await?;

  for item in stuck.into_iter().filter(|item| !told.contains(&item.id)) {
    let assignee = item.assignee.as_deref().unwrap_or_default();
    let orderer = orderer(service.as_ref(), &item.as_item()).await?;

    receiver
      .instruct(&job_notices::unlaunchable(item.id, &item.title, assignee, &orderer))
      .await?;
    settings::set(db, &format!("{UNLAUNCHABLE_TOLD_PREFIX}{}", item.id), &now.to_string())
      .await
      .map_err(fault)?;
    println!(
      "queue: item #{} names {assignee}, which no checkout is of, told to its orderer",
      item.id
    );
  }

  Ok(())
}

/// An item whose launch was refused for its `agent` label told to its orderer, once for that label
/// (docs/agent-control.md, 「An item's change is told to its orderer」): the label told is kept by the item, so
/// the same label refused again is not told again and another one is. Answers whether it was told now.
pub async fn tell_agent_refused(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  id: i64,
  item: &Value,
  label: &str,
  why: &str,
) -> Result<bool, String> {
  let Some(receiver) = Receiver::of(vars) else { return Ok(false) };
  let fault = |why: Refusal| why.detail;
  let key = format!("{AGENT_TOLD_PREFIX}{id}");

  if settings::get(db, &key).await.map_err(fault)?.as_deref() == Some(label) {
    return Ok(false);
  }

  let mut item = item.clone();

  item["id"] = json!(id);

  let orderer = orderer(JobService::of(vars).as_ref(), &item).await?;
  let title = item["title"].as_str().unwrap_or_default();

  receiver.instruct(&job_notices::agent_refused(id, title, label, why, &orderer)).await?;
  settings::set(db, &key, label).await.map_err(fault)?;
  println!("queue: item #{id} names agent={label}, which no job can run on, told to its orderer");

  Ok(true)
}

/// The mark of an item's refused `agent` label let go, as a job of the item is launched.
pub async fn agent_told_let_go(db: &DatabaseConnection, id: i64) {
  if let Err(why) = settings::forget(db, &format!("{AGENT_TOLD_PREFIX}{id}")).await {
    eprintln!("queue: the told mark of item #{id}'s agent was not let go: {}", why.detail);
  }
}

/// Whether the queue is idle at `now` and not yet told for this stretch: work a free checkout could take, no
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

/// What agency itself holds of the queue's standing: its checkouts, its jobs and the alarm's own rows, the open
/// items split by whether their queue gives a lease.
pub async fn held(db: &DatabaseConnection, open: Vec<Open>) -> Result<Standing, Refusal> {
  // An item a job of agency's is still at work on waits on nothing, its lease run out or not.
  let working: Vec<i64> = all(
    db,
    r#"SELECT "item_id" FROM "jobs" WHERE "state" IN ('starting', 'running', 'reviewing')"#,
    vec![],
  )
  .await?
  .iter()
  .filter_map(|row| row.try_get::<i64>("", "item_id").ok())
  .collect();
  let (paused, open): (Vec<Open>, Vec<Open>) = open
    .into_iter()
    .filter(|item| !working.contains(&item.id))
    .partition(|item| item.paused);
  let busy: Vec<String> = all(
    db,
    r#"SELECT DISTINCT "worker_name" FROM "jobs"
       WHERE "state" IN ('starting', 'running') OR ("state" = 'reviewing' AND NOT "review_parked")"#,
    vec![],
  )
  .await?
  .iter()
  .filter_map(|row| row.try_get::<String>("", "worker_name").ok())
  .collect();
  let workers = crate::queue::workers::workers(db, std::path::Path::new("")).await?;
  let known = workers.iter().map(|worker| worker.name.clone()).collect();
  let free = workers
    .into_iter()
    .filter(|worker| !worker.builtin && !busy.contains(&worker.name))
    .map(|worker| worker.name)
    .collect();
  let last_start = one(db, r#"SELECT MAX("started_at") AS "at" FROM "jobs""#, vec![])
    .await?
    .and_then(|row| row.try_get::<Option<i64>>("", "at").ok().flatten());

  Ok(Standing {
    open,
    paused,
    free,
    known,
    last_start,
    told: setting_number(db, TOLD_KEY).await?,
    silenced: settings::get(db, SWITCH_KEY).await?.is_some_and(|value| value.trim() == "off"),
    minutes: setting_number(db, MINUTES_KEY).await?.unwrap_or(QUEUE_IDLE.idle_minutes),
  })
}

// One read of the job service, its answer's body.
async fn read_job(service: &JobService, path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
  let answer = reqwest::Client::new()
    .get(format!("{}{path}", service.url))
    .query(query)
    .bearer_auth(&service.token)
    .send()
    .await
    .map_err(|why| why.to_string())?;

  if !answer.status().is_success() {
    return Err(format!("the job service answered {} to {path}", answer.status().as_u16()));
  }

  answer.json().await.map_err(|why| why.to_string())
}

// Every page of a list of the job service's, past `after` while it says more stand.
async fn every_page(
  service: &JobService,
  path: &str,
  query: &[(&str, &str)],
) -> Result<Vec<Value>, String> {
  let mut held = Vec::new();
  let mut after: Option<String> = None;

  loop {
    let mut asked = query.to_vec();

    asked.push(("limit", "500"));

    if let Some(after) = &after {
      asked.push(("after", after));
    }

    let page = read_job(service, path, &asked).await?;
    let members = page["items"].as_array().cloned().unwrap_or_default();

    held.extend(members.iter().cloned());

    match members.last().and_then(|last| last["id"].as_i64()) {
      Some(last) if page["more"] == true => after = Some(last.to_string()),
      _ => return Ok(held),
    }
  }
}

// The job service's open items, each marked by whether its queue gives a lease — open, holding nothing. An item
// stands open only once an approver approved it or it was raised with an unattended label, so its state is the
// whole of whether it may be launched.
async fn open_items(service: &JobService) -> Result<Vec<Open>, String> {
  let queues = read_job(service, "/queues", &[]).await?;
  let taking: Vec<&str> = queues["items"]
    .as_array()
    .into_iter()
    .flatten()
    .filter(|queue| {
      queue["state"] == "open" && queue["holds"].as_array().is_none_or(|holds| holds.is_empty())
    })
    .filter_map(|queue| queue["name"].as_str())
    .collect();

  Ok(
    every_page(service, "/items", &[("state", "open")])
      .await?
      .iter()
      .filter_map(|item| {
        let queue = item["queueName"].as_str().unwrap_or_default().to_owned();

        Some(Open {
          id: item["id"].as_i64()?,
          title: item["title"].as_str().unwrap_or_default().to_owned(),
          assignee: item["assigneeName"].as_str().filter(|name| !name.is_empty()).map(str::to_owned),
          room: room_of(item),
          parent: parent_of(item),
          paused: !taking.contains(&queue.as_str()),
          queue,
        })
      })
      .collect(),
  )
}

/// Each draft awaiting approval told to its orderer, once: the told rows of items no longer a draft let go, and
/// a row kept for each told now.
pub async fn tell_drafts(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let (Some(service), Some(receiver)) = (JobService::of(vars), Receiver::of(vars)) else {
    return Ok(());
  };
  let fault = |why: Refusal| why.detail;
  let drafts: Vec<Draft> = every_page(&service, "/items", &[("state", "draft")])
    .await?
    .iter()
    .filter_map(draft_of)
    .collect();
  let standing: BTreeSet<i64> = drafts.iter().map(|draft| draft.id).collect();
  let told = kept_marks(db, DRAFT_TOLD_PREFIX, &standing).await?;

  for draft in drafts_due(&drafts, &told) {
    let orderer = service.orderer_of(&draft.as_item()).await?;

    receiver.instruct(&approval_instruction(draft, &orderer)).await?;
    settings::set(db, &format!("{DRAFT_TOLD_PREFIX}{}", draft.id), &crate::now_ms().to_string())
      .await
      .map_err(fault)?;
    println!("queue: draft #{} awaits approval, told to its orderer", draft.id);
  }

  Ok(())
}

/// One pass of the alarm: the queue read; each item awaiting a launch, each open item that cannot go and each
/// draft awaiting approval told to its orderer once; the job service's events told; and an idle queue told to
/// the owner once, the moments kept.
pub async fn pass(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let Some(service) = JobService::of(vars) else { return Ok(()) };
  let open = open_items(&service).await?;
  let standing = held(db, open).await.map_err(|why| why.detail)?;
  let now = crate::now_ms();

  if let Err(why) = tell_launches(db, vars, &standing, now).await {
    eprintln!("queue: an item awaiting a launch was not told: {why}");
  }

  if let Err(why) = tell_stuck(db, vars, &standing, now).await {
    eprintln!("queue: an item that cannot go was not told: {why}");
  }

  if let Err(why) = tell_drafts(db, vars).await {
    eprintln!("queue: a draft awaiting approval was not told: {why}");
  }

  if let Err(why) = crate::watches::job_events::pass(db, vars).await {
    eprintln!("queue: the job service's events were not told: {why}");
  }

  if !idle(&standing, now) {
    return Ok(());
  }

  let Some(receiver) = Receiver::of(vars) else { return Ok(()) };

  receiver.notify_owner(&notice(&standing)).await?;
  settings::set(db, TOLD_KEY, &now.to_string()).await.map_err(|why| why.detail)?;
  println!(
    "queue: idle for {} minutes with {} items a free checkout could take, told to the owner",
    standing.minutes,
    waiting(&standing).len()
  );

  Ok(())
}

/// A job started: the owner's switch on again.
pub async fn job_started(db: &DatabaseConnection) {
  if let Err(why) = settings::forget(db, SWITCH_KEY).await {
    eprintln!("queue: the idle alarm's switch was not turned on again: {}", why.detail);
  }
}
