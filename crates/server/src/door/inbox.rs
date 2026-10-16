//! One turn of agency's own loop standing in its agent's pane (docs/agent-control.md, 「What a resident
//! turn reads」): the loop stood on its command, the turn put into its inbox, and the loop's record read
//! until that turn has ended. The turn carries only its own variables; the key is the loop's, read from its
//! own environment, and nothing else of the caller's goes into the inbox.

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Duration;

use policy::providers::Resident;
use policy::resident_turn::RESIDENT_TURN;
use serde_json::{Value, json};

use super::place_run::{begin_run, end_run, run_variables};
use super::record::{ModelSpend, Reply};
use super::stand::{interrupt_program, stand_resident};
use super::surface::{Execution, Run, SurfaceEnding, SurfaceTurn, interrupt_of};
use super::turn::Watch;
use crate::lines::place::{Place, Program, command_digest, is_turn_variable};
use crate::now_ms;
use crate::places::Ground;

const RUN_NAME: &str = "BMO_RUN_NAME";
// The gateway address a turn's tap stands at, which travels with the turn rather than the loop.
const LOCAL_URL: &str = "BMO_DSH_LOCAL_URL";

/// The record the loop keeps in its agent's place.
pub fn record_path(place: &Path) -> PathBuf {
  place.join("loop.jsonl")
}

fn inbox_dir(place: &Path) -> PathBuf {
  place.join("inbox")
}

/// Whether a variable belongs to one turn rather than to the loop: the gateway address its tap stands
/// at, and the run's own variables. They travel with the turn through the inbox, so the loop's own
/// command — and so its identity — stays the same from one turn to the next.
fn is_loop_turn_variable(name: &str) -> bool {
  name == LOCAL_URL || is_turn_variable(name)
}

/// The lines of the loop's record.
pub fn read_record(place: &Path) -> Vec<Value> {
  record_of(place).1
}

// The lines of the loop's record as it wrote them and as they read, each one it has ended: what follows the
// last newline is a line still being written, left for the next look.
fn record_of(place: &Path) -> (Vec<String>, Vec<Value>) {
  let text = std::fs::read_to_string(record_path(place)).unwrap_or_default();
  let mut lines: Vec<&str> = text.split('\n').collect();

  lines.pop();
  lines
    .into_iter()
    .filter(|line| !line.trim().is_empty())
    .filter_map(|line| serde_json::from_str(line).ok().map(|read| (line.to_owned(), read)))
    .unzip()
}

fn kind(record: &Value) -> Option<&str> {
  record.get("kind").and_then(Value::as_str)
}

/// Where the lines of the record that belong to one turn stand: from its start to the next turn's start.
fn span_of(records: &[Value], id: &str) -> Option<Range<usize>> {
  let start = records.iter().position(|record| {
    kind(record) == Some("start") && record.get("id").and_then(Value::as_str) == Some(id)
  })?;
  let next = records[start + 1..].iter().position(|record| kind(record) == Some("start"));

  Some(start..next.map_or(records.len(), |next| start + 1 + next))
}

/// The lines of the record that belong to one turn.
fn lines_of<'a>(records: &'a [Value], id: &str) -> Option<&'a [Value]> {
  span_of(records, id).map(|span| &records[span])
}

/// A turn put into the inbox: written whole, under a name that sorts it after every turn put in
/// before it.
fn put_turn(place: &Path, turn: &Value, id: &str) -> std::io::Result<()> {
  let inbox = inbox_dir(place);

  agent_host::disk::private_dir(&inbox)?;

  let mut bytes = serde_json::to_vec(turn).map_err(std::io::Error::other)?;

  bytes.push(b'\n');
  agent_host::disk::publish_bytes(&inbox.join(format!("{:015}-{id}.json", now_ms())), &bytes)
}

/// Where a turn read out of the loop's record stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
  Working,
  Ended,
  Interrupted,
}

