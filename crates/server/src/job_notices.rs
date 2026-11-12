//! What an item's orderer and the owner are told of a change in the queue (docs/agent-control.md, 「An item's
//! change is told to its orderer」): each instruction an order under the sort `job` in the orderer's room, with
//! a kind and an event of its row and the item's or the question's number; each owner notice a code with its
//! fields; and the receiver they are queued with.

use std::collections::BTreeMap;

use policy::fill_marks;
use policy::job_notices::JOB_NOTICES;
use policy::machine_turn::MACHINE_TURN;
use serde_json::{Value, json};

use crate::job_end::{address, token};
use crate::queue::orderer::{JobService, Orderer};

/// The receiver as the queue's notices reach it: its address and token.
#[derive(Clone, Debug)]
pub struct Receiver {
  url: String,
  token: String,
}

impl Receiver {
  /// The receiver the deployment names; none where it names none, or the ground is a test's.
  pub fn of(vars: &BTreeMap<String, String>) -> Option<Self> {
    let (Some(url), Some(token)) = (
      address(vars, "BMO_RECEIVER_URL"),
      token(vars, "BMO_RECEIVER_TOKEN", "BMO_RECEIVER_TOKEN_FILE"),
    ) else {
      return None;
    };

    Some(Self { url, token })
  }

  async fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
    let answer = reqwest::Client::new()
      .post(format!("{}{path}", self.url))
      .bearer_auth(&self.token)
      .json(body)
      .send()
      .await
      .map_err(|why| why.to_string())?;

    if !answer.status().is_success() {
      return Err(format!("the receiver answered {} to {path}", answer.status().as_u16()));
    }

    Ok(answer.json().await.unwrap_or(Value::Null))
  }

  /// An instruction queued: the id the receiver answers it with, where it answers one.
  pub async fn instruct(&self, body: &Value) -> Result<Option<i64>, String> {
    Ok(self.post("/instructions", body).await?["id"].as_i64())
  }

  /// An owner notice queued.
  pub async fn notify_owner(&self, body: &Value) -> Result<(), String> {
    self.post("/owner/notices", body).await.map(|_| ())
  }

  /// A pending instruction withdrawn; one already handed or gone is no refusal.
  pub async fn withdraw(&self, id: i64) -> Result<(), String> {
    let answer = reqwest::Client::new()
      .delete(format!("{}/instructions/{id}", self.url))
      .bearer_auth(&self.token)
      .send()
      .await
      .map_err(|why| why.to_string())?;

    match answer.status() {
      status if status.is_success() || status == reqwest::StatusCode::NOT_FOUND => Ok(()),
      status => Err(format!("the receiver answered {} to the withdrawal", status.as_u16())),
    }
  }
}

/// An instruction for an orderer: an order under the sort `job` in its room, the owner's private room where it
/// names none, of the row `event` about `number`, so a newer one of the same kind replaces one still pending.
pub fn instruction(event: &str, number: i64, orderer: &Orderer, summary: String, order: String) -> Value {
  json!({
    "room": orderer.room,
    "said": { "order": order },
    "agentName": Value::Null,
    "sort": "job",
    "kind": format!("{event}:{number}"),
    "summary": summary,
    "events": [format!("{event}:#{number}")],
    "run": Value::Null,
  })
}

fn title_of(item: &Value) -> &str {
  item["title"].as_str().unwrap_or_default()
}

fn id_of(item: &Value) -> i64 {
  item["id"].as_i64().unwrap_or_default()
}

/// An open item in a queue that gives no lease.
pub fn paused(item: i64, title: &str, queue: &str, orderer: &Orderer) -> Value {
  let number = item.to_string();
  let order =
    fill_marks(&JOB_NOTICES.paused_order, &[("item", &number), ("title", title), ("queue", queue)]);

  instruction(
    "item-queue-paused",
    item,
    orderer,
    format!("Item #{item} waits on its paused queue"),
    order,
  )
}

