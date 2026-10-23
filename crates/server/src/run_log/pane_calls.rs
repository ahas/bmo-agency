//! A pane program's tool calls (docs/run-records.md, 「Agent log」): bmo-tui's record says a call began by a
//! `progress` line naming the tool and the state dsh gave it, and that it settled by one naming the state it
//! settled in. Each call is filed as a `tool` line under the tool's name and a `tool-result` line saying how it
//! ended and how long it took, as a stream's calls are, so a turn on that road reads back with its calls. The
//! record carries no call's id and no arguments, so a call is matched to its ending first in, first out, under a
//! reference made of the turn's id and the call's place in it.

use std::collections::VecDeque;

use super::json::Json;
use super::lines::Line;

// The name bmo-tui writes where an update names no tool, as a call's settling does.
const UNNAMED: &str = "tool";

// The states a call stands in before it settles, in dsh's words.
const PENDING: &str = "pending";
const STARTED: [&str; 2] = [PENDING, "in_progress"];

#[derive(Debug)]
struct Open {
  reference: String,
  name: String,
  at: Option<i64>,
  pending: bool,
}

/// The calls of one run's pane program begun and not yet settled, and how many it has made.
#[derive(Debug, Default)]
pub struct PaneCalls {
  open: VecDeque<Open>,
  made: u64,
}

fn number(event: &Json, name: &str) -> Option<i64> {
  match event.get(name)? {
    Json::Number(number) => number.as_i64(),
    _ => None,
  }
}

impl PaneCalls {
  /// The lines one `progress` line of the record stands for beside itself: a call's start, its ending, or
  /// none.
  pub fn lines(&mut self, event: &Json) -> Vec<Line> {
    if event.get("phase").and_then(Json::as_str) != Some("tool") {
      return vec![];
    }

    let name = event.get("tool").and_then(Json::as_str).unwrap_or(UNNAMED);
    let state = event.get("state").and_then(Json::as_str);
    let at = number(event, "at");

    match state {
      Some("completed" | "failed") => self.settled(state == Some("failed"), at),
      None => self.begun(name, true, event, at),
      Some(state) if STARTED.contains(&state) => self.begun(name, state == PENDING, event, at),
      Some(_) => vec![],
    }
  }

  // A call begun: a new one, unless it is the last one asked still waiting to start, now under way.
  fn begun(&mut self, name: &str, pending: bool, event: &Json, at: Option<i64>) -> Vec<Line> {
    if name == UNNAMED {
      return vec![];
    }

    if let Some(last) = self.open.back_mut()
      && last.pending
      && last.name == name
    {
      last.pending = pending;

      return vec![];
    }

    self.made += 1;

    let turn = event.get("id").and_then(Json::as_str).unwrap_or_default();
    let reference = format!("{turn}:{}", self.made);

    self.open.push_back(Open {
      reference: reference.clone(),
      name: name.to_owned(),
      at,
      pending,
    });

    vec![Line {
      kind: "tool",
      name: Some(name.to_owned()),
      reference: Some(reference),
      state: Some("requested"),
      ..Line::default()
    }]
  }

  // The oldest call open settled, with how long it ran where both moments are known.
  fn settled(&mut self, failed: bool, at: Option<i64>) -> Vec<Line> {
    let Some(open) = self.open.pop_front() else {
      return vec![];
    };
    let word = if failed { "failed" } else { "completed" };
    let text = match open.at.zip(at) {
      Some((began, ended)) => format!("{word} in {} ms", (ended - began).max(0)),
      None => word.to_owned(),
    };

    vec![Line {
      kind: "tool-result",
      reference: Some(open.reference),
      state: Some(if failed { "failed" } else { "succeeded" }),
      text,
      ..Line::default()
    }]
  }

  /// How many calls the run has made.
  pub fn made(&self) -> u64 {
    self.made
  }
}
