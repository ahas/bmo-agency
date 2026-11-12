//! The idle queue's alarm (docs/agent-control.md, 「An idle queue is told to the owner」): approved work a
//! free worker could take and no job started for the idle minutes, told to the owner once for each stretch
//! through the receiver, and silenced by the owner's switch until the next job starts; and a draft awaiting
//! approval told once to the supervisor that ordered it, in its room or its parent's up the `raised-by` chain;
//! and a held item every item it waits on has ended told once to its orderer while it stays held.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use agency_server::queue::orderer::Orderer;
use agency_server::watches::queue_idle::{
  DRAFT_TOLD_PREFIX, Draft, LAUNCH_OWED, MINUTES_KEY, Open, SEEN_PREFIX, SWITCH_KEY, Standing,
  approval_instruction, awaiting_launch, brief_head, draft_of, drafts_due, idle, job_started,
  launch_instruction, launches_due, notice, pass, room_of, unlaunchable, waiting,
};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::Uri;
use axum::response::IntoResponse;
use policy::queue_idle::QUEUE_IDLE;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::rows::{repository, supervisor};
use crate::support::scratch::Scratch;
use crate::support::wording::assert_no_instruction;

fn open(id: i64, assignee: Option<&str>) -> Open {
  Open {
    id,
    title: format!("item {id}"),
    assignee: assignee.map(str::to_owned),
    queue: "todo".to_owned(),
    ..Open::default()
  }
}

fn room(room: Option<i64>) -> Orderer {
  Orderer {
    room,
    requester: None,
  }
}

fn standing(open: Vec<Open>, free: &[&str]) -> Standing {
  Standing {
    open,
    free: free.iter().map(|name| (*name).to_owned()).collect(),
    known: ["w1", "w2", "worker"].map(str::to_owned).to_vec(),
    last_start: Some(1_000),
    minutes: 5,
    ..Standing::default()
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
    vec![&open(1, Some("w1")), &open(3, None)],
    "its own assignee free, or naming none while a checkout is free"
  );
  assert!(
    awaiting_launch(&standing(vec![open(3, None)], &[])).is_empty(),
    "naming none while no checkout is free"
  );

  let seen = BTreeMap::from([(1, 10_000)]);
  let none = BTreeMap::new();

  assert!(launches_due(&quiet, &seen, &none, 10_000 + MINUTE - 1).is_empty(), "not yet a minute");
  assert_eq!(launches_due(&quiet, &seen, &none, 10_000 + MINUTE), vec![&open(1, Some("w1"))]);
  assert_eq!(
    launches_due(&quiet, &BTreeMap::from([(1, 10_000), (3, 10_000)]), &none, 10_000 + MINUTE),
    vec![&open(1, Some("w1")), &open(3, None)]
  );
  assert!(
    launches_due(&quiet, &seen, &BTreeMap::from([(1, 10_000)]), 10_000 + MINUTE).is_empty(),
    "told once"
  );
  assert!(
    launches_due(&quiet, &BTreeMap::new(), &none, 10_000 + MINUTE).is_empty(),
    "never seen waiting"
  );

  quiet.silenced = true;
  assert!(launches_due(&quiet, &seen, &none, 10_000 + MINUTE).is_empty(), "silenced");
}

fn for_agent(id: i64, assignee: &str, agent: &str) -> Open {
  Open {
    agent: Some(agent.to_owned()),
    ..open(id, Some(assignee))
  }
}

#[test]
fn an_item_whose_agent_carries_a_run_waits_on_that_agent_and_awaits_once_it_is_free() {
  let items = vec![for_agent(1, "w1", "coder-a"), open(2, Some("w1"))];
  let mut busy = standing(items.clone(), &["w1"]);

  busy.carrying = vec!["coder-a".to_owned()];
  assert_eq!(awaiting_launch(&busy), vec![&open(2, Some("w1"))], "its agent carries a run");
  assert_eq!(
    awaiting_launch(&standing(items, &["w1"])),
    vec![&for_agent(1, "w1", "coder-a"), &open(2, Some("w1"))],
    "its agent free again"
  );
}

