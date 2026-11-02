//! How a run is stopped from outside (docs/agent-control.md, 「Stopping」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunStop {
  /// What a run in a pane is told when it is being stopped.
  pub instruction: String,
}

crate::part!(pub RUN_STOP: RunStop = "run-stop.toml");
