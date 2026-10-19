//! The names under a worker's workbench agency keeps (docs/worker-bench.md, 「An item's scratch」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobPrompt {
  pub workbench: Workbench,
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
