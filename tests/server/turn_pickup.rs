//! A turn in flight across agency's own restart, picked up again (docs/sessions.md, 「A restart hands the
//! session over」): a job whose turn ended in its program's record after the process that sent it went away
//! ends with its report rather than stopped, a chat turn's end reaches its caller under its own id with its
//! run closed, a turn its program is still working on is waited on, and one no record names ends failed; a
//! job's turn adopted at start stands as seen, its slot kept while it goes and given back as it ends; a job's
//! turn on a thrown-away session is read from that session's place, which the sweep at start keeps while the
//! turn goes on and which is ended with the turn, while a job's turn no record names is reaped as before; a
//! turn on a Claude Code pane, which keeps no record of agency's, is read from the place's run, the hooks'
//! calls and the session's transcript, a job's and a supervisor's double's alike; and a job's turn sent on
//! a thrown-away session is kept in flight while it goes, whatever run the worker's own place holds, so the
//! start after its process went away adopts the job from the row its launch wrote.

use std::collections::HashMap;
use std::sync::Arc;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::work::reap_stale;
use agency_server::herdr_backend::TurnEnded;
use agency_server::lines::place::Place;
use agency_server::now_ms;
use agency_server::order_run::dispatch;
use agency_server::places::Ground;
use agency_server::turn_pickup::*;
use harness_providers::Machine;
use serde_json::json;

use crate::support::grounds::{deepseek_balance, job_slot, seeded, slot_now};
use crate::support::scratch::Scratch;

fn ground(scratch: &Scratch, home: &std::path::Path) -> Arc<Ground> {
  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), home.to_string_lossy().into_owned()),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
    ],
    home.join("data"),
  );

  Arc::new(Ground::new(&service, &machine, None))
}

// A line of a pane program's record, as bmo-tui appends it.
fn append(place: &Place, line: serde_json::Value) {
  use std::io::Write as _;

  std::fs::create_dir_all(&place.path).unwrap();

  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(place.path.join("loop.jsonl"))
    .unwrap();

  writeln!(file, "{line}").unwrap();
}

fn started(place: &Place, id: &str, name: &str) {
  append(
    place,
    json!({ "kind": "start", "id": id, "name": name, "title": "a turn", "at": 1, "typed": false }),
  );
}

fn answered(place: &Place, id: &str, text: &str) {
  append(
    place,
    json!({ "kind": "answer", "id": id, "text": text, "model": "[\"deepseek\",\"deepseek-flash\"]",
            "usage": { "inputTokens": 10, "outputTokens": 5, "cacheReadTokens": 0 } }),
  );
  append(place, json!({ "kind": "end", "id": id }));
}

fn in_flight(
  run: &str,
  agent: &str,
  turn: i64,
  chat_id: Option<i64>,
  ledger: Option<i64>,
  job: Option<i64>,
) -> InFlight {
  InFlight {
    session: format!("{agent}@worker"),
    turn,
    run: run.to_owned(),
    agent: agent.to_owned(),
    chat_id,
    ledger,
    job,
    started_at: now_ms() - 1_000,
  }
}

#[test]
fn a_kept_turn_stands_as_its_programs_record_says() {
  let data = tempfile::tempdir().unwrap();
  let place = Place::of(data.path(), "agent-a");
  let turn = in_flight("chat:5:1", "agent-a", 1, Some(5), None, None);

  assert_eq!(standing(&place, &turn), Standing::Lost, "no record names it");

  std::fs::create_dir_all(place.path.join("inbox")).unwrap();
  std::fs::write(
    place.path.join("inbox/000-t.json"),
    json!({ "id": "t", "name": "chat:5:1" }).to_string(),
  )
  .unwrap();
  assert_eq!(standing(&place, &turn), Standing::Going, "it waits in the inbox");

  std::fs::remove_file(place.path.join("inbox/000-t.json")).unwrap();
  started(&place, "t", "chat:5:1");
  assert_eq!(standing(&place, &turn), Standing::Going, "the program works on it");

  append(&place, json!({ "kind": "progress", "id": "t", "phase": "tool" }));
  answered(&place, "t", "the answer");

  match standing(&place, &turn) {
    Standing::Ended(Came::Answered(reply)) => {
      assert_eq!(reply.text, "the answer");
      assert_eq!(reply.mode, "deepseek");
    }
    other => panic!("{other:?}"),
  }

  // A later turn under another name does not hide this one's end.
  started(&place, "u", "chat:5:2");
  assert!(matches!(standing(&place, &turn), Standing::Ended(Came::Answered(_))));
}

