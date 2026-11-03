//! A turn in flight across agency's own restart, picked up again (docs/sessions.md, 「A restart hands the
//! session over」): a job whose turn ended in its program's record after the process that sent it went away
//! ends with its report rather than stopped, a chat turn's end reaches its caller under its own id with its
//! run closed, a turn its program is still working on is waited on, and one no record names ends failed; a
//! job's turn adopted at start stands as seen, its slot kept while it goes and given back as it ends; a job's
//! turn on a thrown-away session is read from that session's place, which the sweep at start keeps while the
//! turn goes on and which is ended with the turn, while a job's turn no record names is reaped as before.

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
