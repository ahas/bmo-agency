//! The bound a program on a node is held to before it takes its turn (docs/work-nodes.md, 「When a node drops」),
//! looked at on moments given rather than waited out.

use std::time::Duration;

use agency_server::door::node_take::{NodeTake, Take};
use tokio::time::Instant;

const NOTICE: Duration = Duration::from_secs(60);
const BOUND: Duration = Duration::from_secs(300);

#[test]
fn the_notice_is_said_once_and_the_bound_ends_the_turn_naming_the_node() {
  let mut take = NodeTake::new("u4", NOTICE, BOUND);
  let start = Instant::now();

  assert_eq!(take.look(true, start), Take::Within);
  assert_eq!(take.look(true, start + Duration::from_secs(59)), Take::Within);

  let Take::Notice { reason, stood } = take.look(true, start + NOTICE) else {
    panic!("the notice is said as it passes");
  };

  assert!(reason.contains("u4") && reason.contains("60 seconds"), "{reason}");
  assert_eq!(stood, NOTICE);
  assert!(take.noticed());
  assert_eq!(take.look(true, start + Duration::from_secs(120)), Take::Within, "said once");

  let Take::Overdue(why) = take.look(true, start + BOUND) else {
    panic!("the bound ends the turn");
  };

  assert!(why.contains("u4") && why.contains("300 seconds"), "{why}");
}

#[test]
fn a_while_the_node_s_link_is_not_heard_is_the_link_s_and_the_count_starts_again_after_it() {
  let mut take = NodeTake::new("u4", NOTICE, BOUND);
  let start = Instant::now();

  assert_eq!(take.look(true, start), Take::Within);
  assert_eq!(take.look(false, start + Duration::from_secs(290)), Take::Within);
  // Heard again: the count begins at this look, so the bound is five minutes from here.
  assert_eq!(take.look(true, start + Duration::from_secs(400)), Take::Within);
  assert!(matches!(take.look(true, start + Duration::from_secs(460)), Take::Notice { .. }));
  assert_eq!(take.look(true, start + Duration::from_secs(699)), Take::Within);
  assert!(matches!(take.look(true, start + Duration::from_secs(700)), Take::Overdue(_)));
}
