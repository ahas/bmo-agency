//! The delegate: a seat asked of the pool under the helper role and waited for in its line however long that
//! takes, one turn into a helper agent's own worker session, resumed as a job's turn is, in the agent's own
//! checkout, waited on with no bound and sent again while the agent is in a turn of its own, the seat bound to
//! the turn's run and finished, the session kept, and the answer the turn's words with the files it created or
//! changed in the task's folder; a seat given back before it was let in, a role nobody holds, a turn that
//! answers nothing and a turn not the owner's past the gate are refused, and a stopped call gives its seat
//! back (docs/tools.md, 「The delegate」).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use delegate_tool::{Api, DelegateInput, Held, ListInput, delegate_with, list_with, may_delegate};
use serde_json::{Value, json};
use tool_call::Session;
use tool_call::room_gate::AccountService;

// How the stub pool, workers and session routes behave, and everything they were asked.
#[derive(Default)]
struct Stub {
  // How many admissions answer the seat not let in before one lets it in; none lets it in where unset.
  waits: Option<usize>,
  // Whether the seat reads as given back while it is not let in.
  given_back: bool,
  // How many sends the session answers busy before it takes one.
  busy: usize,
  // The answer the turn ends with.
  answer: Option<String>,
  // How long the turn works before it answers.
  works: Duration,
  asked_seat: Option<Value>,
  admissions: usize,
  bound: Option<Value>,
  finished: Vec<Value>,
  sends: Vec<String>,
  turned: Option<Value>,
}

type Shared = Arc<Mutex<Stub>>;

fn seat(state: &str) -> Value {
  json!({
    "token": "t1", "state": state, "provider": "claude-code", "model": "claude-sonnet-4-6",
    "effort": "medium", "tier": "Reliable", "workKey": "role:helper",
  })
}

async fn workers() -> Json<Value> {
  Json(json!({ "items": [
    { "name": "builder", "role": "worker", "state": "idle", "cwd": "/work/builder", "sessionId": null },
    { "name": "helper-b", "role": "helper", "state": "working", "cwd": "/work/helper-b", "sessionId": null },
    { "name": "helper-a", "role": "helper", "state": "idle", "cwd": "/work/helper-a", "sessionId": null },
    { "name": "helper-c", "role": "helper", "state": "idle", "cwd": "/work/helper-c", "nodeName": "n1" },
    { "name": "helper-d", "role": "helper", "state": "idle", "cwd": "/work/helper-d", "restingSince": 1 },
  ], "more": false }))
}

async fn ask_seat(State(stub): State<Shared>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
  stub.lock().unwrap().asked_seat = Some(body);

  (StatusCode::CREATED, Json(seat("waiting")))
}

async fn move_seat(State(stub): State<Shared>, Json(body): Json<Value>) -> Json<Value> {
  let mut stub = stub.lock().unwrap();

  match body["state"].as_str() {
    Some("bound") => {
      stub.bound = Some(body);
      Json(seat("bound"))
    }
    Some("finished") => {
      stub.finished.push(body);
      Json(json!({ "held": true, "stoodDown": false, "note": null }))
    }
    _ => {
      stub.admissions += 1;

      match stub.waits {
        Some(waits) if stub.admissions > waits => {
          Json(json!({ "request": seat("active"), "demand": null }))
        }
        _ => Json(json!({ "request": null, "demand": null })),
      }
    }
  }
}

async fn read_seat(State(stub): State<Shared>) -> Json<Value> {
  Json(seat(if stub.lock().unwrap().given_back { "cancelled" } else { "waiting" }))
}

// The session answers busy as often as it is told to, then works for a while, writes a file into the folder
// its words name as the helper would, and answers.
async fn turn(
  State(stub): State<Shared>,
  Path(session): Path<String>,
  Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
  let (works, answer) = {
    let mut stub = stub.lock().unwrap();

    stub.sends.push(session);

    if stub.busy > 0 {
      stub.busy -= 1;

      return (
        StatusCode::CONFLICT,
        Json(json!({ "code": "session-busy", "error": "helper-a@worker is in a turn" })),
      );
    }

    stub.turned = Some(body.clone());
    (stub.works, stub.answer.clone())
  };

  tokio::time::sleep(works).await;

  let prompt = body["prompt"].as_str().unwrap();
  let folder = prompt.strip_prefix("Work in ").and_then(|rest| rest.split_once(".\n")).unwrap().0;

  std::fs::write(std::path::Path::new(folder).join("found.txt"), "what the helper found").unwrap();

  (
    StatusCode::CREATED,
    Json(json!({
      "id": 1, "state": "ended", "outcome": if answer.is_some() { "answered" } else { "failed" },
      "answer": answer,
    })),
  )
}

