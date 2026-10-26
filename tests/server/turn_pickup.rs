//! A turn in flight across agency's own restart, picked up again (docs/sessions.md, 「A restart hands the
//! session over」): a job whose turn ended in its program's record after the process that sent it went away
//! ends with its report rather than stopped, a chat turn's end reaches its caller under its own id with its
//! run closed, a turn its program is still working on is waited on, and one no record names ends failed.

use std::collections::HashMap;
use std::sync::Arc;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::work::reap_stale;
use agency_server::herdr_backend::TurnEnded;
use agency_server::lines::place::Place;
use agency_server::now_ms;
use agency_server::places::Ground;
use agency_server::turn_pickup::*;
use harness_providers::Machine;
use serde_json::json;

use crate::scratch::Scratch;

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

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "node") VALUES ('worker-a', 'worker', NULL)"#,
      vec![],
    )
    .await;
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
      r#"INSERT INTO "assignments" ("id", "item_id", "holder_name", "worker_name", "state", "expires_at",
           "created_at", "changed_at")
         VALUES (60, 1, 'harness', 'worker-a', 'running', $1, $1, $1)"#,
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