/// A turn of the loop out of the lines its record holds of it, as the provider the loop stands for
/// carried it. The loop runs one headless dsh per turn and takes nothing while it works, so nothing is
/// ever left queued; its tokens are what dsh's session log said, and no money or window, which nothing
/// on these roads reports.
pub fn read_turn(records: &[Value], run: &Run, provider: &str) -> (State, Option<String>, Reply) {
  let turn = records
    .iter()
    .rposition(|record| kind(record) == Some("start"))
    .map_or(records, |start| &records[start..]);
  let failed = turn.iter().rev().find(|record| kind(record) == Some("failed"));
  let answer = turn.iter().rev().find(|record| kind(record) == Some("answer"));
  let model = answer
    .and_then(|answer| answer.get("model").and_then(Value::as_str))
    .map_or_else(|| run.model.clone(), str::to_owned);
  // Any finite number the loop wrote counts, cut to its whole part, since a spend holds whole tokens.
  let tokens = |name: &str| {
    answer
      .and_then(|answer| answer.get("usage"))
      .and_then(|usage| usage.get(name))
      .and_then(Value::as_f64)
      .filter(|count| count.is_finite())
      .map_or(0, |count| count as i64)
  };
  // The share read from the cache counts toward the turn's tokens as the progress line shows it, and is
  // none where the upstream reported none.
  let (input, output, cache_read) =
    (tokens("inputTokens"), tokens("outputTokens"), tokens("cacheReadTokens"));
  let state = if turn.iter().any(|record| kind(record) == Some("interrupted")) {
    State::Interrupted
  } else if turn.iter().any(|record| matches!(kind(record), Some("end" | "failed"))) {
    State::Ended
  } else {
    State::Working
  };
  let error =
    failed.map(|failed| failed.get("text").and_then(Value::as_str).unwrap_or("dsh failed").to_owned());
  let text = if failed.is_none() {
    answer
      .and_then(|answer| answer.get("text").and_then(Value::as_str))
      .unwrap_or_default()
      .trim()
      .to_owned()
  } else {
    String::new()
  };

  (
    state,
    error,
    Reply {
      mode: provider.to_owned(),
      model: Some(model.clone()),
      effort: None,
      text,
      session_id: None,
      context_ratio: None,
      context_tokens: None,
      session_usage: None,
      windows: vec![],
      spend: if input + output + cache_read == 0 {
        vec![]
      } else {
        vec![ModelSpend {
          mode: provider.to_owned(),
          model,
          tokens: input + output + cache_read,
          input_tokens: input,
          output_tokens: output,
          cache_read_tokens: cache_read,
          cache_write_tokens: 0,
          cost: 0.0,
        }]
      },
      live_tasks: vec![],
      workflows: vec![],
      account: None,
      last_request_usage: None,
      request_usages: None,
    },
  )
}

/// One turn of the provider's loop, from standing it to its answer, the turn's lines of the loop's record
/// handed to the watch as they are written.
pub async fn turn<W: Watch>(
  ground: &Ground,
  provider: &str,
  resident: &Resident,
  turn: &SurfaceTurn,
  watch: &mut W,
) -> Result<SurfaceEnding, String> {
  let place = Place::of(&ground.roots.data, &turn.home.agent);
  let program = Program {
    identity: command_digest(&turn.command),
    command: turn.command.clone(),
    restart: None,
    opened: None,
  };
  // A loop's run names no effort of its own.
  let mut execution = {
    let mut asked = turn.clone();

    asked.run.effort = String::new();
    Execution::begin(ground, provider, &asked, None, &program.command.args).await?
  };

  execution.save(ground, turn).await;
  watch.opened(execution.row.id()).await;

  let ended = put(ground, provider, resident, turn, &place, &program, &mut execution, watch).await;

  let _ = end_run(&place.path, now_ms());
  execution.close(ground, turn, ended.is_err()).await;
  ended
}