/// An open item whose assignee no checkout agency holds is of.
pub fn unlaunchable(item: i64, title: &str, assignee: &str, orderer: &Orderer) -> Value {
  let number = item.to_string();
  let order = fill_marks(
    &JOB_NOTICES.unlaunchable_order,
    &[("item", &number), ("title", title), ("assignee", assignee)],
  );

  instruction("item-unlaunchable", item, orderer, format!("Item #{item} names no repository"), order)
}

/// An item whose launch was refused for its `agent` label: none named, or one no job can run on.
pub fn agent_refused(item: i64, title: &str, label: &str, why: &str, orderer: &Orderer) -> Value {
  let number = item.to_string();
  let order = fill_marks(
    &JOB_NOTICES.agent_order,
    &[("item", &number), ("title", title), ("label", label), ("why", why)],
  );

  instruction(
    "item-unlaunchable",
    item,
    orderer,
    format!("Item #{item} names no agent a job can run on"),
    order,
  )
}

/// An item moved while a job of agency's still runs on it: held, dropped, merged or leased by another holder.
pub fn moved(item: &Value, job: i64, worker: &str, orderer: &Orderer) -> Value {
  let id = id_of(item);
  let number = id.to_string();
  let job = job.to_string();
  let state = moved_state(item);
  let merged = item["mergedInto"]
    .as_i64()
    .map(|into| format!(" into item #{into}"))
    .unwrap_or_default();
  let order = fill_marks(
    &JOB_NOTICES.moved_order,
    &[
      ("item", &number),
      ("title", title_of(item)),
      ("state", &state),
      ("merged", &merged),
      ("job", &job),
      ("worker", worker),
    ],
  );

  instruction(
    "item-moved-under-job",
    id,
    orderer,
    format!("Item #{id} moved under its running job"),
    order,
  )
}

/// The state a moved item is told in: its own, or leased by another holder where a lease of another stands.
pub fn moved_state(item: &Value) -> String {
  match item["state"].as_str().unwrap_or("gone") {
    "leased" | "open" => "leased by another holder".to_owned(),
    state => state.to_owned(),
  }
}

/// An item moved to done by a caller other than its orderer.
pub fn closed(item: &Value, by: &str, orderer: &Orderer) -> Value {
  let id = id_of(item);
  let number = id.to_string();
  let order =
    fill_marks(&JOB_NOTICES.closed_order, &[("item", &number), ("title", title_of(item)), ("by", by)]);

  instruction("item-closed", id, orderer, format!("Item #{id} was closed by {by}"), order)
}

/// A held item whose every item it waits on has ended, `label` the label naming them and `waited` each of
/// them as the job service answers it (docs/agent-control.md, 「An item's change is told to its orderer」).
pub fn wait_ended(item: i64, title: &str, label: &str, waited: &[Value], orderer: &Orderer) -> Value {
  let number = item.to_string();
  let waited = waited
    .iter()
    .map(|other| {
      let state = other["state"].as_str().unwrap_or_default();
      let state = match other["labels"][MACHINE_TURN.close_label.as_str()].as_str() {
        Some(phase) => format!("{state}, {} {phase}", MACHINE_TURN.close_label),
        None => state.to_owned(),
      };

      fill_marks(
        &JOB_NOTICES.waited_item,
        &[("item", &id_of(other).to_string()), ("title", title_of(other)), ("state", &state)],
      )
    })
    .collect::<Vec<_>>()
    .join("; ");
  let order = fill_marks(
    &JOB_NOTICES.wait_ended_order,
    &[("item", &number), ("title", title), ("label", label), ("waited", &waited)],
  );

  instruction("item-wait-ended", item, orderer, format!("Item #{item} waits on nothing more"), order)
}

/// A job's commit handed over to go live while its job waits on the deployment's outcome
/// (docs/agent-control.md, 「An item's change is told to its orderer」).
pub fn awaits_deploy(item: &Value, commit: &str, orderer: &Orderer) -> Value {
  let id = id_of(item);
  let number = id.to_string();
  let order = fill_marks(
    &JOB_NOTICES.deploy_order,
    &[
      ("item", &number),
      ("title", title_of(item)),
      ("repository", item["assigneeName"].as_str().unwrap_or("the host")),
      ("commit", commit),
    ],
  );

  instruction(
    "item-awaits-deploy",
    id,
    orderer,
    format!("Item #{id}'s commit {commit} waits to go live"),
    order,
  )
}

