//! An agent's place: how soon a program started often is started again.

use agent_host::place::restart_delay;

#[test]
fn three_starts_a_minute_go_at_once_and_then_the_wait_doubles() {
  let now = 1_000_000;
  let starts = |n: u64| (0..n).map(|i| now - 1_000 * (i + 1)).collect::<Vec<_>>();

  assert_eq!(restart_delay(&starts(2), now), 0);
  assert_eq!(restart_delay(&starts(3), now), 2_000);
  assert_eq!(restart_delay(&starts(4), now), 4_000);
  assert_eq!(restart_delay(&starts(5), now), 8_000);
  assert_eq!(restart_delay(&starts(9), now), 30_000);
  // Older than a minute counts for the loop, not the wait.
  assert_eq!(restart_delay(&[now - 120_000; 6], now), 0);
}
