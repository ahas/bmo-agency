//! A job's end giving back its slot, reaching its orderer, and its lease kept for the item's close or given
//! back (docs/api.md, 「assignments and jobs」): the slot every request of the job's run held, given back
//! whatever its lease is bound to; the report a run left, queued for the item's orderer through the receiver's
//! instruction route, and a failure in another room told to the owner as well; the job service's lease on the
//! item kept standing where the job ended, so the item stays leased until its close ends the lease, and given
//! back where it was stopped, so the item is open again for a rerun. A run's end is written where the run
//! ends, so neither the room's session being replaced nor the harness that launched the job being gone loses
//! it: the receiver keeps the row until a turn takes it.

use std::collections::BTreeMap;

use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use store::run;

use crate::order_run::dispatch;
use crate::places::Ground;

fn setting<'a>(vars: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
  vars.get(name).map(String::as_str).map(str::trim).filter(|value| !value.is_empty())
}

/// Whether these settings stand a test's server: a ground a test stands reaches no peer, whatever its
/// environment names, so a test never posts into a room or a queue a person holds.
pub(crate) fn test_ground(vars: &BTreeMap<String, String>) -> bool {
  setting(vars, "BMO_TEST_GROUND").is_some_and(|set| set == "1")
}

/// The token a peer is called with: a value the deployment names, or the file it names.
pub(crate) fn token(vars: &BTreeMap<String, String>, variable: &str, file: &str) -> Option<String> {
  if test_ground(vars) {
    return None;
  }

  if let Some(token) = setting(vars, variable) {
    return Some(token.to_owned());
  }

  setting(vars, file)
    .and_then(|path| std::fs::read_to_string(path).ok())
    .map(|token| token.trim().to_owned())
    .filter(|token| !token.is_empty())
}

pub(crate) fn address(vars: &BTreeMap<String, String>, variable: &str) -> Option<String> {
  if test_ground(vars) {
    eprintln!("{variable}: a test ground reaches no peer; the call is not made");

    return None;
  }

  setting(vars, variable).map(|url| url.trim_end_matches('/').to_owned())
}

/// The slot a job's run holds given back as the job's end is written (docs/role-slots.md, 「The line」): every
/// request asked under the job's owner given back as that owner's release gives it back, kept with the reason
/// the end carries, whatever its lease is bound to. Every write of a job's end calls it, so no ended job keeps
/// a slot of its role; a request its holder already gave back is left as it stands, and a store that would
/// not take it is said and never fails the end. The job's own session is started for each turn and ended with
/// it, so no role session is left for this release to end.
pub async fn slot_given_back(db: &DatabaseConnection, events: &crate::events::Bus, id: i64, why: &str) {
  let owner = format!("{}{id}", dispatch::JOB_OWNER);
  let why: String = format!("its job ended: {}", why.trim()).chars().take(REASON_CHARS).collect();

  match dispatch::release_owner_for(db, events, &owner, &why).await {
    Ok(done) if done.released + done.cancelled + done.reservations > 0 => {
      println!(
        "job {id}: its slot given back with its end: {} let in, {} waiting, {} reservations",
        done.released, done.cancelled, done.reservations
      );
    }
    Ok(_) => {}
    Err(fault) => eprintln!("job {id}: its slot was not given back with its end: {fault}"),
  }
}

// The most characters of an end's reason a given-back request keeps, within what the line keeps of one.
const REASON_CHARS: usize = 400;

/// Whether a job's end keeps its item's lease standing for the item's close: a job that ended, holding a
/// lease. A stopped job's item is open again for a rerun.
pub fn keeps_lease(member: &Value) -> bool {
  member["state"].as_str() == Some("ended") && member["leaseId"].as_i64().is_some_and(|lease| lease > 0)
}

