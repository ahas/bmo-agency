//! A person's PC (docs/person-nodes.md): the ports the far end of the link stands on there, and the grants a
//! caller of the deployment's own holds, which reach every node.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonNodes {
  pub mirror_port: u16,
  pub herdr_port: u16,
  pub bridge_port: u16,
  pub bridge_relay_base: u16,
  pub wait_seconds: u64,
  /// The resources every tool session issued for a caller of the deployment's own holds.
  pub administrator_grants: Vec<String>,
}

crate::part!(pub PERSON_NODES: PersonNodes = "person-nodes.toml");