#[test]
fn a_turn_put_in_after_others_under_the_same_name_stands_as_its_own_lines_say() {
  let data = tempfile::tempdir().unwrap();
  let place = Place::of(data.path(), "search-9-rounds");

  started(&place, "a", "research-9");
  answered(&place, "a", "the first round");
  assert!(matches!(
    standing_after(&place, "research-9", 0),
    Standing::Ended(Came::Answered(reply)) if reply.text == "the first round"
  ));
  assert_eq!(standing_after(&place, "research-9", 1), Standing::Lost, "the second never went in");

  std::fs::create_dir_all(place.path.join("inbox")).unwrap();
  std::fs::write(
    place.path.join("inbox/000-b.json"),
    json!({ "id": "b", "name": "research-9" }).to_string(),
  )
  .unwrap();
  assert_eq!(standing_after(&place, "research-9", 1), Standing::Going, "it waits in the inbox");
  assert_eq!(standing_after(&place, "research-9", 2), Standing::Lost, "no third went in");

  std::fs::remove_file(place.path.join("inbox/000-b.json")).unwrap();
  started(&place, "b", "research-9");
  assert_eq!(standing_after(&place, "research-9", 1), Standing::Going, "the program works on it");

  answered(&place, "b", "the final round");
  assert!(matches!(
    standing_after(&place, "research-9", 1),
    Standing::Ended(Came::Answered(reply)) if reply.text == "the final round"
  ));
}

#[tokio::test]
async fn a_job_whose_turn_ended_after_the_restart_ends_with_its_report_and_a_chat_turn_reaches_its_caller()
{
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path());
  let old = now_ms() - 20 * 60 * 1000;
  let worker = Place::of(&ground.roots.data, "worker-a");
  let chat = Place::of(&ground.roots.data, "agent-a");
  let mut ended = ground.events.subscribe::<TurnEnded>();

  crate::support::rows::repository(scratch.db(), "worker-a", "/checkouts/worker-a", &[]).await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
           "run_id", "run_name", "room_id", "state", "started_at", "prompt")
         VALUES (1, 0, 'harness', 60, 'worker-a', 600, 'job-6', 7, 'running', $1, 'six')"#,
      vec![old.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "runs" ("agent", "unit", "depth", "started_at", "id") VALUES
           ('worker-a', 'job-6', 0, $1, 600), ('agent-a', 'chat:5:1', 0, $1, 601)"#,
      vec![old.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "assignments" ("id", "item_id", "holder_name", "worker_name", "state", "created_at",
           "changed_at")
         VALUES (60, 1, 'harness', 'worker-a', 'running', $1, $1)"#,
      vec![now_ms().into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "agent_executions" ("id", "body") VALUES ('job-execution', $1)"#,
      vec![
        json!({
          "id": "job-execution", "agent": "worker-a", "line": "worker", "provider": "deepseek",
          "state": "running", "startedAt": old, "endedAt": null,
          "inboxTurn": { "id": "j", "name": "job-6" },
        })
        .to_string()
        .into(),
      ],
    )
    .await;

  let job_id = store::one(scratch.db(), r#"SELECT "id" FROM "jobs" WHERE "run_name" = 'job-6'"#, vec![])
    .await
    .unwrap()
    .unwrap()
    .try_get::<i64>("", "id")
    .unwrap();

  keep(scratch.db(), &in_flight("job-6", "worker-a", 41, Some(7), None, Some(job_id))).await;
  keep(
    scratch.db(),
    &InFlight {
      session: "agent-a".into(),
      ..in_flight("chat:5:1", "agent-a", 42, Some(5), Some(601), None)
    },
  )
  .await;
  started(&worker, "j", "job-6");
  answered(&worker, "j", "The job's report.");
  started(&chat, "c", "chat:5:1");
  answered(&chat, "c", "The room's answer.");

  // The pass that ends a job left running leaves one whose turn is kept to the pickup.
  assert_eq!(reap_stale(&ground).await.unwrap(), 0);
  assert_eq!(at_start(&ground).await.unwrap(), 2);

  let job = store::one(
    scratch.db(),
    r#"SELECT "state", "outcome", "report" FROM "jobs" WHERE "id" = $1"#,
    vec![job_id.into()],
  )
  .await
  .unwrap()
  .unwrap();

  assert_eq!(job.try_get::<String>("", "state").unwrap(), "ended");
  assert_eq!(job.try_get::<Option<String>>("", "outcome").unwrap(), None);
  assert_eq!(job.try_get::<String>("", "report").unwrap(), "The job's report.");

  let run = |id: i64| {
    let db = scratch.db();
    async move {
      store::one(db, r#"SELECT "finished_at", "outcome" FROM "runs" WHERE "id" = $1"#, vec![id.into()])
        .await
        .unwrap()
        .unwrap()
    }
  };

  assert_eq!(run(600).await.try_get::<String>("", "outcome").unwrap(), "answered");
  assert_eq!(run(601).await.try_get::<String>("", "outcome").unwrap(), "answered");

  let execution = store::one(
    scratch.db(),
    r#"SELECT "body" FROM "agent_executions" WHERE "id" = 'job-execution'"#,
    vec![],
  )
  .await
  .unwrap()
  .unwrap()
  .try_get::<String>("", "body")
  .unwrap();

  assert!(
    execution.contains("\"endedAt\":") && !execution.contains("\"endedAt\":null"),
    "{execution}"
  );

  let mut ends = vec![];

  while let Some(end) = ended.ready() {
    ends.push((end.session, end.end.turn, end.end.answer));
  }

  ends.sort();
  assert_eq!(
    ends,
    [
      ("agent-a".to_owned(), 42, Some("The room's answer.".to_owned())),
      ("worker-a@worker".to_owned(), 41, Some("The job's report.".to_owned())),
    ]
  );
  assert!(
    in_flight_turns(&scratch).await.is_empty(),
    "no turn stays kept once its end is handed back"
  );
}

async fn in_flight_turns(scratch: &Scratch) -> Vec<InFlight> {
  agency_server::turn_pickup::in_flight(scratch.db()).await.unwrap()
}

#[tokio::test]
async fn a_turn_still_working_is_waited_on_and_one_no_record_names_ends_failed() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path());
  let chat = Place::of(&ground.roots.data, "agent-b");
  let mut ended = ground.events.subscribe::<TurnEnded>();
  let mut program = std::process::Command::new("sleep").arg("30").spawn().unwrap();
  let identity = agent_host::identity::ProcessIdentity::of(program.id() as i32).unwrap();

  std::fs::create_dir_all(&chat.path).unwrap();
  std::fs::write(
    chat.path.join("running.json"),
    json!({ "identity": "one", "pid": identity.pid, "birth": identity.birth, "loop": identity,
            "starts": 1, "startedAt": now_ms(), "recent": [] })
    .to_string(),
  )
  .unwrap();
  keep(
    scratch.db(),
    &InFlight {
      session: "agent-b".into(),
      ..in_flight("chat:5:3", "agent-b", 43, Some(5), None, None)
    },
  )
  .await;
  keep(
    scratch.db(),
    &InFlight {
      session: "agent-c".into(),
      ..in_flight("chat:5:4", "agent-c", 44, Some(5), None, None)
    },
  )
  .await;
  started(&chat, "w", "chat:5:3");

  assert_eq!(at_start(&ground).await.unwrap(), 2);

  let lost = ended.next().await.unwrap();

  assert_eq!((lost.session.as_str(), lost.end.turn), ("agent-c", 44));
  assert!(lost.end.answer.is_none() && lost.end.reason.is_some(), "{lost:?}");

  answered(&chat, "w", "Worth the wait.");

  let waited = tokio::time::timeout(std::time::Duration::from_secs(10), ended.next())
    .await
    .unwrap()
    .unwrap();

  assert_eq!((waited.session.as_str(), waited.end.turn), ("agent-b", 43));
  assert_eq!(waited.end.answer.as_deref(), Some("Worth the wait."));

  let _ = program.kill();
  let _ = program.wait();
}

#[tokio::test]
async fn a_turn_whose_program_died_is_closed_and_its_room_freed() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path());
  let place = Place::of(&ground.roots.data, "agent-dead");
  let turn = in_flight("chat:7:1", "agent-dead", 7, Some(7), None, None);
  let mut ended = ground.events.subscribe::<TurnEnded>();

  std::fs::create_dir_all(&place.path).unwrap();
  std::fs::write(
    place.path.join("run.json"),
    json!({ "name": "chat:7:1", "title": "a turn", "person": false,
            "startedAt": now_ms() - 1_000, "endedAt": null })
    .to_string(),
  )
  .unwrap();
  let _ = store::run(
    scratch.db(),
    r#"INSERT INTO "agent_executions" ("id", "body") VALUES ('dead-execution', $1)"#,
    vec![
      json!({
        "id": "dead-execution", "agent": "agent-dead", "line": "chat", "provider": "deepseek",
        "startedAt": now_ms() - 1_000, "endedAt": null,
        "inboxTurn": { "name": "chat:7:1" },
        "resident": { "place": place.path.to_string_lossy(), "pid": 99999999, "birth": "gone" }
      })
      .to_string()
      .into(),
    ],
  )
  .await;
  keep(scratch.db(), &turn).await;

  assert_eq!(reap_dead(&ground).await, 1);
  assert!(in_flight_turns(&scratch).await.is_empty(), "the room's turn is freed");
  let run = agency_server::door::place_run::read_run(&place.path).unwrap();
  assert!(run.ended_at.is_some(), "the run carries its end");
  let end = ended.ready().expect("the turn's end is published");
  assert_eq!(end.session, "agent-dead@worker");
  assert!(matches!(end.end.outcome, agent_session::Outcome::Failed));
  assert!(end.end.reason.unwrap_or_default().contains("died silent"));
}

