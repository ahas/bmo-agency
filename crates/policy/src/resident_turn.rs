//! One turn put into an agent's standing program (docs/agent-control.md, 「The order for putting in one
//! turn and waiting for it to end」): the lead its words go in behind, and the paces it keeps. None of them
//! ends a turn or a wait in it.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResidentTurn {
  /// What the typed lead says about the words pasted after it.
  pub pasted_lead: String,
  /// What it says about a room's round pasted after it: nothing, since a round carries no typed lead.
  pub round_lead: String,
  /// What it says about a run apart's task pasted after it.
  pub apart_lead: String,
  pub poll_ms: u64,
  pub redraw_ms: u64,
  pub screen_answers: u32,
  pub submit_ms: u64,
  /// How often the record of the pane program that carries its own turn is looked at.
  pub inbox_poll_ms: u64,
  /// The most lines of a pane program's error stream one turn's run log takes.
  pub stderr_lines: usize,
  pub gone_looks: u32,
  pub gone_gap_ms: u64,
  pub end_poll_ms: u64,
  /// How full a session's context may be before it is folded once its turn is over, where neither the
  /// fold-ratio table nor the session's provider names a share.
  pub fold_ratio: f64,
}

crate::part!(pub RESIDENT_TURN: ResidentTurn = "resident-turn.toml");
