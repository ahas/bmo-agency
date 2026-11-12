//! Which jobs stand in the machine's turn (docs/role-agents.md, 「When an agent is busy」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineTurn {
  /// How often the testbed's lanes and the job service's items under a close are read, in seconds.
  pub read_seconds: u64,
  /// The label the job service's close writes on its item while it tests and while its commit waits to go live.
  pub close_label: String,
}

crate::part!(pub MACHINE_TURN: MachineTurn = "machine-turn.toml");
