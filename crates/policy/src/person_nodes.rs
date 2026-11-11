//! The ports the far end of the link stands on at a person's PC (docs/person-nodes.md).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonNodes {
  pub mirror_port: u16,
  pub herdr_port: u16,
  pub bridge_port: u16,
  pub bridge_relay_base: u16,
  pub wait_seconds: u64,
}

crate::part!(pub PERSON_NODES: PersonNodes = "person-nodes.toml");