/// A run's question, for the orderer to put to the owner.
pub fn question(question: &Value, orderer: &Orderer) -> Value {
  let id = id_of(question);
  let number = id.to_string();
  let item = question["itemId"]
    .as_i64()
    .map(|item| item.to_string())
    .unwrap_or_else(|| "none".to_owned());
  let choices = choices_of(question);
  let order = fill_marks(
    &JOB_NOTICES.question_order,
    &[
      ("question", &number),
      ("item", &item),
      ("text", question["text"].as_str().unwrap_or_default()),
      ("choices", if choices.is_empty() { "an open answer" } else { &choices }),
    ],
  );

  instruction("question-awaits-answer", id, orderer, format!("Question #{id} awaits the owner"), order)
}

fn choices_of(question: &Value) -> String {
  question["choices"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|choice| {
      choice
        .as_str()
        .map(str::to_owned)
        .or_else(|| choice["label"].as_str().map(str::to_owned))
    })
    .collect::<Vec<_>>()
    .join(" / ")
}

/// A job waiting for a road or a slot, with why.
pub fn waiting(item: i64, worker: &str, why: &str, orderer: &Orderer) -> Value {
  let number = item.to_string();
  let order =
    fill_marks(&JOB_NOTICES.waiting_order, &[("item", &number), ("worker", worker), ("why", why)]);

  instruction("job-waiting", item, orderer, format!("The job of item #{item} waits"), order)
}

/// An ended job's item still leased, not closed.
pub fn awaits_close(item: i64, lease: i64, orderer: &Orderer) -> Value {
  let number = item.to_string();
  let lease = lease.to_string();
  let order = fill_marks(&JOB_NOTICES.close_order, &[("item", &number), ("lease", &lease)]);

  instruction("job-awaits-close", item, orderer, format!("Item #{item} awaits its close"), order)
}

/// The owner notice of a question still open past its due time.
pub fn question_due(question: &Value) -> Value {
  json!({
    "code": JOB_NOTICES.question_due_code,
    "questionId": question["id"],
    "itemId": question["itemId"],
    "text": question["text"],
    "choices": question["choices"],
    "askerName": question["askerName"],
    "dueAt": question["dueAt"],
  })
}

/// Whether a job's end is a failure the owner is told of as well: its outcome `failed` or `blocked`, its
/// orderer a room other than the owner's private room.
pub fn owner_hears_failure(member: &Value, orderer: &Orderer) -> bool {
  orderer.room.is_some() && matches!(member["outcome"].as_str(), Some("failed" | "blocked"))
}

/// The owner notice of a job that failed in a room other than his own.
pub fn failed(member: &Value, room: Option<i64>) -> Value {
  json!({
    "code": JOB_NOTICES.failed_code,
    "jobId": member["id"],
    "itemId": member["itemId"],
    "worker": member["workerName"],
    "outcome": member["outcome"],
    "room": room,
  })
}

/// A job's wait told to its item's orderer as it begins (「A job's wait is told as it begins」); a deployment
/// naming no job service or no receiver tells nothing, and a call that fails is said and fails nothing.
pub async fn tell_waiting(vars: &BTreeMap<String, String>, item: i64, worker: &str, why: &str) {
  let (Some(service), Some(receiver)) = (JobService::of(vars), Receiver::of(vars)) else {
    return;
  };

  if item <= 0 {
    return;
  }

  let told = async {
    let (_, orderer) = service.orderer_of_id(item).await?;

    receiver.instruct(&waiting(item, worker, why, &orderer)).await
  };

  match told.await {
    Ok(_) => println!("job of item #{item}: its wait told to its orderer"),
    Err(why) => eprintln!("job of item #{item}: its wait was not told: {why}"),
  }
}
