//! The clock a job turn's tool calls are held to (docs/agent-control.md, 「A job turn's tool calls are
//! held to a clock」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolClock {
  pub call: CallClock,
  pub turn: TurnClock,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallClock {
  /// How long a call goes before the clock watches it at all.
  pub min_seconds: u64,
  /// How long a call's process tree may burn no processor time before the call is stopped.
  pub stall_seconds: u64,
  /// How often the call's process tree is read.
  pub look_seconds: u64,
  /// When the first still-running line is written for a call.
  pub log_after_seconds: u64,
  /// How often the still-running line is repeated.
  pub still_every_seconds: u64,
  /// On a stop, how long a running call may burn no processor time before it is stopped.
  pub stop_stall_seconds: u64,
  /// A stop the job has not ended with itself within this long is ended by the interrupt.
  pub stop_bound_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TurnClock {
  /// A job turn past this clock is cut and its item released.
  pub bound_minutes: u64,
  /// Why a job's turn was cut at its bound; `{minutes}` is the bound in minutes.
  pub cut: String,
}

crate::part!(pub TOOL_CLOCK: ToolClock = "tool-clock.toml");
