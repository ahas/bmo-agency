//! What a conversational agent's shell guard refuses beside a start of Claude Code (docs/agents.md, 「No agent
//! reaches another unit's store」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellGuard {
  /// The database clients it may not run.
  pub programs: Vec<String>,
  /// The address schemes it may not name.
  pub schemes: Vec<String>,
  /// The loopback ports the deployment's units answer on, one by one.
  pub ports: Vec<u16>,
  /// The same, by ranges, both ends included.
  pub port_ranges: Vec<[u16; 2]>,
  /// The sentence a refusal ends with, naming the road instead.
  pub road: String,
}

impl ShellGuard {
  /// Whether a loopback port is one a unit answers on.
  pub fn guards_port(&self, port: u16) -> bool {
    self.ports.contains(&port) || self.port_ranges.iter().any(|[low, high]| (*low..=*high).contains(&port))
  }
}

crate::part!(pub SHELL_GUARD: ShellGuard = "shell-guard.toml");
