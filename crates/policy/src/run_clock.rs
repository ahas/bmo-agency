//! The clock a run of work is held to and the ask put past it (docs/agent-control.md, 「A clock asks
//! before it cuts」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunClock {
  /// How long an ask waits for a verdict before agency cuts the run itself.
  pub answer_wait_seconds: u64,
  /// How often a run waiting on a verdict looks for one.
  pub look_every_seconds: u64,
  /// The longest one keep gives.
  pub longest_keep_seconds: u64,
  /// How long a clock's rows are kept once its run has ended.
  pub kept_days: u64,
  /// The command a verdict is given with.
  pub verb: String,
  /// Why a run cut on its clock failed, by whose call.
  pub cut_by_agent: String,
  pub cut_by_agency: String,
}

crate::part!(pub RUN_CLOCK: RunClock = "run-clock.toml");