async fn server(stub: Stub) -> (Api, Shared) {
  let shared = Arc::new(Mutex::new(stub));
  let app = Router::new()
    .route("/workers", get(workers))
    .route("/seat-requests", post(ask_seat))
    .route("/seat-requests/{token}", patch(move_seat).get(read_seat))
    .route("/sessions/{id}/turns", post(turn))
    .with_state(shared.clone());
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  (
    Api {
      port,
      token: None,
      pace: Duration::from_millis(10),
    },
    shared,
  )
}

fn session(root: &std::path::Path) -> Session {
  Session {
    variables: BTreeMap::from([
      ("BMO_DATA_DIR".to_owned(), root.to_string_lossy().into_owned()),
      ("BMO_TURN_AGENT".to_owned(), "bmo".to_owned()),
      ("BMO_TURN_CHAT".to_owned(), "-7".to_owned()),
    ]),
    ..Session::default()
  }
}

fn input(instruction: &str) -> DelegateInput {
  DelegateInput {
    title: "Read the node's disk".into(),
    instruction: instruction.into(),
    role: None,
    folder: None,
  }
}

#[tokio::test]
async fn a_seat_waits_in_line_and_the_helper_s_turn_answers_with_its_words_and_the_files_it_made() {
  let home = tempfile::tempdir().unwrap();
  let scratch = home.path().join("agent-workspaces").join("bmo").join("scratch");

  std::fs::create_dir_all(&scratch).unwrap();
  std::fs::write(scratch.join("kept.txt"), "here before the task").unwrap();

  let (api, stub) = server(Stub {
    waits: Some(5),
    busy: 3,
    answer: Some("The disk is 81% full.".into()),
    works: Duration::from_millis(1500),
    ..Stub::default()
  })
  .await;
  let answer = delegate_with(
    &api,
    &session(home.path()),
    input("ssh to the node and read its disk"),
    &Held::default(),
  )
  .await;
  let text = serde_json::to_string(&answer).unwrap();

  assert!(text.contains("The disk is 81% full."), "{text}");
  assert!(text.contains("found.txt"), "the file the turn made is named: {text}");
  assert!(!text.contains("kept.txt"), "a file the turn left alone is not: {text}");

  let stub = stub.lock().unwrap();
  let asked = stub.asked_seat.clone().unwrap();

  // The seat is asked under the policy's role for delegated work, on Claude Code, under the delegate's source.
  assert_eq!(asked["role"], policy::roles::ROLES.delegate.as_str());
  assert_eq!(asked["role"], "helper");
  assert_eq!(asked["source"], "delegate");
  assert_eq!(asked["provider"], "claude-code");
  assert_eq!(asked["requestKey"], asked["owner"]);
  // It is asked under its title and the asking agent, which the delegation is listed by.
  assert_eq!(asked["title"], "Read the node's disk");
  assert_eq!(asked["askedBy"], "bmo");

  // It asked for its admission as long as the line held it, and only then sent the turn.
  assert_eq!(stub.admissions, 6);

  // The turn goes to the free helper on this machine, into its own session, sent again while it was busy.
  assert_eq!(stub.sends, ["helper-a@worker"; 4]);

  let turned = stub.turned.clone().unwrap();
  let run = turned["name"].as_str().unwrap();

  assert_eq!(stub.bound.clone().unwrap()["childId"], run, "the seat is bound to the turn's run");
  assert_eq!(stub.bound.clone().unwrap()["agentName"], "helper-a", "and names the helper it runs on");
  assert!(
    turned["prompt"]
      .as_str()
      .unwrap()
      .starts_with(&format!("Work in {}.", scratch.display()))
  );
  assert!(turned["prompt"].as_str().unwrap().ends_with("ssh to the node and read its disk"));
  assert_eq!(turned["cwd"], "/work/helper-a", "the session stands in the helper's own checkout");
  assert_eq!(turned["resume"], json!({ "kind": "auto" }), "a warm session goes on");
  assert_eq!(
    turned["mapping"],
    json!({ "model": "claude-sonnet-4-6", "effort": "medium", "tier": "Reliable" })
  );
  assert_eq!(turned["workKey"], "role:helper");
  assert_eq!(turned["chat"], -7);

  // The seat is finished with what the turn came to; the helper's session is kept for the next delegation.
  assert_eq!(stub.finished, [json!({ "state": "finished", "outcome": "answered", "cancel": false })]);
}