#[test]
fn a_free_agent_is_offered_its_first_item_a_pass_and_the_next_once_that_one_is_told() {
  let quiet = standing(
    vec![
      for_agent(7, "w2", "coder-a"),
      for_agent(5, "w1", "coder-a"),
      for_agent(6, "w1", "coder-b"),
    ],
    &["w1", "w2"],
  );
  let seen = BTreeMap::from([(5, 0), (6, 0), (7, 0)]);

  assert_eq!(
    launches_due(&quiet, &seen, &BTreeMap::new(), MINUTE),
    vec![&for_agent(5, "w1", "coder-a"), &for_agent(6, "w1", "coder-b")],
    "one item an agent, the lowest number"
  );
  assert_eq!(
    launches_due(&quiet, &seen, &BTreeMap::from([(5, 0), (6, 0)]), MINUTE),
    vec![&for_agent(7, "w2", "coder-a")],
    "the next once the first is told and the agent still free"
  );
  assert!(
    launches_due(&quiet, &seen, &BTreeMap::from([(5, MINUTE - 1), (6, 0)]), MINUTE).is_empty(),
    "not before the next-offer seconds have passed since the first was told"
  );
  assert_eq!(QUEUE_IDLE.next_offer_seconds * 1000, MINUTE, "a minute, as the policy sets it");

  let raised = standing(
    vec![
      for_agent(5, "w1", "coder-a"),
      Open {
        priority: 1,
        ..for_agent(9, "w1", "coder-a")
      },
    ],
    &["w1"],
  );
  let seen = BTreeMap::from([(5, 0), (9, 0)]);

  assert_eq!(
    launches_due(&raised, &seen, &BTreeMap::new(), MINUTE)
      .iter()
      .map(|item| item.id)
      .collect::<Vec<_>>(),
    [9],
    "an item whose priority was raised goes before a lower number"
  );
}

#[test]
fn an_item_awaiting_a_launch_is_told_in_the_owner_s_room_with_its_data() {
  let told = launch_instruction(&open(2265, Some("w1")), None, &room(None));
  let order = told["said"]["order"].as_str().unwrap();

  assert_eq!(told["room"], Value::Null, "an item naming no room is the owner's private room's");
  assert_eq!((&told["sort"], &told["kind"]), (&json!("job"), &json!("item-awaits-launch:2265")));

  // An item asked in a room is told in that room: its roomId, else the room its scope names.
  let asked = open(2268, Some("w1"));

  assert_eq!(
    launch_instruction(&asked, None, &room(room_of(&json!({ "scope": "-9000000000003" }))))["room"],
    json!(-9_000_000_000_003_i64)
  );

  let anyone = launch_instruction(&open(2269, None), None, &room(None));
  let anyone = anyone["said"]["order"].as_str().unwrap();

  assert!(anyone.contains(&policy::job_notices::JOB_NOTICES.any_worker), "{anyone}");
  assert_eq!(room_of(&json!({ "roomId": 5, "scope": "-7" })), Some(5));
  assert_eq!(room_of(&json!({ "scope": "" })), None);
  assert_eq!(told["events"], json!(["item-awaits-launch:#2265"]));

  for word in ["#2265", "item 2265", "w1"] {
    assert!(order.contains(word), "{order}");
  }

  assert_no_instruction(order);
  assert_no_instruction(anyone);
}

#[test]
fn the_notice_carries_its_code_the_waiting_items_and_its_marker() {
  let said = notice(&standing(vec![open(1, Some("w1")), open(2, Some("w2"))], &["w1"]));

  assert_eq!(said["code"], QUEUE_IDLE.code.as_str());
  assert_eq!(said["openItems"], json!([{ "id": 1, "title": "item 1", "assignee": "w1" }]));
  assert_eq!(said["freeWorkers"], json!(["w1"]));
  assert_eq!(said["idleMinutes"], 5);
  assert_eq!(said["ask"], QUEUE_IDLE.ask.as_str());
  assert_no_instruction(&QUEUE_IDLE.ask);
}

#[test]
fn a_draft_names_who_raised_it_its_parent_and_its_own_room() {
  let derived = draft_of(&json!({
    "id": 2433, "title": "a follow-up", "state": "draft", "brief": "do it",
    "labels": { "raised-by": "2432" }, "requesterName": "chat", "assigneeName": "w1",
  }))
  .unwrap();

  assert_eq!(
    (
      derived.parent,
      derived.raised_by.as_deref(),
      derived.room,
      derived.repository.as_deref()
    ),
    (Some(2432), Some("item #2432"), None, Some("w1"))
  );

  let asked =
    draft_of(&json!({ "id": 7, "title": "asked", "requesterName": "member", "roomId": 5 })).unwrap();

  assert_eq!((asked.parent, asked.raised_by.as_deref(), asked.room), (None, Some("member"), Some(5)));
  assert_eq!(draft_of(&json!({ "labels": { "raised-by": 12 } })), None, "a member with no number");
  assert_eq!(
    drafts_due(&[derived.clone(), asked], &BTreeSet::from([7])),
    vec![&derived],
    "a draft already told is not due"
  );
}

