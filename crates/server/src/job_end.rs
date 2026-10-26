//! A job's end reaching the room and giving its lease back (docs/api.md, 「assignments and jobs」): the
//! report a run left, queued in the room's chat agent through the receiver's instruction route, and the
//! job service's lease on the item returned as the job ends. A run's end is written where the run ends, so
//! neither the room's session being replaced nor the harness that launched the job being gone loses it: the
//! receiver keeps the row until a turn takes it, and the item is open again the moment the run is over.
//!
//! Agency calls no downstream but for these two: the lease it was handed back, and the end told to the room
//! delivery that holds it (docs/api.md, 「Calls agency makes」).

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{Value, json};

use crate::places::Ground;

/// How long either call waits before the end stands on its own.
const ANSWER_WITHIN: Duration = Duration::from_secs(5);

fn setting<'a>(vars: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
  vars.get(name).map(String::as_str).map(str::trim).filter(|value| !value.is_empty())
}

/// The token a peer is called with: a value the deployment names, or the file it names.
fn token(vars: &BTreeMap<String, String>, variable: &str, file: &str) -> Option<String> {
  if let Some(token) = setting(vars, variable) {
    return Some(token.to_owned());
  }

  setting(vars, file)
    .and_then(|path| std::fs::read_to_string(path).ok())
    .map(|token| token.trim().to_owned())
    .filter(|token| !token.is_empty())
}

fn address(vars: &BTreeMap<String, String>, variable: &str) -> Option<String> {
  setting(vars, variable).map(|url| url.trim_end_matches('/').to_owned())
}

/// A job's end delivered: its lease given back to the job service, and its report queued for the room's
/// chat agent. Each call stands on its own, and a call that fails is said and never fails the job.
pub async fn delivered(ground: &Ground, member: &Value) {
  delivered_with(ground.vars(), member).await;
}

/// The same, kept from the settings the deployment names, so a test reaches it with a map of its own.
pub async fn delivered_with(vars: &BTreeMap<String, String>, member: &Value) {
  return_lease(vars, member).await;
  tell_room(vars, member).await;
}

/// The lease a job's item was held by, given back, so the item is open as the job ends whichever way it
/// ended. A lease already gone is no failure.
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
    .timeout(ANSWER_WITHIN)
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

/// The job's report queued for the room's chat agent: an instruction that waits in the receiver for the
/// next turn, naming the item and carrying the report whole. No address is a deployment that carries no
/// receiver, and then no room is told.
async fn tell_room(vars: &BTreeMap<String, String>, member: &Value) {
  let (Some(url), Some(token)) = (
    address(vars, "BMO_RECEIVER_URL"),
    token(vars, "BMO_RECEIVER_TOKEN", "BMO_RECEIVER_TOKEN_FILE"),
  ) else {
    return;
  };
  if let Err(why) = post_instruction(&url, &token, &instruction_body(member)).await {
    eprintln!("job {}: its end was not told to the room: {why}", id_of(member));
  }
}

// One instruction handed to the receiver's instruction route.
async fn post_instruction(url: &str, token: &str, body: &Value) -> Result<(), String> {
  let answer = reqwest::Client::new()
    .post(format!("{url}/instructions"))
    .bearer_auth(token)
    .json(body)
    .timeout(ANSWER_WITHIN)
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
  let (Some(url), Some(token)) = (
    address(vars, "BMO_RECEIVER_URL"),
    token(vars, "BMO_RECEIVER_TOKEN", "BMO_RECEIVER_TOKEN_FILE"),
  ) else {
    return Err("no receiver is named".to_owned());
  };

  post_instruction(&url, &token, body).await
}

fn id_of(member: &Value) -> i64 {
  member["id"].as_i64().unwrap_or_default()
}

/// What the room's chat agent is told: an order carrying the item, the worker and the report, and asking
/// for the briefing the chat agent's own manual describes. A job that left no report says so.
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
  let text = format!(
    "The job of item #{item} on worker {worker} {ended}.\n\nIts report, as the run left it:\n\n{}\n\n\
     Tell the owner this report in his language, whole: what finished, what changed and what is left, \
     keeping every figure, path and command as it stands, then say what the report could not.",
    report.unwrap_or("The run left no report; its end is all there is to tell.")
  );

  text
}

/// The instruction queued for the room: the order, the item it names as news of the queue, the room the
/// job answers where it names one, and the run it was.
pub fn instruction_body(member: &Value) -> Value {
  let item = member["itemId"].as_i64().unwrap_or_default();
  let room = member["roomId"].as_i64().filter(|room| *room != 0);
  let run = member["runId"].as_i64().filter(|run| *run != 0);
  json!({
    "room": room,
    "said": { "order": order(member) },
    "sort": "notice",
    "kind": format!("job-ended:{item}"),
    "summary": format!("The job of item #{item} ended"),
    "events": [format!("job-ended:#{item}")],
    "run": run,
  })
}
