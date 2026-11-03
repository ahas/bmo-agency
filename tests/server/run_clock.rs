//! A run held to a clock is asked about past it, never cut by it (docs/agent-control.md, 「A clock asks, it
//! never cuts」): its clock read by its name and the unit it runs under, an ask opened and published past the
//! clock, a keep holding it until its moment, the supervisor's cut cutting it, an ask with no verdict standing
//! for as long as the run goes, a verdict on a closed ask refused, and the run's end closing what stood open —
//! on a store of the test's own.

use std::time::Duration;

use agency_server::door::turn::{Overrun, Overtime, Quiet, Watch};
use agency_server::events::Bus;
use agency_server::now_ms;
use agency_server::run_clock::{
  Asked, ClockAskClosed, ClockAsked, Clocked, NotSettled, Verdict, clock_of, close, open, open_asks, settle,
};

use crate::support::scratch::Scratch;

fn clocked(run: &str) -> Clocked {
  Clocked {
    run: run.to_owned(),
    parent: Some("batch-unit".to_owned()),
    work_key: "voice-reading".to_owned(),
    agent: Some("supervisor".to_owned()),
    what: "The voice-reading run".to_owned(),
    clock: Duration::from_secs(60),
  }
}

fn overdue() -> Overrun {
  Overrun {
    started_at: now_ms() - 61_000,
    bound_ms: 60_000,
    last_activity_at: None,
    last_activity: Some("Read".to_owned()),
  }
}

#[tokio::test]
async fn a_clocked_run_is_asked_about_kept_and_cut_on_the_chat_agents_verdict() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let bus = Bus::default();
  let mut asked_events = bus.subscribe::<ClockAsked>();
  let mut closed_events = bus.subscribe::<ClockAskClosed>();
  let clock = clocked("run-one");

  open(db, &clock).await.unwrap();

  let read = clock_of(db, "batch-unit").await.unwrap();

  assert_eq!(read, clock_of(db, "run-one").await.unwrap());
  assert!(read.cap_ends.unwrap() >= now_ms() + 50_000);
  assert_eq!(read.kept_until, None);
  assert_eq!(clock_of(db, "another").await.unwrap().cap_ends, None);

  let mut watch = Asked::new(Quiet, db, &bus, &clock);

  assert!(matches!(watch.overtime(&overdue()).await, Overtime::AskAgainAt(_)));

  let ask = asked_events.ready().expect("the ask was published");

  assert_eq!(
    (ask.run.as_str(), ask.agent.as_str(), ask.verb.as_str()),
    ("run-one", "supervisor", "bmo-agency overtime")
  );
  assert_eq!(clock_of(db, "run-one").await.unwrap().kept_until, None);
  assert_eq!(open_asks(db).await.unwrap().len(), 1);

  let kept = settle(db, &bus, ask.ask, &Verdict::Keep(Some(20)), "still reading").await.unwrap();
  let until = kept.kept_until.unwrap();

  assert!((until - now_ms() - 20 * 60_000).abs() < 5_000);
  assert_eq!(closed_events.ready().unwrap().by.as_deref(), Some("agent"));
  assert!(matches!(
    settle(db, &bus, ask.ask, &Verdict::Cut, "late").await,
    Err(NotSettled::Closed)
  ));
  assert!(matches!(settle(db, &bus, 999_999, &Verdict::Cut, "").await, Err(NotSettled::Unknown)));
  assert_eq!(watch.overtime(&overdue()).await, Overtime::AskAgainAt(until));
  assert_eq!(clock_of(db, "batch-unit").await.unwrap().kept_until, Some(until));

  // The keep run out, the run is asked again, and cut on the verdict with the supervisor's reason.
  scratch.run(r#"UPDATE "clock_asks" SET "kept_until" = 1"#, vec![]).await;

  assert!(matches!(watch.overtime(&overdue()).await, Overtime::AskAgainAt(_)));

  let again = asked_events.ready().expect("the second ask was published");

  settle(db, &bus, again.ask, &Verdict::Cut, "stuck on one file").await.unwrap();

  assert_eq!(watch.overtime(&overdue()).await, Overtime::Cut);
  assert!(watch.cut.as_deref().unwrap().contains("stuck on one file"));

  close(db, &bus, "run-one").await.unwrap();

  assert_eq!(clock_of(db, "run-one").await.unwrap().cap_ends, None);
}

#[tokio::test]
async fn an_ask_with_no_verdict_stands_while_the_run_goes_and_closes_as_ended_at_its_end() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let bus = Bus::default();
  let mut asked_events = bus.subscribe::<ClockAsked>();
  let mut closed_events = bus.subscribe::<ClockAskClosed>();
  let clock = clocked("run-two");

  open(db, &clock).await.unwrap();

  let mut watch = Asked::new(Quiet, db, &bus, &clock);

  watch.overtime(&overdue()).await;

  let open_ask = asked_events.ready().unwrap();

  // However long it stands unanswered, the run is only looked at again.
  scratch.run(r#"UPDATE "clock_asks" SET "asked_at" = 1"#, vec![]).await;

  for _ in 0..3 {
    assert!(matches!(watch.overtime(&overdue()).await, Overtime::AskAgainAt(_)));
  }

  assert!(watch.cut.is_none());
  assert!(closed_events.ready().is_none());
  assert_eq!(open_asks(db).await.unwrap().len(), 1);

  close(db, &bus, "run-two").await.unwrap();

  let ended = closed_events.ready().unwrap();

  assert_eq!((ended.ask, ended.verdict.as_str(), ended.by), (open_ask.ask, "ended", None));
  assert!(open_asks(db).await.unwrap().is_empty());
}
