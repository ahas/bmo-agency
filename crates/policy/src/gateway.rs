//! The model gateway's front (docs/own-llm.md, 「A streamed answer whose bytes stop is cut」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gateway {
  /// How long a streamed answer may send no byte, from its headers on, before the front cuts it, in seconds.
  pub stall_seconds: u64,
}

crate::part!(pub GATEWAY: Gateway = "gateway.toml");
