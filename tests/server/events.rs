//! The event bus: an event reaches every subscriber of its kind from the moment it subscribed, and nobody
//! where there is none; and a stream across the boundary reads the kinds it names, each by its name and its
//! JSON.

use agency_server::events::{Bus, Event, Named};
use serde_json::json;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
struct Said(String);

impl Event for Said {
  const NAME: &'static str = "said";
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
struct Ended(u32);

impl Event for Ended {
  const NAME: &'static str = "ended";
}

#[test]
fn an_event_with_no_subscriber_reaches_nobody() {
  let bus = Bus::default();

  bus.publish(Said("unheard".into()));

  assert_eq!(bus.subscribers::<Said>(), 0);

  let mut late = bus.subscribe::<Said>();

  assert_eq!(late.ready(), None, "a subscription reads from the moment it was made");
}

#[tokio::test]
async fn every_subscriber_of_a_kind_takes_each_event_and_no_other_kind() {
  let bus = Bus::default();
  let mut first = bus.subscribe::<Said>();
  let mut second = bus.clone().subscribe::<Said>();
  let mut ended = bus.subscribe::<Ended>();

  bus.publish(Said("hello".into()));
  bus.publish(Said("again".into()));

  assert_eq!(first.next().await, Some(Said("hello".into())));
  assert_eq!(first.next().await, Some(Said("again".into())));
  assert_eq!(second.ready(), Some(Said("hello".into())));
  assert_eq!(second.ready(), Some(Said("again".into())));
  assert_eq!(ended.ready(), None);

  bus.publish(Ended(7));

  assert_eq!(ended.ready(), Some(Ended(7)));
  assert_eq!(first.ready(), None);
}

#[test]
fn a_dropped_subscription_is_let_go() {
  let bus = Bus::default();
  let kept = bus.subscribe::<Said>();
  let dropped = bus.subscribe::<Said>();

  assert_eq!(bus.subscribers::<Said>(), 2);

  drop(dropped);

  assert_eq!(bus.subscribers::<Said>(), 1);

  bus.publish(Said("still heard".into()));
  drop(kept);
  bus.publish(Said("unheard".into()));

  assert_eq!(bus.subscribers::<Said>(), 0);
}

#[tokio::test]
async fn a_stream_reads_the_kinds_it_names_by_name_and_json() {
  let bus = Bus::default();
  let mut said_only = bus.subscribe_wire(Some(vec!["said".into()]));
  let mut every = bus.subscribe_wire(None);

  bus.publish(Ended(7));
  bus.publish(Said("hello".into()));

  assert_eq!(
    said_only.next().await,
    Some(Named {
      name: "said",
      body: json!("hello")
    })
  );
  assert_eq!(
    every.next().await,
    Some(Named {
      name: "ended",
      body: json!(7)
    })
  );
  assert_eq!(
    every.next().await,
    Some(Named {
      name: "said",
      body: json!("hello")
    })
  );
}