#[allow(clippy::too_many_arguments)]
async fn put<W: Watch>(
  ground: &Ground,
  provider: &str,
  resident: &Resident,
  turn: &SurfaceTurn,
  place: &Place,
  program: &Program,
  execution: &mut Execution,
  watch: &mut W,
) -> Result<SurfaceEnding, String> {
  let every = turn.clocks.poll.unwrap_or(Duration::from_millis(RESIDENT_TURN.loop_poll_ms));
  let ready = turn.clocks.ready.unwrap_or(Duration::from_secs(RESIDENT_TURN.loop_ready_seconds));
  let settle = turn.clocks.settle.unwrap_or(Duration::from_millis(RESIDENT_TURN.settle_ms));
  let stood = match stand_resident(
    ground,
    place,
    &turn.home,
    program,
    &turn.run.line,
    ready,
    Duration::from_millis(250),
  )
  .await
  {
    Ok(stood) => stood,
    Err(fault) => {
      return Ok(SurfaceEnding::Error(format!(
        "the loop did not stand: {}: {}",
        fault.code, fault.message
      )));
    }
  };
  let pane = stood.pane;
  let started = place.running();
  let from = read_record(&place.path).len();
  // Only the variables that are this turn's travel with it, so nothing of the caller's environment —
  // the key least of all — is ever written into the inbox.
  let mut env: BTreeMap<String, String> = turn
    .run
    .env
    .iter()
    .filter(|(name, _)| is_loop_turn_variable(name) && *name != RUN_NAME)
    .map(|(name, value)| (name.clone(), value.clone()))
    .collect();

  env.insert(RUN_NAME.to_owned(), turn.name.clone());
  execution.set("paneId", json!(pane));
  execution.set(
    "resident",
    json!({
      "place": place.path,
      "record": record_path(&place.path),
      "from": from,
      "pid": started.as_ref().map(|running| running.pid),
      "birth": started.as_ref().and_then(|running| running.birth.clone()),
    }),
  );

  if execution.cancelled(ground, turn).await {
    execution.ending = "cancelled";

    return Ok(SurfaceEnding::Error("the turn was stopped before it went in".to_owned()));
  }

  let prompted_at = now_ms();
  let id = uuid::Uuid::new_v4().to_string();
  let mut run = turn.given.clone();

  run["env"] = json!(env);
  run["resident"] = Value::Null;

  let _ = begin_run(&place.path, &turn.name, &turn.title, prompted_at, run_variables(&env));
  put_turn(
    &place.path,
    &json!({
      "id": id,
      "name": turn.name,
      "title": turn.title,
      "timeoutMs": turn.timeout.map(|bound| bound.as_millis() as i64),
      "run": run,
    }),
    &id,
  )
  .map_err(|why| format!("the loop's inbox: {why}"))?;
  execution.set("state", json!("running"));
  execution.save(ground, turn).await;

  let deadline = turn.timeout.map(|bound| prompted_at + bound.as_millis() as i64);
  let read = |records: &[Value]| {
    lines_of(records.get(from..).unwrap_or_default(), &id)
      .map(|lines| read_turn(lines, &turn.run, provider))
  };
  // How many of the turn's own lines the watch has been handed.
  let mut fed = 0;

  loop {
    let cut = if execution.cancelled(ground, turn).await {
      Some(("the turn was stopped", "cancelled"))
    } else if deadline.is_some_and(|deadline| now_ms() >= deadline) {
      Some(("the turn ran past its time", "timeout"))
    } else {
      None
    };

    // A turn cut short is given a while to write that it was.
    if let Some((why, ending)) = cut {
      interrupt_program(place, &turn.home, &pane, &interrupt_of(resident)).await;

      let settle_by = tokio::time::Instant::now() + settle;

      while tokio::time::Instant::now() < settle_by {
        if read(&read_record(&place.path)).is_none_or(|(state, _, _)| state != State::Working) {
          break;
        }

        tokio::time::sleep(every).await;
      }

      execution.ending = ending;

      return Ok(SurfaceEnding::Error(why.to_owned()));
    }

    let alive = place.program_alive();

    if alive.is_none()
      || started
        .as_ref()
        .is_some_and(|started| alive.as_ref().is_some_and(|alive| alive.pid != started.pid))
    {
      return Err("the loop went away during the turn".to_owned());
    }

    let (written, records) = record_of(&place.path);
    let after = from.min(records.len());
    let reading = match span_of(&records[after..], &id) {
      None => None,
      Some(span) => {
        let lines = &written[after..][span.clone()];

        // What the turn's lines gained since the last look is its record as it goes.
        if lines.len() > fed {
          watch.record(&lines[fed..]).await;
          fed = lines.len();
        }

        Some(read_turn(&records[after..][span], &turn.run, provider))
      }
    };

    if let Some((state, error, reply)) = reading
      && state != State::Working
    {
      execution.ending = if state == State::Interrupted { "cancelled" } else { "exited" };

      return Ok(if state == State::Interrupted {
        SurfaceEnding::Error("the turn was interrupted".to_owned())
      } else if let Some(error) = error {
        SurfaceEnding::Error(error)
      } else if reply.text.is_empty() {
        SurfaceEnding::Error("the loop wrote no answer".to_owned())
      } else {
        SurfaceEnding::Reply(Box::new(reply))
      });
    }

    tokio::time::sleep(every).await;
  }
}
