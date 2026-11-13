//! The idle queue's alarm (docs/agent-control.md, 「An idle queue is told to the owner」): open work in the
//! job service that a free checkout could take, and no job started for the idle minutes, told to the owner once
//! for each idle stretch through the receiver's owner notice. The same minute pass owes each item's orderer
//! what calls for its judgement (「An item's change is told to its orderer」), each in agency's outbox: an open
//! item waiting a minute on a free checkout, standing while it waits so, and, each once, an open item its paused
//! queue holds, an open item whose assignee no checkout is of, a draft awaiting approval, and a held item every
//! item it waits on has ended; and it reads the job service's events for the rest. The owner's switch silences
//! the idle notice and the launch notice until the next job starts.

use std::collections::{BTreeMap, BTreeSet};

use policy::fill_marks;
use policy::job_notices::JOB_NOTICES;
use policy::queue_idle::QUEUE_IDLE;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use store::{all, one};

use crate::Refusal;
use crate::job_notices;
use crate::outbox::{self, Keep, Notice, Receiver};
use crate::queue::orderer::{JobService, Orderer};
use crate::queue::settings;
use crate::repositories::{AGENT_LABEL, MODEL_LABEL, ROLE_LABEL, named_by};
use crate::role_agents::RoleAgent;

pub use crate::queue::orderer::{RAISED_BY, parent_of, room_of};

/// The settings row the owner silences the alarm with, `off`, until the next job starts.
pub const SWITCH_KEY: &str = "queue_idle_alarm";
/// The settings row the moment the owner was last told stands in.
pub const TOLD_KEY: &str = "queue_idle_told";
/// The settings row the owner's own idle minutes stand in.
pub const MINUTES_KEY: &str = "queue_idle_minutes";
/// The settings rows, one an item by its number, of when an item was first seen waiting on a free checkout.
pub const SEEN_PREFIX: &str = "queue_item_waiting:";
/// The prefix of the key an item awaiting a launch is owed under, its number after it.
pub const LAUNCH_OWED: &str = "item-awaits-launch:";
/// The settings rows, one an item by its number, of when the orderer was told the item waits on its paused
/// queue.
pub const PAUSED_TOLD_PREFIX: &str = "queue_paused_told:";
/// The settings rows, one an item by its number, of when the orderer was told the item names no checkout.
pub const UNLAUNCHABLE_TOLD_PREFIX: &str = "queue_unlaunchable_told:";
/// The settings rows, one an item by its number, of the `agent` label whose refused launch the orderer was
/// told of.
pub const AGENT_TOLD_PREFIX: &str = "queue_agent_told:";
/// The settings rows, one an item by its number, of the role and the model the orderer was told no agent runs.
pub const NO_CANDIDATE_TOLD_PREFIX: &str = "queue_no_candidate_told:";
/// The settings rows, one an item by its number, of when the orderer was told the draft awaits approval.
pub const DRAFT_TOLD_PREFIX: &str = "queue_draft_told:";
/// The settings rows, one an item by its number, of when the orderer was told the held item's wait has ended.
pub const WAIT_TOLD_PREFIX: &str = "queue_wait_told:";

/// An open item as the alarm reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Open {
  pub id: i64,
  pub title: String,
  /// The checkout it is to go to, where it names one.
  pub assignee: Option<String>,
  /// The agent its `agent` label names, where it names one: its job waits for that agent alone.
  pub agent: Option<String>,
  /// The model its `model` label names, where it names one: with no agent named, its job waits for a free agent
  /// of its role on that model.
  pub model: Option<String>,
  /// The role its `role` label names, where it names one.
  pub role: Option<String>,
  /// Its priority in its queue, the higher first.
  pub priority: i64,
  /// Its place in its queue among items of one priority, the lower first.
  pub position: i64,
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
    json!({ "id": self.id, "title": self.title, "roomId": self.room, "raisedBy": self.parent })
  }

  /// The role its job runs as: its `role` label's, else the role a job runs as.
  pub fn role(&self) -> &str {
    self.role.as_deref().unwrap_or_else(|| crate::agent_roles::job_role())
  }

  /// The model its job is to take an agent for: its `model` label's where no `agent` label names one
  /// (docs/repositories.md, 「An agent an item names」).
  pub fn asked_model(&self) -> Option<&str> {
    self.model.as_deref().filter(|_| self.agent.is_none())
  }

  // Its place in the queue's order: the highest priority, then the earliest place, then the lowest number.
  fn place(&self) -> (std::cmp::Reverse<i64>, i64, i64) {
    (std::cmp::Reverse(self.priority), self.position, self.id)
  }
}

