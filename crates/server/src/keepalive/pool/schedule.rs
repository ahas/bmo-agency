//! When a pool place is worth a ping (docs/keepalive-schedule.md, 「When a pool place is worth a ping」):
//! the keepalive schedule's general rule on the place's clocks and record, while its last write is still
//! inside its usable lifetime. The lifetime and the idle stretch are its provider's declaration; the
//! margin is the pool's policy.

use cache_warmer::{Clocks, Record};
use policy::pool_warm::POOL_WARM;

const MINUTE_MS: i64 = 60_000;

/// The usable part of a published lifetime, in milliseconds: the pool's margin taken off, and none where
/// nothing is left of it.
pub fn usable(published_ms: i64) -> Option<i64> {
  cache_warmer::usable(published_ms, POOL_WARM.margin_minutes * MINUTE_MS)
}

/// A kept prefix as the pool judges it, every moment and span in milliseconds: when it was last written,
/// the lifetime its provider publishes for it, its idle stretch, and when a ping to its place last
/// answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefix {
  pub last_write_at: i64,
  pub published_ms: i64,
  pub idle_stretch_ms: i64,
  pub last_answered_at: Option<i64>,
}

impl Prefix {
  /// Whether it is worth a ping at `now`: due by the schedule's rule, its last write and its last
  /// answered ping each at least its idle stretch ago, and its last write still under its usable
  /// lifetime. A prefix whose provider publishes no usable lifetime is never due.
  pub fn due(&self, now: i64) -> bool {
    let clocks = Clocks {
      idle_ms: self.idle_stretch_ms,
      lifetime_ms: Some(self.published_ms),
      margin_ms: POOL_WARM.margin_minutes * MINUTE_MS,
      failure_touches: false,
      honours_idle_override: false,
    };
    let record = Record {
      last_used_at: Some(self.last_write_at),
      last_warmed_at: self.last_answered_at,
      last_failed_at: None,
    };

    clocks.due(&record, now, None) && clocks.kept(now - self.last_write_at)
  }
}
