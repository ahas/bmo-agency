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

/// A job's end delivered: its lease given back to the job service, and its report queued for the room's
/// chat agent. Each call stands on its own, and a call that fails is said and never fails the job.
pub async fn delivered(ground: &Ground, member: &Value) {
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
    eprintln!("job {}: its end was not told to the room: no receiver is named", id_of(member));

    return;
  };
  // The job's end is queued for the agent that filed the item and no other (the receiver's `docs/api.md`,
  // 「Instructions」), so its requester is read from the job service where the item names one.
  let requester = requester(vars, member).await;

  if let Err(why) = post_instruction(&url, &token, &instruction_body(member, requester.as_deref())).await {
    eprintln!("job {}: its end was not told to the room: {why}", id_of(member));
  }
}

/// The agent that filed the item, read from the job service: the row's `requesterName`, none where the item
/// names the job service no requester or cannot be read.
async fn requester(vars: &BTreeMap<String, String>, member: &Value) -> Option<String> {
  let item = member["itemId"].as_i64().filter(|id| *id > 0)?;
  let (Some(url), Some(token)) =
    (address(vars, "BMO_JOB_URL"), token(vars, "BMO_JOB_TOKEN", "BMO_JOB_TOKEN_FILE"))
  else {
    return None;
  };
  let answer = reqwest::Client::new()
    .get(format!("{url}/items/{item}"))
    .bearer_auth(token)
    .timeout(ANSWER_WITHIN)
    .send()
    .await;

  let said: Value = match answer {
    Ok(answer) if answer.status().is_success() => answer.json().await.ok()?,
    Ok(answer) => {
      eprintln!("job {item}: its requester was not read: {}", answer.status());

      return None;
    }
    Err(why) => {
      eprintln!("job {item}: its requester was not read: {why}");

      return None;
    }
  };

  said["requesterName"]
    .as_str()
    .map(str::trim)
    .filter(|name| !name.is_empty())
    .map(str::to_owned)
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
     Relay its end in a few lines in this room, in the asker's language: the outcome, the commit or \
     artefact it left, and one line of what it left out, then what the report could not say. The report \
     stays whole on the item; never paste it into the room.",
    report.unwrap_or("The run left no report; its end is all there is to tell.")
  );

  text
}

/// The instruction queued for the room: the order, the item it names as news of the queue, the agent that
/// filed the item where the job service named one, the room the job answers where it names one, and the run
/// it was. It is a job's status notice, an order under the sort `job`, so the receiver queues it for the
/// requester rather than for whoever answers the room.
pub fn instruction_body(member: &Value, requester: Option<&str>) -> Value {
  let item = member["itemId"].as_i64().unwrap_or_default();
  let room = member["roomId"].as_i64().filter(|room| *room != 0);
  let run = member["runId"].as_i64().filter(|run| *run != 0);
  json!({
    "room": room,
    "said": { "order": order(member) },
    "agentName": requester,
    "sort": "job",
    "kind": format!("job-ended:{item}"),
    "summary": format!("The job of item #{item} ended"),
    "events": [format!("job-ended:#{item}")],
    "run": run,
  })
}
