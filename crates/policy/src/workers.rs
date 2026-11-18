//! What a job worker may be (docs/worker-slots.md).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerRules {
  /// The pattern a worker's name matches.
  pub name_pattern: String,
  /// The longest a worker's label may be, in characters.
  pub label_most: usize,
  /// The longest a worker's preamble may be, in characters.
  pub preamble_most: usize,
  /// The longest a worker's dev server command may be, in characters.
  pub dev_command_most: usize,
  /// How many support workers one base may have where its count is a number.
  pub support_most: i64,
  /// How long an idle support checkout of a repository set to `auto` stands before it is taken away, in
  /// hours.
  pub support_idle_hours: i64,
  /// The first support worker's number.
  pub first_support_slot: i64,
}

crate::part!(pub WORKERS: WorkerRules = "workers.toml");
