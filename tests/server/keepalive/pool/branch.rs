//! The co-work places a reservation's owner is answered with: none by default, so nothing is warmed, and
//! the consumer's answer as it registered it.

use agency_server::hooks::Hooks;
use agency_server::keepalive::pool::branch::{CoworkPlaces, OwnerPlaces};
use agency_server::order_run::places::Held;

#[tokio::test]
async fn a_reservation_no_consumer_answers_for_has_no_places() {
  assert_eq!(Hooks::default().ask::<CoworkPlaces>("work-order:12".into()).await, None);
}

#[tokio::test]
async fn the_consumer_answers_its_orders_checkout_and_the_places_its_tasks_sit_in() {
  let hooks = Hooks::default();

  hooks.register::<CoworkPlaces, _, _>(|owner| async move {
    (owner == "work-order:12").then(|| OwnerPlaces {
      cwd: "/nowhere/one".into(),
      held: vec![Held {
        cwd: "/nowhere/one".into(),
        slot: 2,
      }],
    })
  });

  let answered = hooks.ask::<CoworkPlaces>("work-order:12".into()).await.unwrap();

  assert_eq!(answered.cwd, "/nowhere/one");
  assert_eq!(
    answered.held,
    [Held {
      cwd: "/nowhere/one".into(),
      slot: 2
    }]
  );
  assert_eq!(hooks.ask::<CoworkPlaces>("work-order:13".into()).await, None);
}
