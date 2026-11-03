//! The idle queue's alarm (docs/agent-control.md, 「An idle queue is told to the owner」): approved work a
//! free worker could take and no job started for the idle minutes, told to the owner once for each stretch
//! through the receiver, and silenced by the owner's switch until the next job starts; and a draft awaiting
//! approval told once to the supervisor that ordered it, in its room or its parent's up the `raised-by` chain.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use agency_server::queue::orderer::Orderer;
use agency_server::watches::queue_idle::{
  DRAFT_TOLD_PREFIX, Draft, MINUTES_KEY, Open, SEEN_PREFIX, SWITCH_KEY, Standing, approval_instruction,
  awaiting_launch, brief_head, draft_of, drafts_due, idle, job_started, launch_instruction, launches_due,
  notice, pass, room_of, unlaunchable, waiting,
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
  let none = BTreeSet::new();

  assert!(launches_due(&quiet, &seen, &none, 10_000 + MINUTE - 1).is_empty(), "not yet a minute");
  assert_eq!(launches_due(&quiet, &seen, &none, 10_000 + MINUTE), vec![&open(1, Some("w1"))]);
  assert_eq!(
    launches_due(&quiet, &BTreeMap::from([(1, 10_000), (3, 10_000)]), &none, 10_000 + MINUTE),
    vec![&open(1, Some("w1")), &open(3, None)]
  );
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
  let told = launch_instruction(&open(2265, Some("w1")), &room(None));
  let order = told["said"]["order"].as_str().unwrap();

  assert_eq!(told["room"], Value::Null, "an item naming no room is the owner's private room's");
  assert_eq!((&told["sort"], &told["kind"]), (&json!("job"), &json!("item-awaits-launch:2265")));

  // An item asked in a room is told in that room: its roomId, else the room its scope names.
  let asked = open(2268, Some("w1"));

  assert_eq!(
    launch_instruction(&asked, &room(room_of(&json!({ "scope": "-9000000000003" }))))["room"],
    json!(-9_000_000_000_003_i64)
  );

  let anyone = launch_instruction(&open(2269, None), &room(None));

  assert!(
    anyone["said"]["order"]
      .as_str()
      .unwrap()
      .contains(&policy::job_notices::JOB_NOTICES.any_worker),
    "{anyone}"
  );
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

  for word in ["#2434", "a follow-up", "item #2432", "w1", "line 1", "job_item_update", "owner"] {
    assert!(order.contains(word), "{order}");
  }

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
