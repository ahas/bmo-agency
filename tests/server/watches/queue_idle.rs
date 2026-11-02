//! The idle queue's alarm (docs/agent-control.md, 「An idle queue is told to the owner」): approved work a
//! free worker could take and no job started for the idle minutes, told to the owner once for each stretch
//! through the receiver, and silenced by the owner's switch until the next job starts.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use agency_server::watches::queue_idle::{
  MINUTES_KEY, Open, SEEN_PREFIX, SWITCH_KEY, Standing, awaiting_launch, idle, job_started,
  launch_instruction, launches_due, notice, pass, room_of, waiting,
};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::Uri;
use axum::response::IntoResponse;
use policy::queue_idle::QUEUE_IDLE;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::scratch::Scratch;

fn open(id: i64, assignee: Option<&str>) -> Open {
  Open {
    id,
    title: format!("item {id}"),
    assignee: assignee.map(str::to_owned),
    room: None,
  }
}

fn standing(open: Vec<Open>, free: &[&str]) -> Standing {
  Standing {
    open,
    free: free.iter().map(|name| (*name).to_owned()).collect(),
    last_start: Some(1_000),
    told: None,
    silenced: false,
    minutes: 5,
  }
}

const MINUTE: i64 = 60_000;

#[test]
fn work_a_free_worker_could_take_with_no_job_started_for_the_minutes_is_idle() {
  let quiet = standing(vec![open(1, Some("w1"))], &["w1"]);

  assert!(!idle(&quiet, 1_000 + 4 * MINUTE), "not yet the idle minutes");
  assert!(idle(&quiet, 1_000 + 5 * MINUTE));
}

#[test]
fn an_item_for_a_busy_worker_waits_on_nobody_and_one_naming_none_waits_on_any_free_worker() {
  let busy = standing(vec![open(1, Some("w2"))], &["w1"]);

  assert!(waiting(&busy).is_empty());
  assert!(!idle(&busy, 1_000 + 10 * MINUTE));

  let anyone = standing(vec![open(1, Some("w2")), open(2, None)], &["w1"]);

  assert_eq!(waiting(&anyone), vec![&open(2, None)]);

  let nobody_free = standing(vec![open(2, None)], &[]);

  assert!(!idle(&nobody_free, 1_000 + 10 * MINUTE));
}

#[test]
fn an_idle_stretch_is_told_once_and_the_switch_silences_it() {
  let mut told = standing(vec![open(1, Some("w1"))], &["w1"]);

  told.told = Some(1_000 + 6 * MINUTE);
  assert!(!idle(&told, 1_000 + 20 * MINUTE), "told since the last job started");

  told.last_start = Some(1_000 + 7 * MINUTE);
  assert!(idle(&told, 1_000 + 20 * MINUTE), "a job started since, a new stretch");

  let mut silenced = standing(vec![open(1, Some("w1"))], &["w1"]);

  silenced.silenced = true;
  assert!(!idle(&silenced, 1_000 + 20 * MINUTE));
}

#[test]
fn an_item_waiting_a_minute_on_its_own_free_worker_is_due_once_and_the_switch_silences_it() {
  let mut quiet = standing(vec![open(1, Some("w1")), open(2, Some("w2")), open(3, None)], &["w1"]);

  assert_eq!(
    awaiting_launch(&quiet),
    vec![&open(1, Some("w1"))],
    "its own assignee free, and only that"
  );

  let seen = BTreeMap::from([(1, 10_000)]);
  let none = BTreeSet::new();

  assert!(launches_due(&quiet, &seen, &none, 10_000 + MINUTE - 1).is_empty(), "not yet a minute");
  assert_eq!(launches_due(&quiet, &seen, &none, 10_000 + MINUTE), vec![&open(1, Some("w1"))]);
  assert!(
    launches_due(&quiet, &seen, &BTreeSet::from([1]), 10_000 + MINUTE).is_empty(),
    "told once"
  );
  assert!(
    launches_due(&quiet, &BTreeMap::new(), &none, 10_000 + MINUTE).is_empty(),
    "never seen waiting"
  );

  quiet.silenced = true;
  assert!(launches_due(&quiet, &seen, &none, 10_000 + MINUTE).is_empty(), "silenced");
}

