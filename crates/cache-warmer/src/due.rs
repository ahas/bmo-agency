//! When a kept prefix is worth a ping (docs/keepalive-schedule.md, 「When a prefix is worth a ping」): idle
//! long enough to need one, still inside its usable lifetime, and not pinged within its idle stretch. The
//! lifetime and the idle stretch are its provider's declaration; the margin is the pool's policy.

use policy::pool_warm::POOL_WARM;

const MINUTE_MS: i64 = 60_000;

/// The usable part of a published lifetime, in milliseconds: the margin taken off, and none where
/// nothing is left of it.
pub fn usable(published_ms: i64) -> Option<i64> {
  let usable = published_ms - POOL_WARM.margin_minutes * MINUTE_MS;

  (usable > 0).then_some(usable)
}

/// A kept prefix as the schedule judges it, every moment and span in milliseconds: when it was last
/// written, the lifetime its provider publishes for it, its idle stretch, and when a ping to its place
/// last answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefix {
  pub last_write_at: i64,
  pub published_ms: i64,
  pub idle_stretch_ms: i64,
  pub last_answered_at: Option<i64>,
}

impl Prefix {
  /// Whether it is worth a ping at `now`: its idle time has reached its idle stretch and is still under
  /// its usable lifetime, and no ping to its place answered within its idle stretch. A prefix whose
  /// provider publishes no usable lifetime is never due.
  pub fn due(&self, now: i64) -> bool {
    let Some(usable) = usable(self.published_ms) else { return false };
    let idle = now - self.last_write_at;

    idle >= self.idle_stretch_ms
      && idle < usable
      && self.last_answered_at.is_none_or(|answered| now - answered >= self.idle_stretch_ms)
  }
}
