//! Agency's tier ladder (docs/model-tiers.md).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tiers {
  /// Every tier, in the order the tiers are shown and compared.
  pub ladder: Vec<String>,
  /// The most places the pool may offer at one tier on one provider.
  pub capacity_most: i64,
  /// The tiers a co-worker is borrowed at, in the ladder's order.
  pub coworker: Vec<CoworkerTier>,
}

/// A tier a co-worker is borrowed at, with the places its pool offers where nobody set any.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoworkerTier {
  pub tier: String,
  pub capacity: i64,
}

impl Tiers {
  pub fn holds(&self, tier: &str) -> bool {
    self.ladder.iter().any(|one| one == tier)
  }

  /// Whether a tier is one a co-worker is borrowed at.
  pub fn lends(&self, tier: &str) -> bool {
    self.coworker.iter().any(|one| one.tier == tier)
  }

  /// The tiers a co-worker is borrowed at, in the ladder's order.
  pub fn coworker_tiers(&self) -> Vec<&str> {
    self.coworker.iter().map(|one| one.tier.as_str()).collect()
  }
}

crate::part!(pub TIERS: Tiers = "tiers.toml");