// A turn this process still carries is its own end's to close: its walk goes from one road to the next under
// the same run, and the place's run ends between them — a failed request ending the first road's turn — while
// the turn goes on (docs/sessions.md, 「A turn whose program dies」).
#[tokio::test]
async fn a_turn_this_process_carries_is_left_to_its_own_end_though_its_place_s_run_ended() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path());
  let place = Place::of(&ground.roots.data, "agent-walking");
  let turn = in_flight("chat:8:1", "agent-walking", 8, Some(8), None, None);
  let mut ended = ground.events.subscribe::<TurnEnded>();

  agency_server::door::place_run::begin_run(
    &place.path,
    "chat:8:1",
    "a turn",
    now_ms() - 1_000,
    Default::default(),
  )
  .unwrap();
  agency_server::door::place_run::record_call(
    &place.path,
    agency_server::lines::place::FAILURE_KIND,
    &json!({ "hook_event_name": "StopFailure", "error": "server_error",
             "last_assistant_message": "API Error: 503 Service is too busy" }),
    now_ms(),
    false,
    None,
  )
  .unwrap();
  assert!(agency_server::door::place_run::read_run(&place.path).unwrap().ended_at.is_some());
  keep(scratch.db(), &turn).await;

  let carrying = Carrying::of("chat:8:1");

  assert_eq!(reap_dead(&ground).await, 0);
  assert!(kept(scratch.db(), "chat:8:1").await, "the turn going on stays kept");
  assert!(ended.ready().is_none(), "nothing is handed back for it");

  // Once its end has gone, a run left ended in its place is no longer kept.
  drop(carrying);
  assert_eq!(reap_dead(&ground).await, 0);
  assert!(!kept(scratch.db(), "chat:8:1").await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lease_of_a_job_s_turn_adopted_at_start_stands_as_seen_and_is_given_back_when_the_turn_ends() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = Arc::new(seeded(scratch.db(), home.path()).await);
  let worker = Place::of(&ground.roots.data, "worker-a");
  let mut program = std::process::Command::new("sleep").arg("30").spawn().unwrap();
  let identity = agent_host::identity::ProcessIdentity::of(program.id() as i32).unwrap();

  deepseek_balance(&ground, 5.0).await;
  crate::support::rows::repository(scratch.db(), "worker-a", "/checkouts/worker-a", &[]).await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
           "run_name", "room_id", "state", "started_at", "prompt")
         VALUES (31, 1, 0, 'harness', 0, 'worker-a', 'job-31', 7, 'running', $1, 'adopted')"#,
      vec![(now_ms() - 60_000).into()],
    )
    .await;

  // The process before held the job's slot, bound to its run, and left its turn going in the program.
  let held = job_slot(&ground, 31, true).await;

  std::fs::create_dir_all(&worker.path).unwrap();
  std::fs::write(
    worker.path.join("running.json"),
    json!({ "identity": "one", "pid": identity.pid, "birth": identity.birth, "loop": identity,
            "starts": 1, "startedAt": now_ms(), "recent": [] })
    .to_string(),
  )
  .unwrap();
  keep(scratch.db(), &in_flight("job-31", "worker-a", 51, Some(7), None, Some(31))).await;
  started(&worker, "j", "job-31");

  assert_eq!(at_start(&ground).await.unwrap(), 1);

  // Adopted, its run stands as seen, and the job keeps its slot while the turn goes on.
  let leases = dispatch::bound_leases(&ground.db).await.unwrap();

  assert_eq!(leases, vec![(held.token.clone(), "job-31".to_owned(), true)]);
  assert_eq!(slot_now(&ground, &held.token).await.state, "active");

  // The turn ends in the program's record: the job ends with it and its slot is given back.
  answered(&worker, "j", "The adopted job's report.");

  let mut given = slot_now(&ground, &held.token).await;

  for _ in 0..80 {
    if given.state != "active" {
      break;
    }

    tokio::time::sleep(std::time::Duration::from_millis(125)).await;
    given = slot_now(&ground, &held.token).await;
  }

  assert_eq!(
    (given.state.as_str(), given.ended_by.as_deref()),
    ("released", Some(dispatch::BY_OWNER)),
    "{given:?}"
  );

  let job = store::one(scratch.db(), r#"SELECT "state" FROM "jobs" WHERE "id" = 31"#, vec![])
    .await
    .unwrap()
    .unwrap();

  assert_eq!(job.try_get::<String>("", "state").unwrap(), "ended");

  let _ = program.kill();
  let _ = program.wait();
}