#[test]
fn a_draft_is_told_as_a_job_order_with_its_kind_event_and_the_start_of_its_brief() {
  let brief = (1..=40).map(|line| format!("line {line}\n\n")).collect::<String>();
  let draft = Draft {
    id: 2434,
    title: "a follow-up".to_owned(),
    raised_by: Some("item #2432".to_owned()),
    parent: Some(2432),
    repository: Some("w1".to_owned()),
    brief,
    room: None,
  };
  let told = approval_instruction(&draft, &room(Some(-9_000_000_000_003)));
  let order = told["said"]["order"].as_str().unwrap();

  assert_eq!(told["sort"], "job");
  assert_eq!(told["kind"], "item-awaits-approval:2434");
  assert_eq!(told["events"], json!(["item-awaits-approval:#2434"]));
  assert_eq!(told["room"], json!(-9_000_000_000_003_i64));
  assert_eq!(
    approval_instruction(&draft, &room(None))["room"],
    Value::Null,
    "the owner's private room"
  );

  for word in ["#2434", "a follow-up", "item #2432", "w1", "line 1"] {
    assert!(order.contains(word), "{order}");
  }

  assert_no_instruction(order);

  let head = brief_head(&draft.brief);

  assert_eq!(head.lines().count(), QUEUE_IDLE.draft_brief_lines, "{head}");
  assert!(!head.contains("\n\n") && !order.contains("line 40"), "{order}");

  let long = brief_head(&"x".repeat(QUEUE_IDLE.draft_brief_chars + 10));

  assert_eq!(long.chars().count(), QUEUE_IDLE.draft_brief_chars + 1, "cut, with its mark");
}

type Calls = Arc<Mutex<Vec<(String, Value)>>>;
type Drafts = Arc<Mutex<Vec<Value>>>;

#[derive(Clone, Default)]
struct Peers {
  calls: Calls,
  drafts: Drafts,
}

// The job service and the receiver in one: two queues, one paused, the open items across them, the drafts
// a test sets and the parents the drafts name; every call kept.
async fn stand_in(State(peers): State<Peers>, uri: Uri, body: Bytes) -> axum::response::Response {
  peers
    .calls
    .lock()
    .unwrap()
    .push((uri.path().to_owned(), serde_json::from_slice(&body).unwrap_or(Value::Null)));

  if uri.path() == "/items" && uri.query().is_some_and(|query| query.contains("state=draft")) {
    return axum::Json(json!({ "items": *peers.drafts.lock().unwrap(), "more": false })).into_response();
  }

  // The parents: 30 asked in a room, 31 raised by 30 and naming none, 33 and 34 raising each other.
  match uri.path() {
    "/items/30" => {
      return axum::Json(json!({ "id": 30, "state": "done", "scope": "-9000000000007" })).into_response();
    }
    "/items/31" => {
      return axum::Json(json!({ "id": 31, "state": "done", "labels": { "raised-by": "30" } }))
        .into_response();
    }
    "/items/33" => {
      return axum::Json(json!({ "id": 33, "state": "done", "labels": { "raised-by": "34" } }))
        .into_response();
    }
    "/items/34" => {
      return axum::Json(json!({ "id": 34, "state": "done", "labels": { "raised-by": "33" } }))
        .into_response();
    }
    path if path.starts_with("/items/") => {
      return (axum::http::StatusCode::NOT_FOUND, axum::Json(json!({ "code": "not-found" })))
        .into_response();
    }
    _ => {}
  }

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
        { "id": 15, "title": "the owner's own ask", "state": "open", "queueName": "later", "assigneeName": "w1",
          "labels": { "asked-by": "owner" }, "scope": "-9000000000003" },
      ],
      "more": false,
    })),
    _ => axum::Json(json!({ "id": 1 })),
  }
  .into_response()
}

async fn stand(peers: &Peers) -> BTreeMap<String, String> {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(stand_in).with_state(peers.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  [
    ("BMO_JOB_URL".to_owned(), address.clone()),
    ("BMO_JOB_TOKEN".to_owned(), "job-token".to_owned()),
    ("BMO_RECEIVER_URL".to_owned(), address.clone()),
    ("BMO_RECEIVER_TOKEN".to_owned(), "receiver-token".to_owned()),
  ]
  .into()
}

#[tokio::test]
async fn a_told_item_that_stops_waiting_is_let_go_so_it_is_told_again_once_free() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let peers = Peers::default();
  let vars = stand(&peers).await;

  repository(db, "w1", "/checkouts/w1", &[]).await;
  repository(db, "w2", "/checkouts/w2", &[]).await;
  supervisor(db, "chat", &[]).await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
           "started_at", "prompt")
         VALUES (20, 1, 'runner', 1, 'w2', 'running', 1000, 'the work')"#,
      vec![],
    )
    .await;

  for id in [11, 12] {
    scratch
      .run(
        r#"INSERT INTO "notices" ("key", "route", "body", "keep", "owed_at", "told_at", "due_at")
           VALUES ($1, 'instruction', '{}', 'standing', 1000, 1000, 9000000000000000)"#,
        vec![format!("{LAUNCH_OWED}{id}").into()],
      )
      .await;
  }

  pass(db, &vars).await.unwrap();

  let marked = |id: i64| {
    scratch.count(
      r#"SELECT COUNT(*) AS "count" FROM "notices" WHERE "key" = $1"#,
      vec![format!("{LAUNCH_OWED}{id}").into()],
    )
  };

  assert_eq!(marked(11).await, 1, "item 11 still waits on its free worker");
  assert_eq!(marked(12).await, 0, "item 12's worker is busy, its notice let go");
}

