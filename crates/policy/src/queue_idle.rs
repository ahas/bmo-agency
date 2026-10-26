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
  /// What the chat agent is asked to do about it.
  pub ask: String,
}

crate::part!(pub QUEUE_IDLE: QueueIdle = "queue-idle.toml");
