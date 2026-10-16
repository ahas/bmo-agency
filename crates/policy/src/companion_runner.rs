//! A companion's turn as its runner carries it (docs/companions.md): its guard's limits, its engine wait,
//! how the room's words and its blocks are laid out, and the fixed lines of its log, refusals and cut.

use serde::Deserialize;

/// Where a turn going nowhere is cut short.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
  pub failures: usize,
  pub repeats: usize,
  pub repeat_window: usize,
  pub likeness: f64,
  pub calls: i64,
  pub minutes: i64,
}

impl Limits {
  /// The longest the runs on one turn's words go on, in milliseconds.
  pub fn ms(&self) -> i64 {
    self.minutes * 60_000
  }
}

/// How a room's words read in a turn.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Words {
  pub owner_mark: String,
  pub quiet_head: String,
}

/// The blocks a turn's words open with.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blocks {
  pub reference_opening: String,
  pub reference_closing: String,
  pub handoff_opening: String,
  pub handoff_closing: String,
}

/// The runs a turn and a handoff are logged as.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runs {
  pub turn: String,
  pub handoff: String,
}

/// The turn's log and the bot's events.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Log {
  pub start: String,
  pub skipped: String,
  pub answered: String,
  pub cleared: String,
  pub system_changed: String,
  pub carried: String,
  pub handoff_start: String,
  pub no_handoff: String,
  pub handed_over: String,
  pub waits: String,
  pub waits_item: String,
  pub title: String,
}

/// What the answer is read under as it is cleaned, and how it is written down.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
  pub label: String,
  pub verdict: String,
  pub verdict_chars: usize,
  pub echoed_words: String,
  pub echoed_line: String,
}

/// Why a turn fails.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refused {
  pub no_pin: String,
  pub no_provider: String,
  pub no_line: String,
  pub no_model: String,
  pub echoed: String,
  pub empty: String,
}

/// A turn its guard cut short.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cut {
  pub overtime: String,
  pub too_many_calls: String,
  pub failing: String,
  pub repeating: String,
  pub mark: String,
  pub notice: String,
  pub instruction: Vec<String>,
  pub kind: String,
  pub sort: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionRunner {
  pub skip: String,
  pub echoed_line_chars: usize,
  pub engine_wait_ms: u64,
  pub tally_every_ms: u64,
  pub answer_stage: String,
  pub turn_stage: String,
  pub no_note: String,
  pub limits: Limits,
  pub words: Words,
  pub blocks: Blocks,
  pub runs: Runs,
  pub log: Log,
  pub answer: Answer,
  pub refused: Refused,
  pub cut: Cut,
}

crate::part!(pub COMPANION_RUNNER: CompanionRunner = "companion-runner.toml");
