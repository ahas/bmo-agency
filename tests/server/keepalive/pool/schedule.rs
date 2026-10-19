//! When a pool place is worth a ping: idle past its stretch and inside its usable lifetime, the published
//! lifetime less the pool's margin, and not pinged within its stretch; a lifetime no longer than the margin
//! leaves nothing, and such a prefix is never due.

use agency_server::keepalive::pool::schedule::{Prefix, usable};
use policy::pool_warm::POOL_WARM;

const MINUTE: i64 = 60_000;
const NOW: i64 = 1_800_000_000_000;

fn margin() -> i64 {
  POOL_WARM.margin_minutes * MINUTE
}

#[test]
fn the_usable_lifetime_is_the_published_one_less_the_margin_and_none_where_nothing_is_left() {
  assert_eq!(usable(60 * MINUTE), Some(60 * MINUTE - margin()));
  assert_eq!(usable(margin()), None);
  assert_eq!(usable(margin() + 1), Some(1));
}

#[test]
fn a_prefix_is_due_from_its_idle_stretch_until_its_usable_lifetime_and_not_once_pinged_within_it() {
  let prefix = |idle: i64, answered: Option<i64>| Prefix {
    last_write_at: NOW - idle,
    published_ms: 60 * MINUTE,
    idle_stretch_ms: 40 * MINUTE,
    last_answered_at: answered,
  };
  let usable = 60 * MINUTE - margin();

  assert!(!prefix(40 * MINUTE - 1, None).due(NOW), "not yet idle enough");
  assert!(prefix(40 * MINUTE, None).due(NOW));
  assert!(prefix(usable - 1, None).due(NOW));
  assert!(!prefix(usable, None).due(NOW), "already gone: left alone");
  assert!(!prefix(45 * MINUTE, Some(NOW - 40 * MINUTE + 1)).due(NOW), "pinged within its stretch");
  assert!(prefix(45 * MINUTE, Some(NOW - 40 * MINUTE)).due(NOW));
}

#[test]
fn a_prefix_whose_lifetime_leaves_nothing_past_the_margin_is_never_due() {
  let prefix = Prefix {
    last_write_at: NOW - MINUTE,
    published_ms: margin(),
    idle_stretch_ms: 0,
    last_answered_at: None,
  };

  assert!(!prefix.due(NOW));
}