#[tokio::test]
async fn a_pass_tells_the_owner_once_of_the_items_a_free_worker_could_take() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let peers = Peers::default();
  let calls = peers.calls.clone();
  let vars = stand(&peers).await;

  repository(db, "w1", "/checkouts/w1", &[]).await;
  repository(db, "w2", "/checkouts/w2", &[]).await;
  supervisor(db, "chat", &[]).await;
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

  // The item on its own free worker is first seen waiting; told to the supervisor once a minute has passed.
  let instructed = |calls: &Calls| -> Vec<Value> {
    calls
      .lock()
      .unwrap()
      .iter()
      .filter(|(path, body)| {
        path == "/instructions"
          && body["kind"].as_str().is_some_and(|kind| kind.starts_with("item-awaits-launch:"))
      })
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

// The approval instructions the stand-in was handed, in the order they came.
fn approvals(calls: &Calls) -> Vec<Value> {
  calls
    .lock()
    .unwrap()
    .iter()
    .filter(|(path, body)| {
      path == "/instructions"
        && body["kind"].as_str().is_some_and(|kind| kind.starts_with("item-awaits-approval:"))
    })
    .map(|(_, body)| body.clone())
    .collect()
}

#[tokio::test]
async fn a_draft_is_told_once_to_the_supervisor_that_ordered_it_and_never_after_it_leaves_draft() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let peers = Peers::default();
  let vars = stand(&peers).await;

  *peers.drafts.lock().unwrap() = vec![
    json!({ "id": 21, "title": "asked in a room", "state": "draft", "scope": "-9000000000005",
            "requesterName": "member", "brief": "fix the thing" }),
    json!({ "id": 22, "title": "a worker's follow-up", "state": "draft", "labels": { "raised-by": "30" },
            "assigneeName": "w1", "brief": "the follow-up" }),
    json!({ "id": 23, "title": "a follow-up of a follow-up", "state": "draft",
            "labels": { "raised-by": "31" } }),
    json!({ "id": 24, "title": "naming nothing", "state": "draft" }),
    json!({ "id": 25, "title": "its parent gone", "state": "draft", "labels": { "raised-by": "32" } }),
    json!({ "id": 26, "title": "a chain going round", "state": "draft", "labels": { "raised-by": "33" } }),
  ];

  pass(db, &vars).await.unwrap();

  let told = approvals(&peers.calls);
  let by_item: BTreeMap<String, (Value, Value)> = told
    .iter()
    .map(|body| (body["kind"].as_str().unwrap().to_owned(), (body["room"].clone(), body["sort"].clone())))
    .collect();

  assert_eq!(told.len(), 6, "each draft told once: {told:?}");
  assert_eq!(
    by_item,
    BTreeMap::from([
      ("item-awaits-approval:21".to_owned(), (json!(-9_000_000_000_005_i64), json!("job"))),
      ("item-awaits-approval:22".to_owned(), (json!(-9_000_000_000_007_i64), json!("job"))),
      ("item-awaits-approval:23".to_owned(), (json!(-9_000_000_000_007_i64), json!("job"))),
      ("item-awaits-approval:24".to_owned(), (Value::Null, json!("job"))),
      ("item-awaits-approval:25".to_owned(), (Value::Null, json!("job"))),
      ("item-awaits-approval:26".to_owned(), (Value::Null, json!("job"))),
    ]),
    "its own room, else its parent's up the chain, else the owner's private room"
  );
  assert!(
    told
      .iter()
      .all(|body| body["events"][0].as_str().unwrap().starts_with("item-awaits-approval:#"))
  );

  // The next pass, as after a restart, tells none of them again while they stay drafts.
  pass(db, &vars).await.unwrap();
  assert_eq!(approvals(&peers.calls).len(), 6, "told once");

  // An approved draft leaves `draft`: its row let go, and it is told nothing more.
  peers.drafts.lock().unwrap().retain(|draft| draft["id"] != 21);
  pass(db, &vars).await.unwrap();
  assert_eq!(approvals(&peers.calls).len(), 6, "an item that left draft is told nothing");

  let rows = |key: String| async move { agency_server::queue::settings::get(db, &key).await.unwrap() };

  assert_eq!(rows(format!("{DRAFT_TOLD_PREFIX}21")).await, None, "its row let go");
  assert!(rows(format!("{DRAFT_TOLD_PREFIX}22")).await.is_some(), "a standing draft's row kept");

  // A new draft is told on the next pass.
  peers
    .drafts
    .lock()
    .unwrap()
    .push(json!({ "id": 27, "title": "a new one", "state": "draft", "roomId": 9 }));
  pass(db, &vars).await.unwrap();

  let told = approvals(&peers.calls);

  assert_eq!(told.len(), 7);
  assert_eq!((&told[6]["kind"], &told[6]["room"]), (&json!("item-awaits-approval:27"), &json!(9)));
}