/// What the queue stands at as one pass reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Standing {
  /// The open items in the queues that take leases.
  pub open: Vec<Open>,
  /// The open items in the queues that give no lease.
  pub paused: Vec<Open>,
  /// The checkouts not resting with no job at work.
  pub free: Vec<String>,
  /// Every checkout agency holds, free or not, the built-in worker among them.
  pub known: Vec<String>,
  /// The agents busy now: a job or a slot carries a run at work on them (docs/role-agents.md, 「When an agent is
  /// busy」).
  pub carrying: Vec<String>,
  /// Every agent the store holds, by role and then by name.
  pub agents: Vec<RoleAgent>,
  /// When the last job started, in epoch milliseconds, where one has.
  pub last_start: Option<i64>,
  /// When the owner was last told, in epoch milliseconds, where he was.
  pub told: Option<i64>,
  /// Whether the owner's switch silences the alarm.
  pub silenced: bool,
  /// The idle minutes that hold.
  pub minutes: i64,
}

impl Standing {
  /// The agents of a role, switched on, whose road runs a model, by name: the candidates of an item naming that
  /// model (docs/role-agents.md, 「Which agent a run takes」).
  pub fn candidates<'a>(&'a self, role: &'a str, model: &'a str) -> impl Iterator<Item = &'a RoleAgent> {
    self
      .agents
      .iter()
      .filter(move |agent| agent.enabled && agent.role == role && agent.runs(model))
  }

  /// The candidates of a role for a model that carry no run now.
  pub fn free_candidates<'a>(&'a self, role: &'a str, model: &'a str) -> impl Iterator<Item = &'a str> {
    self
      .candidates(role, model)
      .map(|agent| agent.name.as_str())
      .filter(|name| !self.carrying.iter().any(|busy| busy == name))
  }
}

/// The open items a free checkout could take: those going to a free checkout, and those naming none while one
/// is free.
pub fn waiting(standing: &Standing) -> Vec<&Open> {
  awaiting_launch(standing)
}

/// The open items awaiting a launch: those whose own assignee is a free checkout, and those naming none while
/// any checkout is free; an item whose `agent` label names an agent carrying a run waits on that agent instead,
/// and one naming a model and no agent waits until one agent of its role on that model is free.
pub fn awaiting_launch(standing: &Standing) -> Vec<&Open> {
  standing
    .open
    .iter()
    .filter(|item| match &item.assignee {
      Some(worker) => standing.free.contains(worker),
      None => !standing.free.is_empty(),
    })
    .filter(|item| match (&item.agent, item.asked_model()) {
      (Some(agent), _) => !standing.carrying.contains(agent),
      (None, Some(model)) => standing.free_candidates(item.role(), model).next().is_some(),
      (None, None) => true,
    })
    .collect()
}

/// The agent each item awaiting a launch waits to go on, by the item's number: the agent its `agent` label names;
/// for an item naming a model, going through the items in the queue's order, the first of its free candidates by
/// name that no item before it is set on, else the first of them, so free agents of one model are offered
/// different items at once. An item naming neither waits on no agent.
pub fn seats(standing: &Standing) -> BTreeMap<i64, String> {
  let mut awaiting = awaiting_launch(standing);
  let mut set: BTreeMap<i64, String> = BTreeMap::new();

  awaiting.sort_by_key(|item| item.place());

  for item in awaiting {
    let seat = match (&item.agent, item.asked_model()) {
      (Some(agent), _) => Some(agent.as_str()),
      (None, Some(model)) => {
        let free: Vec<&str> = standing.free_candidates(item.role(), model).collect();

        free
          .iter()
          .find(|name| !set.values().any(|taken| taken == *name))
          .or(free.first())
          .copied()
      }
      (None, None) => None,
    };

    if let Some(seat) = seat {
      set.insert(item.id, seat.to_owned());
    }
  }

  set
}