// A job's row a test stands, running since `started_at` under the run `job-<id>` in worker-a's checkout.
async fn running_job(scratch: &Scratch, id: i64, started_at: i64) {
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
           "run_name", "room_id", "state", "started_at", "prompt")
         VALUES ($1, 1, 0, 'harness', 0, 'worker-a', $2, 7, 'running', $3, 'apart')"#,
      vec![id.into(), format!("job-{id}").into(), started_at.into()],
    )
    .await;
}

async fn job_state(scratch: &Scratch, id: i64) -> (String, Option<String>) {
  let row =
    store::one(scratch.db(), r#"SELECT "state", "report" FROM "jobs" WHERE "id" = $1"#, vec![id.into()])
      .await
      .unwrap()
      .unwrap();

  (row.try_get("", "state").unwrap(), row.try_get("", "report").unwrap())
}

// A thrown-away session's place as the process before left it, marked as the session's own.
fn apart_place(ground: &Ground, session: &str) -> (String, Place) {
  let id = agent_session::SessionId(session.to_owned());
  let name = agency_server::apart_places::place_name(&id);

  agency_server::apart_places::mark(ground, &name, &id).unwrap();

  (name.clone(), Place::of(&ground.roots.data, &name))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_s_turn_on_a_thrown_away_session_is_adopted_from_its_place_and_its_place_ends_with_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = Arc::new(seeded(scratch.db(), home.path()).await);
  let data = ground.roots.data.clone();
  let mut program = std::process::Command::new("sleep").arg("30").spawn().unwrap();
  let identity = agent_host::identity::ProcessIdentity::of(program.id() as i32).unwrap();

  deepseek_balance(&ground, 5.0).await;
  crate::support::rows::repository(scratch.db(), "worker-a", "/checkouts/worker-a", &[]).await;
  running_job(&scratch, 41, now_ms() - 20 * 60 * 1000).await;

  // The process before held the job's slot, bound to its run, and left its turn going in the program of
  // the session's own place; another session's place it left carries no turn.
  let held = job_slot(&ground, 41, true).await;
  let (adopted, place) = apart_place(&ground, "worker-a@run:13");
  let (idle, _) = apart_place(&ground, "worker-a@run:12");

  std::fs::write(
    place.path.join("running.json"),
    json!({ "identity": "one", "pid": identity.pid, "birth": identity.birth, "loop": identity,
            "starts": 1, "startedAt": now_ms(), "recent": [] })
    .to_string(),
  )
  .unwrap();
  keep(
    scratch.db(),
    &InFlight {
      session: "worker-a@run:13".into(),
      ..in_flight("job-41", "worker-a", 61, Some(7), None, Some(41))
    },
  )
  .await;
  started(&place, "j", "job-41");

  assert_eq!(place_name_of(&in_flight_turns(&scratch).await[0]), adopted);
  assert_eq!(at_start(&ground).await.unwrap(), 1);

  // The sweep at start ends the place carrying no turn and keeps the adopted one.
  assert_eq!(agency_server::apart_places::sweep(&ground).await, 1);
  assert!(!agency_server::apart_places::is_apart_place(&data, &idle), "the idle place is ended");
  assert!(
    agency_server::apart_places::is_apart_place(&data, &adopted),
    "the adopted place is kept"
  );

  // The pass ending a job left running leaves the adopted one: it stays running with its slot, seen.
  assert_eq!(reap_stale(&ground).await.unwrap(), 0);
  assert_eq!(job_state(&scratch, 41).await.0, "running");
  assert_eq!(
    dispatch::bound_leases(&ground.db).await.unwrap(),
    vec![(held.token.clone(), "job-41".to_owned(), true)]
  );
  assert_eq!(slot_now(&ground, &held.token).await.state, "active");

  // The turn ends in its place's record: the job ends through its own end path, its slot is given back
  // and the session's place is ended with it.
  answered(&place, "j", "The thrown-away session's report.");

  for _ in 0..80 {
    if !agency_server::apart_places::is_apart_place(&data, &adopted) {
      break;
    }

    tokio::time::sleep(std::time::Duration::from_millis(125)).await;
  }

  assert!(
    !agency_server::apart_places::is_apart_place(&data, &adopted),
    "the place ends with its turn"
  );
  assert_eq!(
    job_state(&scratch, 41).await,
    ("ended".to_owned(), Some("The thrown-away session's report.".to_owned()))
  );

  let given = slot_now(&ground, &held.token).await;

  assert_eq!(
    (given.state.as_str(), given.ended_by.as_deref()),
    ("released", Some(dispatch::BY_OWNER)),
    "{given:?}"
  );
  assert!(in_flight_turns(&scratch).await.is_empty());

  let _ = program.kill();
  let _ = program.wait();
}

#[tokio::test]
async fn a_job_s_turn_on_a_thrown_away_session_no_record_names_is_reaped_and_its_place_ended() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = Arc::new(seeded(scratch.db(), home.path()).await);
  let data = ground.roots.data.clone();

  crate::support::rows::repository(scratch.db(), "worker-a", "/checkouts/worker-a", &[]).await;
  running_job(&scratch, 42, now_ms() - 20 * 60 * 1000).await;

  let (name, _) = apart_place(&ground, "worker-a@run:14");

  keep(
    scratch.db(),
    &InFlight {
      session: "worker-a@run:14".into(),
      ..in_flight("job-42", "worker-a", 62, Some(7), None, Some(42))
    },
  )
  .await;

  assert_eq!(at_start(&ground).await.unwrap(), 1);
  assert!(in_flight_turns(&scratch).await.is_empty(), "the lost job's turn is left to the reap");
  assert_eq!(agency_server::apart_places::sweep(&ground).await, 1);
  assert!(!agency_server::apart_places::is_apart_place(&data, &name), "its place is ended");
  assert_eq!(reap_stale(&ground).await.unwrap(), 1);

  let (state, report) = job_state(&scratch, 42).await;

  assert_eq!(state, "stopped");
  assert!(report.unwrap_or_default().starts_with(agency_server::control::work::REAPED));
}