#[tokio::test]
async fn an_item_the_owner_asked_for_opens_at_once_and_is_never_told_for_approval() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let peers = Peers::default();
  let vars = stand(&peers).await;

  // The open items the stand-in serves carry one the owner's ask opened; no draft stands.
  pass(db, &vars).await.unwrap();
  pass(db, &vars).await.unwrap();

  assert!(approvals(&peers.calls).is_empty(), "{:?}", approvals(&peers.calls));
  assert!(
    peers.calls.lock().unwrap().iter().any(|(path, _)| path == "/items"),
    "the pass read the job service"
  );
}

#[test]
fn an_item_whose_assignee_is_no_checkout_is_unlaunchable_and_never_awaits_a_launch() {
  let stuck = standing(vec![open(1, Some("gone")), open(2, Some("w1")), open(3, None)], &["w1"]);

  assert_eq!(unlaunchable(&stuck), vec![&open(1, Some("gone"))]);
  assert!(!awaiting_launch(&stuck).contains(&&open(1, Some("gone"))));
}

// The kinds of the instructions the stand-in was handed, each with its room, in the order they came.
fn kinds(peers: &crate::support::peers::Peers) -> Vec<(String, Value)> {
  peers
    .lock()
    .unwrap()
    .instructed()
    .iter()
    .map(|body| (body["kind"].as_str().unwrap_or_default().to_owned(), body["room"].clone()))
    .collect()
}

#[tokio::test]
async fn open_items_that_cannot_go_or_go_to_any_checkout_are_told_once_to_their_orderer() {
  use crate::support::peers::{item, stand};

  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;

  repository(db, "w1", "/checkouts/w1", &[]).await;

  {
    let mut held = peers.lock().unwrap();

    held.queues = vec![
      json!({ "name": "todo", "state": "open", "holds": [] }),
      json!({ "name": "later", "state": "paused", "holds": [] }),
      json!({ "name": "aside", "state": "open", "holds": [{ "reason": "deploy" }] }),
    ];
    // 90 holds the room; 91 is raised by it and names no checkout; 92 waits on a paused queue, 93 on one
    // under a hold; 94 names a repository no checkout is of.
    held.items.insert(90, item(90, "done", json!({ "scope": "-9000" })));
    held.items.insert(91, item(91, "open", json!({ "raisedBy": 90 })));
    held
      .items
      .insert(92, item(92, "open", json!({ "queueName": "later", "scope": "-9200" })));
    held
      .items
      .insert(93, item(93, "open", json!({ "queueName": "aside", "labels": { "raised-by": "90" } })));
    held
      .items
      .insert(94, item(94, "open", json!({ "assigneeName": "nowhere", "scope": "-9400" })));
  }

  pass(db, &vars).await.unwrap();

  // An item naming no checkout is first seen waiting; told once a minute has passed.
  scratch
    .run(
      r#"UPDATE "settings" SET "value" = $1 WHERE "key" = $2"#,
      vec![(agency_server::now_ms() - 61_000).to_string().into(), format!("{SEEN_PREFIX}91").into()],
    )
    .await;
  pass(db, &vars).await.unwrap();
  pass(db, &vars).await.unwrap();

  let told: BTreeMap<String, Value> = kinds(&peers).into_iter().collect();

  assert_eq!(kinds(&peers).len(), 4, "each told once: {:?}", kinds(&peers));
  assert_eq!(
    told,
    BTreeMap::from([
      ("item-queue-paused:92".to_owned(), json!(-9200)),
      ("item-queue-paused:93".to_owned(), json!(-9000)),
      ("item-unlaunchable:94".to_owned(), json!(-9400)),
      ("item-awaits-launch:91".to_owned(), json!(-9000)),
    ]),
    "each in its orderer's room, a derived item's in its parent's"
  );

  // The paused queue opens: the item's mark let go; the queue paused again tells it again.
  peers.lock().unwrap().queues[1]["state"] = json!("open");
  pass(db, &vars).await.unwrap();
  assert!(agency_server::queue::settings::get(db, "queue_paused_told:92").await.unwrap().is_none());
  peers.lock().unwrap().queues[1]["state"] = json!("paused");
  pass(db, &vars).await.unwrap();

  let again = kinds(&peers).into_iter().filter(|(kind, _)| kind == "item-queue-paused:92").count();

  assert_eq!(again, 2, "a transition that comes again is told again");
}

