//! The names under a worker's workbench agency keeps (docs/worker-bench.md, 「An item's scratch」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobPrompt {
  pub workbench: Workbench,
  pub notes: Notes,
  pub lent: Lent,
}

/// What opens a job's prompt where checkouts of other repositories are lent to it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lent {
  /// The heading over the list.
  pub heading: String,
  /// The line before the list.
  pub lead: String,
  /// One line a lent checkout, with `{repository}`, `{path}` and `{branch}`.
  pub line: String,
}

/// How much of its item's notes a job's prompt carries.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notes {
  /// The most bytes of notes a prompt carries, the newest first.
  pub most_bytes: usize,
  /// The line saying how many older notes were left out, with `{n}`.
  pub left_out: String,
}

/// The names under a worker's workbench.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workbench {
  /// The folder that holds each item's scratch, one folder per item number.
  pub scratch: String,
  /// What an item's own database is named from: this, the item number, an underscore, and anything.
  pub item_database: String,
}

crate::part!(pub JOB_PROMPT: JobPrompt = "job-prompt.toml");