// A moment as a Claude Code transcript stamps its lines.
fn stamp(at: i64) -> String {
  chrono::DateTime::from_timestamp_millis(at).unwrap().to_rfc3339()
}

// Lines appended to a Claude Code session's transcript.
fn transcribe(transcript: &std::path::Path, lines: &[serde_json::Value]) {
  use std::io::Write as _;

  let mut file = std::fs::OpenOptions::new().create(true).append(true).open(transcript).unwrap();

  for line in lines {
    writeln!(file, "{line}").unwrap();
  }
}

// A Claude Code pane's place as the process before left it with a turn going: the place's run naming the
// turn, the program standing in it, the hooks' start and submit, and the session's transcript holding the
// prompt. Answers the transcript's path and the hooks' payload.
fn claude_code_turn(
  place: &Place,
  run: &str,
  began: i64,
  program: u32,
) -> (std::path::PathBuf, serde_json::Value) {
  use agency_server::door::place_run::{begin_run, record_call};

  let identity = agent_host::identity::ProcessIdentity::of(program as i32).unwrap();
  let transcript = place.path.join("session-s.jsonl");
  let payload = json!({ "session_id": "s", "transcript_path": transcript });

  begin_run(&place.path, run, "a turn", began, Default::default()).unwrap();
  std::fs::write(
    place.path.join("running.json"),
    json!({ "identity": "one", "pid": identity.pid, "birth": identity.birth, "loop": identity,
            "starts": 1, "startedAt": began, "recent": [], "paneId": "w1:p2" })
    .to_string(),
  )
  .unwrap();
  record_call(&place.path, "start", &payload, began + 1, false, None).unwrap();
  record_call(&place.path, "submit", &payload, began + 2, false, None).unwrap();
  transcribe(
    &transcript,
    &[json!({ "type": "user", "timestamp": stamp(began + 3), "message": { "content": "the work" } })],
  );

  (transcript, payload)
}

