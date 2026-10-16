//! A companion's run going in a room: its start published as it begins and its end as it is let go.

use agency_server::companion::going::{CompanionRunGoing, going};
use agency_server::events::Bus;

#[test]
fn a_run_is_published_going_and_then_ended_once_let_go() {
  let bus = Bus::default();
  let mut seen = bus.subscribe::<CompanionRunGoing>();
  let held = going(&bus, "pal", "handoff:7:1", 7);

  assert_eq!(
    seen.ready(),
    Some(CompanionRunGoing {
      agent: "pal".into(),
      run: "handoff:7:1".into(),
      chat_id: 7,
      going: true,
    })
  );
  assert_eq!(seen.ready(), None);

  drop(held);

  assert_eq!(seen.ready().map(|ended| ended.going), Some(false));
}