#[test]
fn an_item_awaiting_a_launch_is_told_in_the_owner_s_room_with_the_three_calls() {
  let told = launch_instruction(&open(2265, Some("w1")));
  let order = told["said"]["order"].as_str().unwrap();

  assert_eq!(told["room"], Value::Null, "an item naming no room is the owner's private room's");

  // An item asked in a room is told in that room: its roomId, else the room its scope names.
  let asked = Open {
    room: room_of(&json!({ "scope": "-9000000000003" })),
    ..open(2268, Some("w1"))
  };

  assert_eq!(launch_instruction(&asked)["room"], json!(-9_000_000_000_003_i64));
  assert_eq!(room_of(&json!({ "roomId": 5, "scope": "-7" })), Some(5));
  assert_eq!(room_of(&json!({ "scope": "" })), None);
  assert_eq!(told["events"], json!(["item-awaits-launch:#2265"]));

  for word in ["#2265", "w1", "job_lease_next", "assign_worker", "run_job", "why not"] {
    assert!(order.contains(word), "{order}");
  }
}

#[test]
fn the_notice_carries_its_code_the_waiting_items_and_what_the_chat_agent_is_asked() {
  let said = notice(&standing(vec![open(1, Some("w1")), open(2, Some("w2"))], &["w1"]));

  assert_eq!(said["code"], QUEUE_IDLE.code.as_str());
  assert_eq!(said["openItems"], json!([{ "id": 1, "title": "item 1", "assignee": "w1" }]));
  assert_eq!(said["freeWorkers"], json!(["w1"]));
  assert_eq!(said["idleMinutes"], 5);
  assert_eq!(said["ask"], QUEUE_IDLE.ask.as_str());
}

type Calls = Arc<Mutex<Vec<(String, Value)>>>;

// The job service and the receiver in one: two queues, one paused, and the open items across them; every
// call kept.
async fn stand_in(State(calls): State<Calls>, uri: Uri, body: Bytes) -> impl IntoResponse {
  calls
    .lock()
    .unwrap()
    .push((uri.path().to_owned(), serde_json::from_slice(&body).unwrap_or(Value::Null)));

  match uri.path() {
    "/queues" => axum::Json(json!({
      "items": [
        { "name": "todo", "state": "open", "holds": [] },
        { "name": "later", "state": "paused", "holds": [] },
      ],
      "more": false,
    })),
    "/items" => axum::Json(json!({
      "items": [
        { "id": 11, "title": "for the free worker", "state": "open", "queueName": "todo", "assigneeName": "w1" },
        { "id": 12, "title": "for the busy worker", "state": "open", "queueName": "todo", "assigneeName": "w2" },
        { "id": 13, "title": "in a paused queue", "state": "open", "queueName": "later", "assigneeName": "w1" },
        { "id": 14, "title": "opened by its lane", "state": "open", "queueName": "todo", "assigneeName": "w1",
          "labels": { "lane": "pr-review" }, "scope": "-9000000000003" },
      ],
      "more": false,
    })),
    _ => axum::Json(json!({ "id": 1 })),
  }
}

