//! A ping's reason cut by the units a string counts. The margin off a published lifetime is the keepalive
//! schedule's, tested with the cache-warmer crate.

use agency_server::keepalive::pool::first_units;

#[test]
fn a_reason_is_cut_by_the_units_a_script_counts() {
  assert_eq!(first_units("a\u{1F600}b", 3), "a\u{1F600}");
  assert_eq!(first_units("ab", 400), "ab");
}