#[tokio::test]
async fn an_item_whose_agent_label_was_refused_is_told_once_for_that_label() {
  use agency_server::watches::queue_idle::{AGENT_TOLD_PREFIX, agent_told_let_go, tell_agent_refused};

  use crate::support::peers::stand;

  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;
  let item = json!({ "title": "on a named agent", "scope": "-9500", "labels": { "agent": "nobody-07" } });
  let why = "Item #95 names agent=nobody-07, which is no agent the agency registers";

  assert!(tell_agent_refused(db, &vars, 95, &item, "nobody-07", why).await.unwrap());
  assert!(!tell_agent_refused(db, &vars, 95, &item, "nobody-07", why).await.unwrap(), "told once");
  assert_eq!(kinds(&peers), vec![("item-unlaunchable:95".to_owned(), json!(-9500))]);

  let said = peers.lock().unwrap().instructed()[0]["said"]["order"]
    .as_str()
    .unwrap_or_default()
    .to_owned();

  assert!(said.contains("agent=nobody-07"), "{said}");

  // Another label refused is told again; a launch of the item lets the mark go.
  assert!(tell_agent_refused(db, &vars, 95, &item, "auto", why).await.unwrap());
  agent_told_let_go(db, 95).await;
  assert!(
    agency_server::queue::settings::get(db, &format!("{AGENT_TOLD_PREFIX}95"))
      .await
      .unwrap()
      .is_none()
  );
  assert_eq!(kinds(&peers).len(), 2);
}

#[test]
fn a_held_item_names_what_it_waits_on_and_an_item_waited_on_ends_once_done_and_not_waiting_live() {
  use agency_server::watches::queue_idle::{follower_of, wait_over};

  let one = follower_of(&json!({ "id": 2650, "title": "after", "state": "held",
    "labels": { "held_kind": "pause", "follows": "2649" }, "scope": "-9100" }))
  .unwrap();

  assert_eq!(
    (one.label.as_str(), one.waits_on.as_slice(), one.room),
    ("follows=2649", &[2649][..], Some(-9100))
  );

  let several =
    follower_of(&json!({ "id": 2693, "labels": { "waiting-on": "#2664, 2665" }, "raisedBy": 2600 }))
      .unwrap();

  assert_eq!((several.waits_on.as_slice(), several.parent), (&[2664, 2665][..], Some(2600)));
  assert_eq!(
    follower_of(&json!({ "id": 1, "labels": { "held_kind": "pause" } })),
    None,
    "no wait label"
  );
  assert_eq!(
    follower_of(&json!({ "id": 1, "labels": { "follows": "2649, soon" } })),
    None,
    "a part naming no item"
  );
  assert_eq!(
    follower_of(&json!({ "labels": { "follows": "2649" } })),
    None,
    "a member with no number"
  );

  assert!(wait_over(&json!({ "state": "done" })));
  assert!(wait_over(
    &json!({ "state": "done", "labels": { "close-phase": "live", "close-commit": "abc" } })
  ));
  assert!(
    wait_over(&json!({ "state": "done", "labels": { "close-phase": "testing" } })),
    "no commit waits"
  );
  assert!(
    wait_over(
      &json!({ "state": "done", "labels": { "close-phase": "waiting-live", "close-commit": "abc" } })
    ),
    "done is past its deployment, whatever the close label still reads"
  );
  assert!(!wait_over(
    &json!({ "state": "leased", "labels": { "close-phase": "waiting-live", "close-commit": "abc" } })
  ));
  assert!(!wait_over(&json!({ "state": "leased" })));
  assert!(!wait_over(&json!({ "state": "open" })));
}

#[test]
fn a_held_item_whose_wait_ended_is_told_with_what_it_waited_on_and_no_instruction() {
  let waited = [
    json!({ "id": 2649, "title": "the first", "state": "done" }),
    json!({ "id": 2651, "title": "the second", "state": "done",
            "labels": { "close-phase": "live", "close-commit": "abc" } }),
  ];
  let told =
    agency_server::job_notices::wait_ended(2650, "after", "follows=2649,2651", &waited, &room(Some(-9100)));
  let order = told["said"]["order"].as_str().unwrap();

  assert_eq!(
    (&told["sort"], &told["kind"], &told["room"]),
    (&json!("job"), &json!("item-wait-ended:2650"), &json!(-9100))
  );
  assert_eq!(told["events"], json!(["item-wait-ended:#2650"]));

  for word in ["#2650", "after", "follows=2649,2651", "#2649", "the first", "#2651", "close-phase live"] {
    assert!(order.contains(word), "{word}: {order}");
  }

  assert_no_instruction(order);
}

