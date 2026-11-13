//! A person's PC (docs/person-nodes.md): the ports the far end of the link stands on there, the verbs its home
//! bridge runs here, and the grants a supervisor of no account holds, which reach every node.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonNodes {
  pub mirror_port: u16,
  pub herdr_port: u16,
  pub bridge_port: u16,
  pub bridge_relay_base: u16,
  pub wait_seconds: u64,
  /// The resources every tool session issued for a supervisor of no account holds.
  pub administrator_grants: Vec<String>,
  /// The verbs of the CLI a person's node may ask the home bridge to run here.
  pub bridge_verbs: Vec<String>,
}

crate::part!(pub PERSON_NODES: PersonNodes = "person-nodes.toml");
