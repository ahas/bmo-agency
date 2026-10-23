//! The places a gate's turns stand in on a road whose program carries its own turns (docs/agent-control.md,
//! 「A gate's places」): how many stand at once and for one program, how long one stands unused, and how many
//! tokens its session may hold.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatePlaces {
  pub places: usize,
  pub per_program: usize,
  pub idle_minutes: u64,
  pub session_tokens: i64,
}

impl GatePlaces {
  /// How long a place may stand unused, in milliseconds.
  pub fn idle_ms(&self) -> i64 {
    i64::try_from(self.idle_minutes.saturating_mul(60_000)).unwrap_or(i64::MAX)
  }
}

crate::part!(pub GATE_PLACES: GatePlaces = "gate-places.toml");