#[tokio::test]
async fn a_pass_tells_the_owner_once_of_the_items_a_free_worker_could_take() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let calls: Calls = Arc::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(stand_in).with_state(calls.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let vars: BTreeMap<String, String> = [
    ("BMO_JOB_URL".to_owned(), address.clone()),
    ("BMO_JOB_TOKEN".to_owned(), "job-token".to_owned()),
    ("BMO_RECEIVER_URL".to_owned(), address.clone()),
    ("BMO_RECEIVER_TOKEN".to_owned(), "receiver-token".to_owned()),
  ]
  .into();

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind") VALUES ('w1', 'worker'), ('w2', 'worker'), ('chat', 'chat')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
           "started_at", "prompt")
         VALUES (12, 1, 'runner', 1, 'w2', 'running', 1000, 'the work')"#,
      vec![],
    )
    .await;

  let told = |calls: &Calls| -> Vec<Value> {
    calls
      .lock()
      .unwrap()
      .iter()
      .filter(|(path, _)| path == "/owner/notices")
      .map(|(_, body)| body.clone())
      .collect()
  };

  pass(db, &vars).await.unwrap();

  // The item on its own free worker is first seen waiting; told to the chat agent once a minute has passed.
  let instructed = |calls: &Calls| -> Vec<Value> {
    calls
      .lock()
      .unwrap()
      .iter()
      .filter(|(path, _)| path == "/instructions")
      .map(|(_, body)| body.clone())
      .collect()
  };

  assert!(instructed(&calls).is_empty(), "not yet a minute");
  scratch
    .run(
      r#"UPDATE "settings" SET "value" = $1 WHERE "key" = $2"#,
      vec![(agency_server::now_ms() - 61_000).to_string().into(), format!("{SEEN_PREFIX}11").into()],
    )
    .await;
  scratch
    .run(
      r#"UPDATE "settings" SET "value" = $1 WHERE "key" = $2"#,
      vec![(agency_server::now_ms() - 61_000).to_string().into(), format!("{SEEN_PREFIX}14").into()],
    )
    .await;
  pass(db, &vars).await.unwrap();
  pass(db, &vars).await.unwrap();

  let launches = instructed(&calls);

  // An item its unattended lane opened is told as an approved one is, in the room it was asked in.
  assert_eq!(launches.len(), 2, "each told once: {launches:?}");
  assert_eq!(launches[0]["events"], json!(["item-awaits-launch:#11"]));
  assert_eq!(
    (&launches[1]["events"], &launches[1]["room"]),
    (&json!(["item-awaits-launch:#14"]), &json!(-9_000_000_000_003_i64))
  );

  let notices = told(&calls);

  assert_eq!(notices.len(), 1, "{notices:?}");
  assert_eq!(notices[0]["code"], QUEUE_IDLE.code.as_str());
  assert_eq!(
    notices[0]["openItems"],
    json!([
      { "id": 11, "title": "for the free worker", "assignee": "w1" },
      { "id": 14, "title": "opened by its lane", "assignee": "w1" },
    ])
  );
  assert_eq!(notices[0]["freeWorkers"], json!(["w1"]));

  pass(db, &vars).await.unwrap();
  assert_eq!(told(&calls).len(), 1, "one stretch is told once");

  // A job that starts opens a new stretch; the owner's switch silences it until the next one starts.
  scratch
    .run(
      r#"UPDATE "jobs" SET "started_at" = 2000, "state" = 'ended' WHERE "worker_name" = 'w2'"#,
      vec![],
    )
    .await;
  scratch
    .run(r#"UPDATE "settings" SET "value" = '1500' WHERE "key" = 'queue_idle_told'"#, vec![])
    .await;
  scratch
    .run(r#"INSERT INTO "settings" ("key", "value") VALUES ($1, 'off')"#, vec![SWITCH_KEY.into()])
    .await;

  pass(db, &vars).await.unwrap();
  assert_eq!(told(&calls).len(), 1, "silenced");

  job_started(db).await;
  pass(db, &vars).await.unwrap();
  assert_eq!(told(&calls).len(), 2, "the switch on again, a new stretch told");

  // The owner's own minutes hold over the policy's.
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ($1, '100000000')"#,
      vec![MINUTES_KEY.into()],
    )
    .await;
  scratch
    .run(
      r#"UPDATE "jobs" SET "started_at" = $1"#,
      vec![(agency_server::now_ms() - 60_000).into()],
    )
    .await;
  scratch.run(r#"DELETE FROM "settings" WHERE "key" = 'queue_idle_told'"#, vec![]).await;

  pass(db, &vars).await.unwrap();
  assert_eq!(told(&calls).len(), 2, "not yet the owner's minutes");
}
