//! The idle queue's alarm (docs/agent-control.md, 「An idle queue is told to the owner」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueIdle {
  /// How long no job may start while approved work waits and a worker is free, in minutes.
  pub idle_minutes: i64,
  /// How often the queue is looked at, in seconds.
  pub check_seconds: u64,
  /// The code the owner's notice carries.
  pub code: String,
  /// What the supervisor is asked to do about it.
  pub ask: String,
  /// How long an approved item may wait on its own free worker before the supervisor is told of it, in
  /// seconds.
  pub launch_seconds: i64,
  /// What the supervisor is told of such an item: `{item}`, `{title}` and `{worker}`.
  pub launch_order: String,
  /// How long after one of a free agent's items is told the next is, while the agent stays free, in seconds.
  pub next_offer_seconds: i64,
  /// How many of a draft's brief's first lines the supervisor is told.
  pub draft_brief_lines: usize,
  /// The most characters of those lines the supervisor is told.
  pub draft_brief_chars: usize,
  /// What the supervisor is told of a draft awaiting approval: `{item}`, `{title}`, `{raised_by}`,
  /// `{repository}` and `{brief}`.
  pub draft_order: String,
}

crate::part!(pub QUEUE_IDLE: QueueIdle = "queue-idle.toml");