#[tokio::test]
async fn a_named_folder_is_the_one_worked_in_and_a_role_nobody_holds_is_refused() {
  let home = tempfile::tempdir().unwrap();
  let folder = home.path().join("elsewhere");
  let (api, stub) = server(Stub {
    waits: Some(0),
    answer: Some("done".into()),
    ..Stub::default()
  })
  .await;
  let named = |role: Option<&str>| DelegateInput {
    title: "Look".into(),
    instruction: "look".into(),
    role: role.map(str::to_owned),
    folder: Some(folder.to_string_lossy().into_owned()),
  };
  let answer = delegate_with(&api, &session(home.path()), named(Some("helper")), &Held::default()).await;

  assert!(serde_json::to_string(&answer).unwrap().contains("done"));
  assert!(folder.join("found.txt").exists(), "the task worked in the folder named");

  let refused = serde_json::to_string(
    &delegate_with(&api, &session(home.path()), named(Some("nobody")), &Held::default()).await,
  )
  .unwrap();

  assert!(refused.contains("holds the nobody role"), "{refused}");
  assert_eq!(stub.lock().unwrap().asked_seat.clone().unwrap()["role"], "helper", "no seat for nobody");
}

#[tokio::test]
async fn a_seat_given_back_before_it_was_let_in_ends_the_call_and_starts_nothing() {
  let home = tempfile::tempdir().unwrap();
  let (api, stub) = server(Stub {
    given_back: true,
    ..Stub::default()
  })
  .await;
  let answer = serde_json::to_string(
    &delegate_with(&api, &session(home.path()), input("look"), &Held::default()).await,
  )
  .unwrap();

  assert!(answer.contains("gave the seat back"), "{answer}");
  assert!(stub.lock().unwrap().sends.is_empty());
}

#[tokio::test]
async fn a_turn_that_answers_nothing_fails_and_its_seat_is_finished_so() {
  let home = tempfile::tempdir().unwrap();
  let (api, stub) = server(Stub {
    waits: Some(0),
    ..Stub::default()
  })
  .await;
  let answer = serde_json::to_string(
    &delegate_with(&api, &session(home.path()), input("look"), &Held::default()).await,
  )
  .unwrap();

  assert!(answer.contains("no answer"), "{answer}");

  let finished = stub.lock().unwrap().finished.clone();

  assert_eq!(finished.len(), 1);
  assert!(finished[0]["outcome"].as_str().unwrap().starts_with("failed"), "{finished:?}");
  assert_eq!(finished[0]["cancel"], false);
}

#[tokio::test]
async fn a_call_stopped_while_its_seat_waits_cancels_the_seat() {
  let home = tempfile::tempdir().unwrap();
  let (api, stub) = server(Stub::default()).await;
  let held = Held::default();
  let session = session(home.path());

  // The seat is never let in: the call waits on, and is stopped from outside.
  let waited =
    tokio::time::timeout(Duration::from_millis(300), delegate_with(&api, &session, input("look"), &held))
      .await;

  assert!(waited.is_err(), "the call waits for its seat however long the line takes");

  held.give_back(&api).await;

  let stub = stub.lock().unwrap();

  assert!(stub.admissions > 3, "it kept asking for its admission");
  assert!(stub.sends.is_empty());
  assert_eq!(
    stub.finished,
    [json!({ "state": "finished", "outcome": "cancelled: the call was stopped", "cancel": true })]
  );
}