/// A job's end delivered: its lease kept for the item's close where it ended, which the lease pass renews
/// until the close ends it, or given back to the job service where it was stopped; and its report queued for
/// the room's supervisor. Each call stands on its own, and a call that fails is said and never fails the job.
pub async fn delivered(ground: &Ground, member: &Value) {
  if keeps_lease(member)
    && let Err(why) = run(
      &ground.db,
      r#"UPDATE "jobs" SET "lease_kept" = true WHERE "id" = $1"#,
      vec![id_of(member).into()],
    )
    .await
  {
    eprintln!("job {}: its lease was not kept for the item's close: {why:?}", id_of(member));
  }

  if ground.testing() {
    eprintln!("job {}: its end was not delivered: the ground is a test's", id_of(member));

    return;
  }

  delivered_with(ground.vars(), member).await;
}

/// The same, kept from the settings the deployment names, so a test reaches it with a map of its own.
pub async fn delivered_with(vars: &BTreeMap<String, String>, member: &Value) {
  if test_ground(vars) {
    eprintln!("job {}: its end was not delivered: the ground is a test's", id_of(member));

    return;
  }

  if !keeps_lease(member) {
    return_lease(vars, member).await;
  }

  tell_room(vars, member).await;
}

/// The lease a stopped job's item was held by, given back, so the item is open again for a rerun. A lease
/// already gone is no failure.
async fn return_lease(vars: &BTreeMap<String, String>, member: &Value) {
  let lease = member["leaseId"].as_i64().unwrap_or_default();

  if lease <= 0 {
    return;
  }

  let (Some(url), Some(token)) =
    (address(vars, "BMO_JOB_URL"), token(vars, "BMO_JOB_TOKEN", "BMO_JOB_TOKEN_FILE"))
  else {
    eprintln!("job {}: its lease {lease} was not returned: no job service is named", id_of(member));
    return;
  };

  let answer = reqwest::Client::new()
    .delete(format!("{url}/leases/{lease}"))
    .bearer_auth(token)
    .send()
    .await;

  match answer {
    Ok(answer) if answer.status().is_success() => {}
    Ok(answer) if answer.status() == reqwest::StatusCode::NOT_FOUND => {}
    Ok(answer) => {
      eprintln!("job {}: its lease {lease} was not returned: {}", id_of(member), answer.status())
    }
    Err(why) => eprintln!("job {}: its lease {lease} was not returned: {why}", id_of(member)),
  }
}

/// The job's report queued for the item's orderer: an instruction that waits in the receiver for the next
/// turn, naming the item and carrying the report whole; and a failure in a room other than the owner's own
/// told to the owner as well. No address is a deployment that carries no receiver, and then no one is told.
async fn tell_room(vars: &BTreeMap<String, String>, member: &Value) {
  let (Some(url), Some(token)) = (
    address(vars, "BMO_RECEIVER_URL"),
    token(vars, "BMO_RECEIVER_TOKEN", "BMO_RECEIVER_TOKEN_FILE"),
  ) else {
    eprintln!("job {}: its end was not told to the room: no receiver is named", id_of(member));

    return;
  };
  let orderer = orderer(vars, member).await;
  let body = instruction_body(member, &orderer);

  if let Err(why) = post_instruction(&url, &token, &body).await {
    eprintln!("job {}: its end was not told to the room: {why}", id_of(member));
  }

  if crate::job_notices::owner_hears_failure(member, &orderer)
    && let Some(receiver) = crate::job_notices::Receiver::of(vars)
    && let Err(why) = receiver.notify_owner(&crate::job_notices::failed(member, orderer.room)).await
  {
    eprintln!("job {}: its failure was not told to the owner: {why}", id_of(member));
  }
}

/// The orderer a job's end is told to: the item's, up the chain it was raised from, read from the job service;
/// where the service cannot be read or the chain finds no room, the room the job was launched for, with the
/// item's requester where it was read.
pub async fn orderer(vars: &BTreeMap<String, String>, member: &Value) -> crate::queue::orderer::Orderer {
  let launched = member["roomId"].as_i64().filter(|room| *room != 0);
  let found = match (crate::queue::orderer::JobService::of(vars), member["itemId"].as_i64()) {
    (Some(service), Some(item)) if item > 0 => match service.orderer_of_id(item).await {
      Ok((_, orderer)) => orderer,
      Err(why) => {
        eprintln!("job {}: its item's orderer was not read: {why}", id_of(member));
        crate::queue::orderer::Orderer::default()
      }
    },
    _ => crate::queue::orderer::Orderer::default(),
  };

  crate::queue::orderer::Orderer {
    room: found.room.or(launched),
    requester: found.requester,
  }
}

