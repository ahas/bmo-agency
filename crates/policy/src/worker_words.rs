//! A word said to a worker: its clocks, what its turn is called, and the lines written under it.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerWords {
  pub answer_minutes: u64,
  pub tick_seconds: u64,
  pub person_wait_seconds: u64,
  /// With `{worker}`.
  pub title: String,
  /// With `{worker}`.
  pub what: String,
  pub work_key: String,
  pub held: Held,
  pub ended: Ended,
}

/// The line written under a word for what held it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Held {
  pub person: String,
  pub untrusted: String,
  pub unsubmitted: String,
  pub stopped: String,
  pub disabled: String,
  pub lost: String,
  pub resting: String,
  pub job_waiting: String,
  pub in_job: String,
  /// With `{provider}`.
  pub pinned: String,
}

/// The line written under the last word a turn took where it wrote no answer.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ended {
  pub silent: String,
  /// With `{clock}` and `{cut}`.
  pub cut: String,
}

crate::part!(pub WORKER_WORDS: WorkerWords = "worker-words.toml");