#[tokio::test]
async fn a_held_item_is_told_once_its_wait_ends_and_let_go_once_it_is_no_longer_held() {
  use agency_server::watches::queue_idle::WAIT_TOLD_PREFIX;

  use crate::support::peers::{item, stand};

  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;

  {
    let mut held = peers.lock().unwrap();

    held.queues = vec![json!({ "name": "todo", "state": "open", "holds": [] })];
    // 100 is done; 101 is leased; 102 is done with its commit waiting to go live; 103 is done and live.
    held.items.insert(100, item(100, "done", json!({})));
    held.items.insert(101, item(101, "leased", json!({})));
    held.items.insert(
      102,
      item(
        102,
        "leased",
        json!({ "labels": { "close-phase": "waiting-live", "close-commit": "abc" } }),
      ),
    );
    held.items.insert(
      103,
      item(103, "done", json!({ "labels": { "close-phase": "live", "close-commit": "def" } })),
    );
    // The held items waiting on them, and one open item naming a done one.
    let follows =
      |on: &str, room: &str| json!({ "labels": { "held_kind": "pause", "follows": on }, "scope": room });

    held.items.insert(110, item(110, "held", follows("100", "-1100")));
    held
      .items
      .insert(111, item(111, "held", json!({ "labels": { "waiting-on": "101" }, "raisedBy": 110 })));
    held.items.insert(112, item(112, "held", follows("102", "-1120")));
    held.items.insert(113, item(113, "held", follows("100, #103", "-1130")));
    held.items.insert(114, item(114, "held", follows("100,101", "-1140")));
    held.items.insert(115, item(115, "open", follows("100", "-1150")));
    held.items.insert(116, item(116, "held", follows("999", "-1160")));
  }

  let ended = |peers: &crate::support::peers::Peers| -> Vec<(String, Value)> {
    kinds(peers)
      .into_iter()
      .filter(|(kind, _)| kind.starts_with("item-wait-ended:"))
      .collect()
  };

  pass(db, &vars).await.unwrap();
  pass(db, &vars).await.unwrap();

  assert_eq!(
    ended(&peers),
    vec![
      ("item-wait-ended:110".to_owned(), json!(-1100)),
      ("item-wait-ended:113".to_owned(), json!(-1130))
    ],
    "a held item following done items is told once; following a leased one, one waiting to go live, an item \
     the job service does not answer for, or not held, it is not"
  );

  // The leased item is done: every item that waited on it alone or with others is told, in its orderer's room.
  peers.lock().unwrap().items.get_mut(&101).unwrap()["state"] = json!("done");
  pass(db, &vars).await.unwrap();
  pass(db, &vars).await.unwrap();

  assert_eq!(
    ended(&peers)[2..],
    [
      ("item-wait-ended:111".to_owned(), json!(-1100)),
      ("item-wait-ended:114".to_owned(), json!(-1140))
    ]
  );

  // Lifted, an item's mark is let go; one still held keeps its mark and is not told again.
  peers.lock().unwrap().items.get_mut(&110).unwrap()["state"] = json!("open");
  pass(db, &vars).await.unwrap();

  let mark = |id: i64| async move {
    agency_server::queue::settings::get(db, &format!("{WAIT_TOLD_PREFIX}{id}"))
      .await
      .unwrap()
  };

  assert_eq!(mark(110).await, None, "no longer held, its mark let go");
  assert!(mark(113).await.is_some(), "still held, its mark kept");
  assert_eq!(ended(&peers).len(), 4, "told once while it stays held");
}

// A coder agent on a road of Claude Code's, switched on.
fn coder(name: &str, model: &str) -> agency_server::role_agents::RoleAgent {
  agency_server::role_agents::RoleAgent {
    name: name.to_owned(),
    role: "coder".to_owned(),
    owner_account: None,
    provider: Some("claude-code".to_owned()),
    model: Some(model.to_owned()),
    effort: None,
    thinking: None,
    answer_tokens: None,
    enabled: true,
    keeps_cache: false,
    sessions: Default::default(),
    fold_ratio: None,
    prompt: None,
    bundles: None,
    skills: None,
    added_at: 0,
  }
}

fn on_model(id: i64, model: &str) -> Open {
  Open {
    model: Some(model.to_owned()),
    role: Some("coder".to_owned()),
    ..open(id, Some("w1"))
  }
}

// Two coders on one model and one on another, those named carrying a run.
fn coders(open: Vec<Open>, carrying: &[&str]) -> Standing {
  Standing {
    agents: vec![coder("coder-a", "opus"), coder("coder-b", "opus"), coder("coder-c", "local")],
    carrying: carrying.iter().map(|name| (*name).to_owned()).collect(),
    ..standing(open, &["w1", "w2"])
  }
}

