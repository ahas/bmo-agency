//! The pool's margin off a published lifetime, and a ping's reason cut by the units a string counts.

use agency_server::keepalive::pool::{first_units, usable};

#[test]
fn the_margin_is_taken_off_a_lifetime_and_nothing_is_left_of_a_short_one() {
  assert_eq!(usable(60 * 60_000), Some(55 * 60_000));
  assert_eq!(usable(5 * 60_000), None);
}

#[test]
fn a_reason_is_cut_by_the_units_a_script_counts() {
  assert_eq!(first_units("a\u{1F600}b", 3), "a\u{1F600}");
  assert_eq!(first_units("ab", 400), "ab");
}
