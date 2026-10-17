//! A run held to a clock is asked about past its bound rather than cut (docs/agent-control.md, 「A clock asks
//! before it cuts」): its clock read by its name and the unit it runs under, an ask opened and published past
//! the bound, a keep holding it until its moment, a cut and a want of a verdict cutting it with whose call it
//! was, a verdict on a closed ask refused, and the run's end closing what stood open; and the editor cap read
//! for an agent — on a store of the test's own.

use std::time::Duration;

use agency_server::door::turn::{Overrun, Overtime, Quiet, Watch};
use agency_server::events::Bus;
use agency_server::now_ms;
use agency_server::run_clock::{
  Asked, ClockAskClosed, ClockAsked, Clocked, NotSettled, Verdict, clock_of, close, editor_cap, open,
  open_asks, settle,
};
use policy::run_clock::RUN_CLOCK;

use crate::scratch::Scratch;

fn clocked(run: &str) -> Clocked {
  Clocked {
    run: run.to_owned(),
    parent: Some("batch-unit".to_owned()),
    work_key: "digest".to_owned(),
    agent: Some("companion".to_owned()),
    what: "The digest run".to_owned(),
    bound: Duration::from_secs(60),
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
    ("run-one", "companion", "bmo-agency overtime")
  );
  assert_eq!(clock_of(db, "run-one").await.unwrap().kept_until, Some(ask.answer_by));
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

  // The keep run out, the run is asked again, and cut on the verdict with the chat agent's reason.
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
async fn an_ask_with_no_verdict_by_its_moment_is_cut_by_agency_and_one_open_at_the_end_closes_as_ended() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let bus = Bus::default();
  let mut asked_events = bus.subscribe::<ClockAsked>();
  let mut closed_events = bus.subscribe::<ClockAskClosed>();
  let clock = clocked("run-two");

  open(db, &clock).await.unwrap();

  let mut watch = Asked::new(Quiet, db, &bus, &clock);

  watch.overtime(&overdue()).await;
  scratch.run(r#"UPDATE "clock_asks" SET "answer_by" = 1"#, vec![]).await;

  assert_eq!(watch.overtime(&overdue()).await, Overtime::Cut);
  assert!(watch.cut.as_deref().unwrap().contains("no verdict came"));
  assert_eq!(closed_events.ready().unwrap().by.as_deref(), Some("agency"));

  let mut other = Asked::new(Quiet, db, &bus, &clock);

  other.overtime(&overdue()).await;

  let _cut = asked_events.ready();
  let open_ask = asked_events.ready().unwrap();

  close(db, &bus, "run-two").await.unwrap();

  let ended = closed_events.ready().unwrap();

  assert_eq!((ended.ask, ended.verdict.as_str(), ended.by), (open_ask.ask, "ended", None));
  assert!(open_asks(db).await.unwrap().is_empty());
}

#[tokio::test]
async fn the_editor_cap_is_the_policys_unless_the_agents_recollector_registration_waits_longer() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let cap = Duration::from_secs(RUN_CLOCK.editor_cap_seconds);

  assert_eq!(editor_cap(db, None, None).await.unwrap(), cap);
  assert_eq!(editor_cap(db, Some("companion"), None).await.unwrap(), cap);

  scratch
    .run(
      r#"INSERT INTO "companion_settings" ("agent", "recollector_model") VALUES ('companion', 'unregistered')"#,
      vec![],
    )
    .await;

  assert_eq!(editor_cap(db, Some("companion"), None).await.unwrap(), cap);

  scratch
    .run(
      "INSERT INTO registered_models VALUES ('local-0-1', 'Local 0.1', 'ko2', 32768, 4096, 2400, 1, 0, 200, 1)",
      vec![],
    )
    .await;
  scratch
    .run(r#"UPDATE "companion_settings" SET "recollector_model" = 'local-0-1'"#, vec![])
    .await;

  assert_eq!(editor_cap(db, Some("companion"), None).await.unwrap(), Duration::from_secs(2_400));
  assert_eq!(editor_cap(db, None, Some("local-0-1")).await.unwrap(), Duration::from_secs(2_400));
}