// The turn's answer written to the transcript and its stop recorded by the hooks, which closes the run.
fn claude_code_answers(
  place: &Place,
  transcript: &std::path::Path,
  payload: &serde_json::Value,
  text: &str,
) {
  let at = now_ms();

  transcribe(
    transcript,
    &[
      json!({ "type": "assistant", "timestamp": stamp(at),
              "message": { "id": "m1", "model": "deepseek-flash",
                           "content": [{ "type": "text", "text": text }] } }),
      json!({ "type": "system", "subtype": "turn_duration", "timestamp": stamp(at + 1) }),
    ],
  );
  agency_server::door::place_run::record_call(&place.path, "stop", payload, at + 2, false, None).unwrap();
}

#[test]
fn a_claude_code_pane_s_turn_stands_as_its_place_s_run_and_hooks_say() {
  let data = tempfile::tempdir().unwrap();
  let place = Place::of(data.path(), "apart-worker-a-run-20");
  let turn = InFlight {
    session: "worker-a@run:20".into(),
    ..in_flight("job-20", "worker-a", 1, None, None, Some(20))
  };
  let began = now_ms() - 5_000;
  let transcript = place.path.join("session-s.jsonl");
  let payload = json!({ "session_id": "s", "transcript_path": transcript });

  std::fs::create_dir_all(&place.path).unwrap();
  assert_eq!(standing(&place, &turn), Standing::Lost, "no run names it");

  agency_server::door::place_run::begin_run(&place.path, "job-20", "a turn", began, Default::default())
    .unwrap();
  assert_eq!(standing(&place, &turn), Standing::Lost, "the program never took it");

  agency_server::door::place_run::record_call(&place.path, "submit", &payload, began + 1, false, None)
    .unwrap();
  assert_eq!(standing(&place, &turn), Standing::Going, "taken and not closed");

  claude_code_answers(&place, &transcript, &payload, "The report.");

  match standing(&place, &turn) {
    Standing::Ended(Came::Answered(reply)) => {
      assert_eq!(reply.text, "The report.");
      assert_eq!(reply.session_id.as_deref(), Some("s"));
    }
    other => panic!("{other:?}"),
  }

  let other = InFlight {
    run: "job-21".into(),
    ..turn.clone()
  };

  assert_eq!(standing(&place, &other), Standing::Lost, "the place's run is another turn's");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_s_turn_on_a_claude_code_pane_is_adopted_across_the_start_and_ends_through_the_job_s_end() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = Arc::new(seeded(scratch.db(), home.path()).await);
  let data = ground.roots.data.clone();
  let mut program = std::process::Command::new("sleep").arg("30").spawn().unwrap();

  deepseek_balance(&ground, 5.0).await;
  crate::support::rows::repository(scratch.db(), "worker-a", "/checkouts/worker-a", &[]).await;
  running_job(&scratch, 43, now_ms() - 20 * 60 * 1000).await;

  let held = job_slot(&ground, 43, true).await;
  let (name, place) = apart_place(&ground, "worker-a@run:16");
  let (transcript, payload) = claude_code_turn(&place, "job-43", now_ms() - 60_000, program.id());

  // The door's row of the turn, named by its liveness row, which the last process left open.
  scratch
    .run(
      r#"INSERT INTO "agent_executions" ("id", "body") VALUES ('door-43', $1)"#,
      vec![
        json!({ "id": "door-43", "agent": name, "line": "worker", "provider": "deepseek",
                "state": "running", "startedAt": now_ms() - 60_000, "endedAt": null, "paneId": "w1:p2",
                "resident": { "place": place.path, "pid": program.id() } })
        .to_string()
        .into(),
      ],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "turn_liveness" ("execution", "run", "line", "session", "phase", "opened_at",
           "written_at", "ending", "ended_at")
         VALUES ('door-43', 'job-43', 'worker', 's', 'working', $1, $1, 'restarted', $1)"#,
      vec![now_ms().into()],
    )
    .await;
  keep(
    scratch.db(),
    &InFlight {
      session: "worker-a@run:16".into(),
      ..in_flight("job-43", "worker-a", 63, Some(7), None, Some(43))
    },
  )
  .await;

  assert_eq!(at_start(&ground).await.unwrap(), 1);
  assert_eq!(agency_server::apart_places::sweep(&ground).await, 0, "the adopted place is kept");
  assert!(agency_server::apart_places::is_apart_place(&data, &name));
  assert_eq!(agency_server::queue::reconcile::pass(scratch.db(), &data).await.unwrap(), 0);
  assert_eq!(reap_stale(&ground).await.unwrap(), 0);
  assert_eq!(job_state(&scratch, 43).await.0, "running", "the job goes on across the start");
  assert_eq!(slot_now(&ground, &held.token).await.state, "active");

  // The program answers and its hooks close the run: the job ends with the answer as its report.
  claude_code_answers(&place, &transcript, &payload, "The Claude Code job's report.");

  for _ in 0..80 {
    if !agency_server::apart_places::is_apart_place(&data, &name) {
      break;
    }

    tokio::time::sleep(std::time::Duration::from_millis(125)).await;
  }

  assert!(
    !agency_server::apart_places::is_apart_place(&data, &name),
    "the place ends with its turn"
  );
  assert_eq!(
    job_state(&scratch, 43).await,
    ("ended".to_owned(), Some("The Claude Code job's report.".to_owned()))
  );
  assert_eq!(slot_now(&ground, &held.token).await.state, "released");

  let door =
    store::one(scratch.db(), r#"SELECT "body" FROM "agent_executions" WHERE "id" = 'door-43'"#, vec![])
      .await
      .unwrap()
      .unwrap()
      .try_get::<String>("", "body")
      .unwrap();

  assert!(!door.contains("\"endedAt\":null"), "the door's row is closed with the turn: {door}");

  let _ = program.kill();
  let _ = program.wait();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_supervisor_s_double_s_turn_on_a_claude_code_pane_is_adopted_and_its_end_reaches_its_session() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = Arc::new(seeded(scratch.db(), home.path()).await);
  let data = ground.roots.data.clone();
  let mut program = std::process::Command::new("sleep").arg("30").spawn().unwrap();
  let mut ended = ground.events.subscribe::<TurnEnded>();
  let (name, place) = apart_place(&ground, "double-a@run:1");
  let (transcript, payload) = claude_code_turn(&place, "delegate:9", now_ms() - 60_000, program.id());

  keep(
    scratch.db(),
    &InFlight {
      session: "double-a@run:1".into(),
      ..in_flight("delegate:9", "double-a", 71, None, None, None)
    },
  )
  .await;

  assert_eq!(at_start(&ground).await.unwrap(), 1);
  assert_eq!(agency_server::apart_places::sweep(&ground).await, 0);
  assert!(ended.ready().is_none(), "nothing ends while the turn goes on");

  claude_code_answers(&place, &transcript, &payload, "Delegated and done.");

  let end = tokio::time::timeout(std::time::Duration::from_secs(10), ended.next())
    .await
    .unwrap()
    .unwrap();

  assert_eq!((end.session.as_str(), end.end.turn), ("double-a@run:1", 71));
  assert_eq!(end.end.answer.as_deref(), Some("Delegated and done."));

  for _ in 0..80 {
    if !agency_server::apart_places::is_apart_place(&data, &name) {
      break;
    }

    tokio::time::sleep(std::time::Duration::from_millis(125)).await;
  }

  assert!(
    !agency_server::apart_places::is_apart_place(&data, &name),
    "the place ends with its turn"
  );

  let _ = program.kill();
  let _ = program.wait();
}

// A job's turn's work parts as the job line names them, under the job's run name.
fn job_work(cwd: &str, run: &str) -> agent_session::WorkParts {
  agent_session::WorkParts {
    provider: "claude-code".into(),
    model: "claude-sonnet-5".into(),
    mapping: agent_session::Mapping {
      model: "claude-sonnet-5".into(),
      effort: None,
      tier: None,
      answer_tokens: None,
    },
    resume: agent_session::Resume::Fresh,
    cwd: cwd.into(),
    name: run.into(),
    title: "a job".into(),
    chat: 0,
    unit: None,
    user: None,
    work_key: "worker".into(),
    hears_words: false,
    variables: Default::default(),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_s_turn_sent_is_kept_in_flight_while_it_goes_and_the_pickup_adopts_the_row_its_launch_wrote()
{
  use agency_server::herdr_backend::HerdrSessions;
  use agency_server::places::Places;
  use agent_session::{Apart, JobMark, Scope, SessionId, Sessions, Start, Turn, TurnContext};

  use crate::integration::common;

  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let line = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&line, 5.0).await;
  crate::support::rows::repository(scratch.db(), "worker", "/checkouts/worker", &[]).await;

  let config = home.path().join("config");

  std::fs::create_dir_all(config.join("herdr")).unwrap();

  let herdr = common::herdr::HerdrModel::stand(config.join("herdr").join("herdr.sock"));
  let at = |path: &std::path::Path| path.to_string_lossy().into_owned();
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), at(home.path())),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
      ("XDG_CONFIG_HOME".to_owned(), at(&config)),
      ("BMO_DEPLOY_DIR".to_owned(), env!("CARGO_MANIFEST_DIR").to_owned()),
    ],
    home.path().join("data"),
  );
  let service = common::service(&scratch, &[("AGENCY_SOURCE_DIR", at(&home.path().join("source")))]);
  let places = Places::new(
    Ground::new(&service, &machine, Some(herdr::Herdr::there(herdr.socket.clone()))),
    Arc::default(),
  );
  let ground = places.ground.clone();
  let sessions = Arc::new(HerdrSessions::new(places));

  // The worker's own place holds the run of an earlier job, long ended, which says nothing of a turn on a
  // thrown-away session.
  agency_server::door::place_run::begin_run(
    &Place::of(&ground.roots.data, "worker").path,
    "job-70",
    "an earlier job",
    now_ms() - 3_600_000,
    Default::default(),
  )
  .unwrap();
  agency_server::door::place_run::end_run(
    &Place::of(&ground.roots.data, "worker").path,
    now_ms() - 3_000_000,
  )
  .unwrap();

  // A job's turn as the job line sends it, on a thrown-away session of its role, on the provider named.
  let turn_of = async |job: i64, provider: &str| {
    let started = sessions
      .start(Start {
        agent: "worker".into(),
        scope: Scope::Apart(Apart {
          provider: provider.into(),
          model: "claude-sonnet-5".into(),
          cwd: at(home.path()),
          role: Some("worker".into()),
          ..Apart::default()
        }),
      })
      .await
      .unwrap();
    let mut work = job_work(&at(home.path()), &format!("job-{job}"));

    work.provider = provider.into();

    let turn = Turn {
      session: started.id.clone(),
      prompt: "work".into(),
      context: TurnContext {
        job: Some(JobMark { job, item: Some(1) }),
        work: Some(work),
        ..TurnContext::default()
      },
    };

    (started.id, turn)
  };

  // A job's turn that ends: kept while it went, as its start's event says, and no longer kept once ended.
  let mut begun = ground.events.subscribe::<agency_server::herdr_backend::TurnStarted>();
  let (ended_session, turn) = turn_of(79, "codex").await;
  let ended = tokio::time::timeout(std::time::Duration::from_secs(20), sessions.send_turn(turn)).await;

  assert!(ended.is_ok(), "the turn on a road that answers nothing ends");
  sessions.end(&ended_session).await.unwrap();
  assert_eq!(begun.ready().map(|started| started.run).as_deref(), Some("job-79"), "it went out");
  assert!(!kept_now(&scratch, "job-79").await, "a turn that ended is no longer kept");

  // A job's turn that goes on: its row stands under its run with the job, the session and no room, and the
  // queue's pass leaves it there although the worker's own place holds another run.
  running_job(&scratch, 78, now_ms() - 20 * 60 * 1000).await;

  let held = job_slot(&ground, 78, true).await;
  let (session, turn) = turn_of(78, "claude-code").await;
  let sending = sessions.clone();
  let going = tokio::spawn(async move { sending.send_turn(turn).await });

  for _ in 0..200 {
    if sessions.turn_run(&session).is_some() || going.is_finished() {
      break;
    }

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
  }

  assert!(!going.is_finished(), "the job's turn goes on");

  let kept = in_flight_turns(&scratch).await;
  let row = kept.iter().find(|turn| turn.run == "job-78").expect("the job's turn is kept in flight");

  assert_eq!((row.job, row.chat_id, row.session.as_str()), (Some(78), None, session.0.as_str()));
  assert_eq!(row.agent, "worker");
  assert_eq!(reap_dead(&ground).await, 0);
  assert!(kept_now(&scratch, "job-78").await, "the queue's pass leaves a running job's turn kept");

  // The process carrying it goes away under it, leaving the row its launch wrote; the next start adopts the
  // job from that row, its session's place standing as a Claude Code pane's with the turn going.
  going.abort();
  let _ = going.await;

  let left = in_flight_turns(&scratch).await;
  let row = left
    .iter()
    .find(|turn| turn.run == "job-78")
    .expect("the launch's row outlives its process");
  let mut program = std::process::Command::new("sleep").arg("30").spawn().unwrap();
  let name = place_name_of(row);

  agency_server::apart_places::mark(&ground, &name, &SessionId(row.session.clone())).unwrap();

  let place = Place::of(&ground.roots.data, &name);
  let (transcript, payload) = claude_code_turn(&place, "job-78", now_ms() - 60_000, program.id());

  assert_eq!(at_start(&ground).await.unwrap(), 1);
  assert_eq!(agency_server::apart_places::sweep(&ground).await, 0, "the adopted place is kept");
  assert!(agency_server::apart_places::is_apart_place(&ground.roots.data, &name));
  assert_eq!(reap_stale(&ground).await.unwrap(), 0);
  assert_eq!(job_state(&scratch, 78).await.0, "running", "the job goes on across the start");
  assert_eq!(slot_now(&ground, &held.token).await.state, "active");

  claude_code_answers(&place, &transcript, &payload, "The relaunched job's report.");

  for _ in 0..80 {
    if job_state(&scratch, 78).await.0 != "running" {
      break;
    }

    tokio::time::sleep(std::time::Duration::from_millis(125)).await;
  }

  assert_eq!(
    job_state(&scratch, 78).await,
    ("ended".to_owned(), Some("The relaunched job's report.".to_owned()))
  );
  assert!(!kept_now(&scratch, "job-78").await);

  let _ = program.kill();
  let _ = program.wait();
}

async fn kept_now(scratch: &Scratch, run: &str) -> bool {
  agency_server::turn_pickup::kept(scratch.db(), run).await
}