/// The open items naming a model, and no agent, that no agent of their role switched on runs, so no launch
/// takes them (docs/repositories.md, 「An agent an item names」).
pub fn without_candidate(standing: &Standing) -> Vec<&Open> {
  standing
    .open
    .iter()
    .chain(&standing.paused)
    .filter(|item| {
      item
        .asked_model()
        .is_some_and(|model| standing.candidates(item.role(), model).next().is_none())
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
/// seconds and more, not told yet (`told` naming when each told one was), and the switch on; of the items naming
/// one agent, only the first in the queue's order — the highest priority, then the earliest place, then the
/// lowest number — and only once the next-offer seconds have passed since another of that agent's waiting items
/// was told, so a free agent is offered its next item rather than its whole series.
pub fn launches_due<'a>(
  standing: &'a Standing,
  seen: &BTreeMap<i64, i64>,
  told: &BTreeMap<i64, i64>,
  now: i64,
) -> Vec<&'a Open> {
  if standing.silenced {
    return vec![];
  }

  let awaiting = awaiting_launch(standing);
  let seats = seats(standing);
  let mut offered: BTreeSet<&String> = awaiting
    .iter()
    .filter_map(|item| Some((seats.get(&item.id)?, told.get(&item.id)?)))
    .filter(|(_, at)| now - **at < QUEUE_IDLE.next_offer_seconds * 1000)
    .map(|(agent, _)| agent)
    .collect();
  let mut due: Vec<&Open> = awaiting
    .into_iter()
    .filter(|item| !told.contains_key(&item.id))
    .filter(|item| seen.get(&item.id).is_some_and(|since| now - since >= QUEUE_IDLE.launch_seconds * 1000))
    .collect();

  due.sort_by_key(|item| item.place());
  due.retain(|item| seats.get(&item.id).is_none_or(|agent| offered.insert(agent)));
  due
}

/// The instruction an item awaiting a launch is queued with for its orderer: for an item naming a model, with
/// the agent `seat` names, the one a launch would take.
pub fn launch_instruction(item: &Open, seat: Option<&str>, orderer: &Orderer) -> Value {
  let number = item.id.to_string();
  let mut order = fill_marks(
    &QUEUE_IDLE.launch_order,
    &[
      ("item", &number),
      ("title", &item.title),
      ("worker", item.assignee.as_deref().unwrap_or(&JOB_NOTICES.any_worker)),
    ],
  );

  if let (Some(model), Some(agent)) = (item.asked_model(), seat) {
    order.push_str(&fill_marks(
      &QUEUE_IDLE.launch_seat,
      &[("agent", agent), ("role", item.role()), ("model", model)],
    ));
  }

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

/// A held item that names the items it waits on, as the pass reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Follower {
  pub id: i64,
  pub title: String,
  /// The labels naming what it waits on, each as `name=value`.
  pub label: String,
  /// The items it waits on, by their number.
  pub waits_on: Vec<i64>,
  /// The room it was asked in, where it names one itself.
  pub room: Option<i64>,
  /// The item it was raised from, where it names one.
  pub parent: Option<i64>,
}

impl Follower {
  /// The item as the orderer's resolver reads it.
  pub fn as_item(&self) -> Value {
    json!({ "id": self.id, "roomId": self.room, "raisedBy": self.parent })
  }
}

/// A job service item read as one that waits on others: none for a member that carries no number or no wait
/// label, or whose wait label has a part naming no item number. A label is the job service's own
/// comma-separated list of numbers, each with or without its `#`.
pub fn follower_of(item: &Value) -> Option<Follower> {
  let mut label = Vec::new();
  let mut waits_on = Vec::new();

  for name in &JOB_NOTICES.wait_labels {
    let value = match &item["labels"][name.as_str()] {
      Value::String(value) => value.clone(),
      Value::Number(value) => value.to_string(),
      _ => continue,
    };

    for part in value.split(',') {
      waits_on.push(part.trim().trim_start_matches('#').parse::<i64>().ok().filter(|id| *id > 0)?);
    }

    label.push(format!("{name}={value}"));
  }

  if waits_on.is_empty() {
    return None;
  }

  Some(Follower {
    id: item["id"].as_i64()?,
    title: item["title"].as_str().unwrap_or_default().to_owned(),
    label: label.join(" and "),
    waits_on,
    room: room_of(item),
    parent: parent_of(item),
  })
}

/// Whether an item waited on has ended: it stands `done`. An item whose commit waits to go live stays leased
/// until that commit is live, so `done` is already past its deployment, whatever its close label still reads.
pub fn wait_over(item: &Value) -> bool {
  item["state"] == "done"
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

/// The items awaiting a launch owed to their orderer while they wait, and the rows kept for them brought up to
/// date: a waiting item first seen now marked, and both the mark and the standing notice of an item no longer
/// waiting, or of every item while the switch is off, let go, so it is told again once its checkout and its agent
/// are free again.
pub async fn tell_launches(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  standing: &Standing,
  now: i64,
) -> Result<(), String> {
  // A deployment that carries no receiver has no supervisor to tell.
  if Receiver::of(vars).is_none() {
    return Ok(());
  }

  let service = JobService::of(vars);
  let fault = |why: Refusal| why.detail;
  let mut seen = marks(db, SEEN_PREFIX).await.map_err(fault)?;
  let awaiting: BTreeSet<i64> = awaiting_launch(standing).iter().map(|item| item.id).collect();

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

  let mut told = BTreeMap::new();

  for (item, owed) in outbox::owed(db, LAUNCH_OWED).await? {
    match item.parse::<i64>() {
      Ok(id) if awaiting.contains(&id) && !standing.silenced => {
        told.insert(id, owed.owed_at);
      }
      _ => outbox::let_go_as_read(db, vars, &format!("{LAUNCH_OWED}{item}"), &owed).await?,
    }
  }

  let seats = seats(standing);

  for item in launches_due(standing, &seen, &told, now) {
    let orderer = orderer(service.as_ref(), &item.as_item()).await?;
    let seat = seats.get(&item.id).map(String::as_str);
    let notice = Notice::instruction(launch_instruction(item, seat, &orderer), Keep::Standing);

    outbox::owe(db, vars, notice).await?;
    println!(
      "queue: item #{} waited on {} for a launch, owed to its orderer",
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
  if Receiver::of(vars).is_none() {
    return Ok(());
  }

  let service = JobService::of(vars);
  let fault = |why: Refusal| why.detail;
  let paused: BTreeSet<i64> = standing.paused.iter().map(|item| item.id).collect();
  let told = kept_marks(db, PAUSED_TOLD_PREFIX, &paused).await?;

  for item in standing.paused.iter().filter(|item| !told.contains(&item.id)) {
    let orderer = orderer(service.as_ref(), &item.as_item()).await?;

    let notice = job_notices::paused(item.id, &item.title, &item.queue, &orderer);

    outbox::owe(db, vars, Notice::instruction(notice, Keep::Once)).await?;
    settings::set(db, &format!("{PAUSED_TOLD_PREFIX}{}", item.id), &now.to_string())
      .await
      .map_err(fault)?;
    println!("queue: item #{} waits on its paused queue {}, owed to its orderer", item.id, item.queue);
  }

  let stuck = unlaunchable(standing);
  let standing_ids: BTreeSet<i64> = stuck.iter().map(|item| item.id).collect();
  let told = kept_marks(db, UNLAUNCHABLE_TOLD_PREFIX, &standing_ids).await?;

  for item in stuck.into_iter().filter(|item| !told.contains(&item.id)) {
    let assignee = item.assignee.as_deref().unwrap_or_default();
    let orderer = orderer(service.as_ref(), &item.as_item()).await?;

    let notice = job_notices::unlaunchable(item.id, &item.title, assignee, &orderer);

    outbox::owe(db, vars, Notice::instruction(notice, Keep::Once)).await?;
    settings::set(db, &format!("{UNLAUNCHABLE_TOLD_PREFIX}{}", item.id), &now.to_string())
      .await
      .map_err(fault)?;
    println!(
      "queue: item #{} names {assignee}, which no checkout is of, owed to its orderer",
      item.id
    );
  }

  let lacking = without_candidate(standing);
  let lacking_ids: BTreeSet<i64> = lacking.iter().map(|item| item.id).collect();

  for id in settings::named(db, NO_CANDIDATE_TOLD_PREFIX).await.map_err(fault)? {
    if id.parse::<i64>().is_ok_and(|id| !lacking_ids.contains(&id)) {
      settings::forget(db, &format!("{NO_CANDIDATE_TOLD_PREFIX}{id}")).await.map_err(fault)?;
    }
  }

  for item in lacking {
    let (role, model) = (item.role(), item.asked_model().unwrap_or_default());
    let agents = crate::role_agents::roads_held(role, &standing.agents);

    tell_no_candidate(db, vars, item.id, &item.as_item(), role, model, &agents).await?;
  }

  Ok(())
}

/// An item whose model no agent of its role runs told to its orderer, once for that role and model
/// (docs/agent-control.md, 「An item's change is told to its orderer」): the role and the model told are kept by
/// the item, so the same pair is not told again and another one is. `item` carries its title and what its
/// orderer is found by. Answers whether it was told now.
pub async fn tell_no_candidate(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  id: i64,
  item: &Value,
  role: &str,
  model: &str,
  agents: &str,
) -> Result<bool, String> {
  if Receiver::of(vars).is_none() {
    return Ok(false);
  }

  let fault = |why: Refusal| why.detail;
  let key = format!("{NO_CANDIDATE_TOLD_PREFIX}{id}");
  let pair = format!("{role} {model}");

  if settings::get(db, &key).await.map_err(fault)?.as_deref() == Some(pair.as_str()) {
    return Ok(false);
  }

  let mut item = item.clone();

  item["id"] = json!(id);

  let orderer = orderer(JobService::of(vars).as_ref(), &item).await?;
  let title = item["title"].as_str().unwrap_or_default();

  let notice = job_notices::no_candidate(id, title, role, model, agents, &orderer);

  outbox::owe(db, vars, Notice::instruction(notice, Keep::Once)).await?;
  settings::set(db, &key, &pair).await.map_err(fault)?;
  println!(
    "queue: item #{id} names model={model} for the {role} role, which no agent runs, owed to its orderer"
  );

  Ok(true)
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
  if Receiver::of(vars).is_none() {
    return Ok(false);
  }

  let fault = |why: Refusal| why.detail;
  let key = format!("{AGENT_TOLD_PREFIX}{id}");

  if settings::get(db, &key).await.map_err(fault)?.as_deref() == Some(label) {
    return Ok(false);
  }

  let mut item = item.clone();

  item["id"] = json!(id);

  let orderer = orderer(JobService::of(vars).as_ref(), &item).await?;
  let title = item["title"].as_str().unwrap_or_default();

  let notice = job_notices::agent_refused(id, title, label, why, &orderer);

  outbox::owe(db, vars, Notice::instruction(notice, Keep::Once)).await?;
  settings::set(db, &key, label).await.map_err(fault)?;
  println!("queue: item #{id} names agent={label}, which no job can run on, owed to its orderer");

  Ok(true)
}

/// The mark of an item's refused `agent` label let go, as a job of the item is launched.
pub async fn agent_told_let_go(db: &DatabaseConnection, id: i64) {
  if let Err(why) = settings::forget(db, &format!("{AGENT_TOLD_PREFIX}{id}")).await {
    eprintln!("queue: the told mark of item #{id}'s agent was not let go: {}", why.detail);
  }
}

/// The mark of an item's model told as having no candidate let go, as a job of the item is launched.
pub async fn no_candidate_told_let_go(db: &DatabaseConnection, id: i64) {
  if let Err(why) = settings::forget(db, &format!("{NO_CANDIDATE_TOLD_PREFIX}{id}")).await {
    eprintln!("queue: the told mark of item #{id}'s model was not let go: {}", why.detail);
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
  let mut carrying: Vec<String> = all(
    db,
    r#"SELECT DISTINCT "item_agent" FROM "jobs"
       WHERE ("state" IN ('starting', 'running') OR ("state" = 'reviewing' AND NOT "review_parked"))
         AND "item_agent" IS NOT NULL AND NOT "machine_turn""#,
    vec![],
  )
  .await?
  .iter()
  .filter_map(|row| row.try_get::<String>("", "item_agent").ok())
  .collect();

  carrying.extend(crate::role_agents::busy_any(db).await?);

  let agents = crate::role_agents::every(db).await?;

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
    carrying,
    agents,
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
/// Whether an open item's close already merged its commit, its label `close-phase` reading `waiting-batch` or an
/// older close's `waiting-live`: its work stands on main and only its repository's deployment remains, so it is no
/// work a checkout could take, and it is told only as waiting to go live.
pub fn merged_awaiting_deploy(item: &Value) -> bool {
  let phase = &item["labels"]["close-phase"];

  phase == "waiting-batch" || phase == "waiting-live"
}

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
      .filter(|item| !merged_awaiting_deploy(item))
      .filter_map(|item| {
        let queue = item["queueName"].as_str().unwrap_or_default().to_owned();

        Some(Open {
          id: item["id"].as_i64()?,
          title: item["title"].as_str().unwrap_or_default().to_owned(),
          assignee: item["assigneeName"].as_str().filter(|name| !name.is_empty()).map(str::to_owned),
          agent: named_by(&item["labels"], AGENT_LABEL).map(str::to_owned),
          model: named_by(&item["labels"], MODEL_LABEL).map(str::to_owned),
          role: named_by(&item["labels"], ROLE_LABEL).map(str::to_owned),
          priority: item["priority"].as_i64().unwrap_or_default(),
          position: item["position"].as_i64().unwrap_or_default(),
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
  let (Some(service), Some(_)) = (JobService::of(vars), Receiver::of(vars)) else {
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

    outbox::owe(db, vars, Notice::instruction(approval_instruction(draft, &orderer), Keep::Once)).await?;
    settings::set(db, &format!("{DRAFT_TOLD_PREFIX}{}", draft.id), &crate::now_ms().to_string())
      .await
      .map_err(fault)?;
    println!("queue: draft #{} awaits approval, owed to its orderer", draft.id);
  }

  Ok(())
}

/// Each held item every item it waits on has ended told to its orderer, once while it stays held: the told rows
/// of items no longer held, or no longer naming what they wait on, let go, and a row kept for each told now. An
/// item waited on that the job service does not answer for ends no wait.
pub async fn tell_waits(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<(), String> {
  let (Some(service), Some(_)) = (JobService::of(vars), Receiver::of(vars)) else {
    return Ok(());
  };
  let fault = |why: Refusal| why.detail;
  let followers: Vec<Follower> = every_page(&service, "/items", &[("state", "held")])
    .await?
    .iter()
    .filter_map(follower_of)
    .collect();
  let standing: BTreeSet<i64> = followers.iter().map(|follower| follower.id).collect();
  let told = kept_marks(db, WAIT_TOLD_PREFIX, &standing).await?;
  let mut read: BTreeMap<i64, Option<Value>> = BTreeMap::new();

  for follower in followers.iter().filter(|follower| !told.contains(&follower.id)) {
    let mut waited = Vec::new();

    for id in &follower.waits_on {
      if !read.contains_key(id) {
        read.insert(*id, read_job(&service, &format!("/items/{id}"), &[]).await.ok());
      }

      waited.push(read[id].clone());
    }

    let Some(waited) = waited.into_iter().collect::<Option<Vec<Value>>>() else { continue };

    if !waited.iter().all(wait_over) {
      continue;
    }

    let orderer = service.orderer_of(&follower.as_item()).await?;

    let notice = job_notices::wait_ended(follower.id, &follower.title, &follower.label, &waited, &orderer);

    outbox::owe(db, vars, Notice::instruction(notice, Keep::Once)).await?;
    settings::set(db, &format!("{WAIT_TOLD_PREFIX}{}", follower.id), &crate::now_ms().to_string())
      .await
      .map_err(fault)?;
    println!("queue: held item #{} waits on nothing more, owed to its orderer", follower.id);
  }

  Ok(())
}

/// One pass of the alarm: the queue read; each item awaiting a launch, each open item that cannot go, each
/// draft awaiting approval and each held item whose wait has ended told to its orderer once; the job service's
/// events told; and an idle queue told to the owner once, the moments kept.
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

  if let Err(why) = tell_waits(db, vars).await {
    eprintln!("queue: a held item whose wait has ended was not told: {why}");
  }

  if let Err(why) = crate::watches::job_events::pass(db, vars).await {
    eprintln!("queue: the job service's events were not told: {why}");
  }

  // Words kept for a running job while its turn could not take them are offered to that turn again.
  crate::job_words::offer_kept(db).await;

  if !idle(&standing, now) {
    return Ok(());
  }

  if Receiver::of(vars).is_none() {
    return Ok(());
  }

  outbox::owe(db, vars, Notice::owner(now, notice(&standing))).await?;
  settings::set(db, TOLD_KEY, &now.to_string()).await.map_err(|why| why.detail)?;
  println!(
    "queue: idle for {} minutes with {} items a free checkout could take, owed to the owner",
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