#[tokio::test]
async fn an_empty_instruction_is_refused_before_the_pool_is_asked() {
  let home = tempfile::tempdir().unwrap();
  let (api, stub) = server(Stub::default()).await;
  let answer =
    serde_json::to_string(&delegate_with(&api, &session(home.path()), input("  "), &Held::default()).await)
      .unwrap();

  assert!(answer.contains("instruction must not be empty"), "{answer}");
  assert!(stub.lock().unwrap().asked_seat.is_none());
}

#[tokio::test]
async fn an_empty_title_is_refused_before_the_pool_is_asked() {
  let home = tempfile::tempdir().unwrap();
  let (api, stub) = server(Stub::default()).await;
  let untitled = DelegateInput {
    title: " \n ".into(),
    ..input("look")
  };
  let answer =
    serde_json::to_string(&delegate_with(&api, &session(home.path()), untitled, &Held::default()).await)
      .unwrap();

  assert!(answer.contains("title must not be empty"), "{answer}");
  assert!(stub.lock().unwrap().asked_seat.is_none());
}

// Agency's listing of the asking agent's delegations, as `GET /delegations` answers it.
async fn delegations(Query(asked): Query<BTreeMap<String, String>>) -> Json<Value> {
  assert_eq!(
    asked.get("askedBy").map(String::as_str),
    Some("bmo"),
    "only the asking agent's are read"
  );
  assert_eq!(asked.get("ended").map(String::as_str), Some("3"));

  Json(json!({ "items": [
    { "token": "t2", "title": "Read the node's disk", "askedBy": "bmo", "stage": "running",
      "agentName": "helper-a", "role": "helper", "askedAt": 1_780_000_000_000_i64, "runningSince": 1, "endedAt": null,
      "outcome": null },
    { "token": "t3", "title": "Find the slow query", "askedBy": "bmo", "stage": "waiting",
      "agentName": null, "role": "helper", "askedAt": 1_780_000_060_000_i64, "runningSince": null,
      "endedAt": null, "outcome": null },
    { "token": "t1", "title": "Check the backup", "askedBy": "bmo", "stage": "ended",
      "agentName": "helper-b", "role": "helper", "askedAt": 1_779_990_000_000_i64, "runningSince": 1,
      "endedAt": 2, "outcome": "answered" },
  ] }))
}

#[tokio::test]
async fn the_listing_names_each_delegation_by_its_title_stage_helper_and_start() {
  let home = tempfile::tempdir().unwrap();
  let app = Router::new().route("/delegations", get(delegations));
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let api = Api {
    port,
    token: None,
    pace: Duration::from_millis(10),
  };
  let answer = list_with(&api, &session(home.path()), ListInput { ended: Some(3) }).await;
  let text = serde_json::to_value(&answer).unwrap()["content"][0]["text"]
    .as_str()
    .unwrap()
    .to_owned();
  let lines: Vec<&str> = text.lines().collect();

  assert_eq!(lines.len(), 3, "{text}");
  assert!(
    lines[0].starts_with("- Read the node's disk — running on helper-a — asked 2026-"),
    "{text}"
  );
  assert!(lines[1].starts_with("- Find the slow query — waiting for a seat — asked "), "{text}");
  assert!(lines[2].contains("Check the backup — ended: answered"), "{text}");
  assert!(lines[2].ends_with("ran on helper-b"), "{text}");
}

#[tokio::test]
async fn a_turn_not_the_owner_s_delegates_only_past_the_owner_s_gate() {
  let home = tempfile::tempdir().unwrap();
  let place = home.path().join("place");

  std::fs::create_dir_all(&place).unwrap();

  let run = |privileged: &str| {
    json!({ "name": "chat:-7:1", "person": false, "env": {
      "BMO_TURN_CHAT": "-7", "BMO_TURN_MESSAGES": "11", "BMO_TURN_PRIVILEGED": privileged,
    } })
  };
  let mut turn = session(home.path());

  turn.labels.place = Some(place.clone());

  // The owner's turn goes on without any account service being asked.
  std::fs::write(place.join("run.json"), run("1").to_string()).unwrap();
  assert_eq!(may_delegate(&AccountService::default(), &turn).await, Ok(()));

  // Anybody else's is refused where the check cannot be made.
  std::fs::write(place.join("run.json"), run("0").to_string()).unwrap();

  let refused = may_delegate(&AccountService::default(), &turn).await.unwrap_err();

  assert!(refused.starts_with("Delegating a task is refused"), "{refused}");
}
