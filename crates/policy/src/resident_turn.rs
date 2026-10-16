//! One turn put into an agent's standing program (docs/agent-control.md, 「The order for putting in one
//! turn and waiting for it to end」): the lead its words go in behind, and the clocks it keeps.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResidentTurn {
  /// What the typed lead says about the words pasted after it.
  pub pasted_lead: String,
  /// What it says about a room's round pasted after it.
  pub round_lead: String,
  /// What it says about a run apart's task pasted after it.
  pub apart_lead: String,
  pub ready_seconds: u64,
  pub poll_ms: u64,
  pub redraw_ms: u64,
  pub trust_answers: u32,
  pub person_turn_minutes: u64,
  pub account_ms: u64,
  pub settle_ms: u64,
  pub submit_ms: u64,
  pub stop_grace_seconds: u64,
  pub quiet_ms: u64,
  pub herdr_patience_seconds: u64,
  pub node_patience_seconds: u64,
  pub loop_ready_seconds: u64,
  pub loop_poll_ms: u64,
  /// How often the resident loop relays the steps dsh took into its record, and how much of a tool's
  /// answer it keeps.
  pub loop_relay_ms: u64,
  pub loop_relay_chars: usize,
  pub gone_looks: u32,
  pub gone_gap_ms: u64,
  pub end_wait_ms: u64,
  pub end_poll_ms: u64,
  /// How full a session's context may be before it is folded once its turn is over.
  pub fold_ratio: f64,
}

crate::part!(pub RESIDENT_TURN: ResidentTurn = "resident-turn.toml");