#[test]
fn an_item_naming_a_model_awaits_while_an_agent_of_its_role_on_it_is_free_and_names_the_one_it_would_take()
{
  use agency_server::watches::queue_idle::seats;

  let items = vec![on_model(1, "opus")];

  assert!(
    awaiting_launch(&coders(items.clone(), &["coder-a", "coder-b"])).is_empty(),
    "every candidate busy"
  );

  let one_free = coders(items.clone(), &["coder-a"]);

  assert_eq!(awaiting_launch(&one_free), vec![&on_model(1, "opus")]);
  assert_eq!(seats(&one_free), BTreeMap::from([(1, "coder-b".to_owned())]));

  let told = launch_instruction(&on_model(1, "opus"), Some("coder-b"), &room(None));
  let order = told["said"]["order"].as_str().unwrap();

  assert!(
    order.contains("coder-b") && order.contains("model=opus") && order.contains("coder"),
    "{order}"
  );
  assert_no_instruction(order);

  // An agent label stands over the model: the item waits on that agent alone.
  let named = Open {
    agent: Some("coder-c".to_owned()),
    ..on_model(2, "opus")
  };

  assert!(awaiting_launch(&coders(vec![named.clone()], &["coder-c"])).is_empty(), "its agent busy");
  assert_eq!(seats(&coders(vec![named], &["coder-a"])), BTreeMap::from([(2, "coder-c".to_owned())]));

  // Candidates are of the item's own role.
  let worker = Open {
    role: Some("worker".to_owned()),
    ..on_model(3, "opus")
  };

  assert!(awaiting_launch(&coders(vec![worker], &[])).is_empty(), "no worker runs the model");
}

#[test]
fn free_agents_of_one_model_are_offered_different_items_and_one_free_agent_its_items_one_at_a_time() {
  use agency_server::watches::queue_idle::seats;

  let items = vec![on_model(6, "opus"), on_model(5, "opus")];
  let seen = BTreeMap::from([(5, 0), (6, 0)]);
  let both = coders(items.clone(), &[]);

  assert_eq!(seats(&both), BTreeMap::from([(5, "coder-a".to_owned()), (6, "coder-b".to_owned())]));
  assert_eq!(launches_due(&both, &seen, &BTreeMap::new(), MINUTE).len(), 2, "one item a free agent");

  let one = coders(items, &["coder-b"]);

  assert_eq!(
    launches_due(&one, &seen, &BTreeMap::new(), MINUTE),
    vec![&on_model(5, "opus")],
    "the first in the queue's order"
  );
  assert_eq!(
    launches_due(&one, &seen, &BTreeMap::from([(5, 0)]), MINUTE),
    vec![&on_model(6, "opus")],
    "the next once the first is told and the agent still free"
  );
}

#[test]
fn an_item_naming_a_model_no_agent_of_its_role_runs_has_no_candidate() {
  use agency_server::role_agents::roads_held;
  use agency_server::watches::queue_idle::without_candidate;

  let named = Open {
    agent: Some("coder-a".to_owned()),
    ..on_model(3, "gpt")
  };
  let mut held = coders(vec![on_model(1, "gpt"), on_model(2, "opus"), named, on_model(4, "local")], &[]);

  held.agents[2].enabled = false;

  assert_eq!(
    without_candidate(&held),
    vec![&on_model(1, "gpt"), &on_model(4, "local")],
    "none, or switched off"
  );
  assert!(!awaiting_launch(&held).contains(&&on_model(1, "gpt")));

  let roads = roads_held("coder", &held.agents);

  assert!(roads.contains("- coder-a: claude-code/opus\n"), "{roads}");
  assert!(roads.contains("- coder-c: claude-code/local, switched off"), "{roads}");
}

#[tokio::test]
async fn an_item_whose_model_no_agent_runs_is_told_once_and_let_go_once_an_agent_runs_it() {
  use agency_server::watches::queue_idle::NO_CANDIDATE_TOLD_PREFIX;

  use crate::support::peers::{item, stand};

  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;

  crate::support::rows::seats(db, "worker", &[("deepseek", "deepseek-flash")]).await;
  peers.lock().unwrap().queues = vec![json!({ "name": "todo", "state": "open", "holds": [] })];
  peers.lock().unwrap().items.insert(
    96,
    item(
      96,
      "open",
      json!({ "scope": "-9600", "labels": { "model": "rare-model", "role": "worker" } }),
    ),
  );

  pass(db, &vars).await.unwrap();
  pass(db, &vars).await.unwrap();

  let told: Vec<(String, Value)> = kinds(&peers)
    .into_iter()
    .filter(|(kind, _)| kind.starts_with("item-no-candidate"))
    .collect();

  assert_eq!(told, vec![("item-no-candidate:96".to_owned(), json!(-9600))], "told once, in its room");

  let order = peers
    .lock()
    .unwrap()
    .instructed()
    .iter()
    .find(|body| body["kind"] == "item-no-candidate:96")
    .unwrap()["said"]["order"]
    .as_str()
    .unwrap_or_default()
    .to_owned();

  assert!(
    order.contains("model=rare-model") && order.contains("the worker role:\n- worker-01"),
    "{order}"
  );
  assert_no_instruction(&order);

  // An agent of the role moved onto the model: the mark is let go.
  scratch
    .run(r#"UPDATE "role_agents" SET "provider" = 'claude-code', "model" = 'rare-model' WHERE "name" = 'worker-01'"#, vec![])
    .await;
  pass(db, &vars).await.unwrap();
  assert!(
    agency_server::queue::settings::get(db, &format!("{NO_CANDIDATE_TOLD_PREFIX}96"))
      .await
      .unwrap()
      .is_none()
  );
}