// One instruction handed to the receiver's instruction route.
async fn post_instruction(url: &str, token: &str, body: &Value) -> Result<(), String> {
  let answer = reqwest::Client::new()
    .post(format!("{url}/instructions"))
    .bearer_auth(token)
    .json(body)
    .send()
    .await;

  match answer {
    Ok(answer) if answer.status().is_success() => Ok(()),
    Ok(answer) => Err(answer.status().to_string()),
    Err(why) => Err(why.to_string()),
  }
}

/// An instruction queued for a room's session through the receiver's instruction route, with the address and
/// token the deployment names; a deployment that carries no receiver queues nothing.
pub async fn instruct(vars: &BTreeMap<String, String>, body: &Value) -> Result<(), String> {
  if test_ground(vars) {
    eprintln!("an instruction was not queued: a test ground reaches no receiver");

    return Err("a test ground reaches no receiver".to_owned());
  }

  let (Some(url), Some(token)) = (
    address(vars, "BMO_RECEIVER_URL"),
    token(vars, "BMO_RECEIVER_TOKEN", "BMO_RECEIVER_TOKEN_FILE"),
  ) else {
    eprintln!("no receiver is named; no instruction was queued");

    return Err("no receiver is named".to_owned());
  };

  post_instruction(&url, &token, body).await
}

fn id_of(member: &Value) -> i64 {
  member["id"].as_i64().unwrap_or_default()
}

/// What the room's supervisor is told: an order carrying the item, the worker, the state and the report;
/// where the job ended holding a lease, the lease that still stands on the item. The order carries no
/// instruction: the supervisor's manual says how to relay an end and close its item. A job that left no
/// report says so.
pub fn order(member: &Value) -> String {
  let item = member["itemId"].as_i64().unwrap_or_default();
  let worker = member["workerName"].as_str().unwrap_or_default();
  let state = member["state"].as_str().unwrap_or_default();
  let outcome = member["outcome"].as_str().map(str::trim).filter(|why| !why.is_empty());
  let report = member["report"].as_str().map(str::trim).filter(|said| !said.is_empty());
  let ended = match (state, outcome) {
    ("ended", None) => "ended".to_owned(),
    ("stopped", None) => "stopped".to_owned(),
    (state, Some(outcome)) if outcome == state => state.to_owned(),
    (state, Some(outcome)) => format!("{state}, its outcome {outcome}"),
    (state, None) => state.to_owned(),
  };
  let mut text = format!(
    "The job of item #{item} on worker {worker} {ended}.\n\nIts report, as the run left it:\n\n{}",
    report.unwrap_or("The run left no report; its end is all there is to tell.")
  );

  if keeps_lease(member) {
    let lease = member["leaseId"].as_i64().unwrap_or_default();

    text.push_str(&format!("\n\nThe item stays leased under lease {lease}: it is not closed."));
  }

  text
}

/// The instruction queued for the orderer: the order, the item it names as news of the queue, the orderer's
/// room, the owner's private room where it names none, the requester of the item that room was found on, and
/// the run it was. It is a job's status notice, an order under the sort `job`.
pub fn instruction_body(member: &Value, orderer: &crate::queue::orderer::Orderer) -> Value {
  let item = member["itemId"].as_i64().unwrap_or_default();
  let run = member["runId"].as_i64().filter(|run| *run != 0);
  json!({
    "room": orderer.room,
    "said": { "order": order(member) },
    "agentName": orderer.requester,
    "sort": "job",
    "kind": format!("job-ended:{item}"),
    "summary": format!("The job of item #{item} ended"),
    "events": [format!("job-ended:#{item}")],
    "run": run,
  })
}
