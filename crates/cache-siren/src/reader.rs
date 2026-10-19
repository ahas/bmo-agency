//! What a reader of an account's windows declares of itself (docs/usage-windows.md, 「Where readings come
//! from」): the provider it reads, how often it passes and how long its reading stands. Each reader answers
//! for its own provider; nothing keeps a table of them.

use std::time::Duration;

/// A reader of one provider's windows on a clock.
pub trait Reader: Send + Sync {
  /// The provider whose windows it reads, as a slot names it.
  fn mode(&self) -> &str;
  /// How often it passes.
  fn cadence(&self) -> Duration;
  /// How long a reading of its provider's windows stands after it was seen.
  fn fresh(&self) -> Duration;
}
