//! The rest a usage limit calls for.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rest {
  /// The share of the five-hour window past which a login rests.
  pub threshold: f64,
  /// The share of the weekly window past which a login rests.
  pub weekly_threshold: f64,
  /// How long a login is put aside where its limit named no reset moment, in seconds.
  pub fallback_seconds: i64,
  /// What the owner is told as the rest that silenced the last open login begins.
  pub resting: String,
  /// How often a rest past its end is looked for and lifted, in seconds.
  pub lift_check_seconds: u64,
}

crate::part!(pub REST: Rest = "rest.toml");
