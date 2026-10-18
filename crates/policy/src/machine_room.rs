//! The part of this machine's room agency judges (docs/agent-control.md, 「Room on the machine」).

use serde::Deserialize;

/// One job-count limit: its default and the reason a launch it holds is written as.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountLimit {
  /// The limit where the settings table holds no positive number for it.
  pub default: u64,
  /// The reason, with `{jobs}` and `{limit}` marked.
  pub held: String,
}

/// The clocks of the held launch's watch and of the pressure stream.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineClocks {
  pub held_watch_seconds: u64,
  pub pressure_reopen_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineRoom {
  /// The jobs at work on this machine.
  pub launch_jobs: CountLimit,
  /// The jobs at work whose place runs a browser.
  pub launch_browser_jobs: CountLimit,
  pub clocks: MachineClocks,
}

crate::part!(pub MACHINE_ROOM: MachineRoom = "machine-room.toml");
