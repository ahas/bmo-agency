//! How a run is stopped from outside (docs/agent-control.md, 「Stopping」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunStop {
  /// What a run in a pane is told when it is being stopped.
  pub instruction: String,
  /// How long a run in a pane that was asked is given before its pane is closed.
  pub grace_seconds: u64,
  /// How long a stopped resident turn's collector is given before the stop reaches the program.
  pub resident_backstop_seconds: u64,
  /// How long a job's stop waits for the job's turn to end before it answers that the stop stands.
  pub job_wait_seconds: u64,
}

crate::part!(pub RUN_STOP: RunStop = "run-stop.toml");
