//! The delegate: a slot of the role asked — the delegator role where the ask names none — and waited for in
//! its line however long that takes, one run of the role started at the task's folder on the road the slot
//! carries, waited on with no bound, the slot bound to the run and finished, and the answer the run's words
//! with the files it created or changed in the task's folder; a slot given back before it was let in, a role
//! the store does not hold, a run that answers nothing and a turn not the owner's past the gate are refused,
//! and a stopped call gives its slot back (docs/tools.md, 「The delegate」).

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

// How the stub slot and session routes behave, and everything they were asked.
#[derive(Default)]
struct Stub {
  // How many admissions answer the slot not let in before one lets it in; none lets it in where unset.
  waits: Option<usize>,
  // Whether the slot reads as given back while it is not let in.
  given_back: bool,
  // The answer the turn ends with.
  answer: Option<String>,
  // How long the turn works before it answers.
  works: Duration,
  asked_seat: Option<Value>,
  started: Option<Value>,
  admissions: usize,
  bound: Option<Value>,
  finished: Vec<Value>,
  sends: Vec<String>,
  turned: Option<Value>,
  // What the double's record holds, as the start answers it; everything the asking record holds where unset.
  carries: Option<Value>,
}

type Shared = Arc<Mutex<Stub>>;

// The slot as agency answers it: the double's road is the asking supervisor's, fixed at its ask.
fn seat(state: &str) -> Value {
  json!({
    "token": "t1", "state": state, "role": "delegator", "provider": "deepseek", "model": "deepseek-flash",
    "effort": "high",
  })
}

// A role the store does not hold is refused as agency refuses it.
async fn ask_seat(State(stub): State<Shared>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
  if body["role"] == "nobody" {
    return (
      StatusCode::UNPROCESSABLE_ENTITY,
      Json(json!({ "code": "unknown-role", "error": "No role named nobody" })),
    );
  }

  stub.lock().unwrap().asked_seat = Some(body);

  (StatusCode::CREATED, Json(seat("waiting")))
}

async fn start(State(stub): State<Shared>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
  let mut stub = stub.lock().unwrap();
  let carries = stub
    .carries
    .clone()
    .unwrap_or_else(|| json!({ "capabilities": ["room.tools"], "namespaces": ["bmo"] }));

  stub.started = Some(body);

  (StatusCode::CREATED, Json(json!({ "id": "delegator@run:1", "carries": carries })))
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

// The run works for a while, writes a file into the folder its words name, and answers.
async fn turn(
  State(stub): State<Shared>,
  Path(session): Path<String>,
  Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
  let (works, answer) = {
    let mut stub = stub.lock().unwrap();

    stub.sends.push(session);
    stub.turned = Some(body.clone());
    (stub.works, stub.answer.clone())
  };

  tokio::time::sleep(works).await;

  let prompt = body["prompt"].as_str().unwrap();
  let folder = prompt.strip_prefix("Work in ").and_then(|rest| rest.split_once(".\n")).unwrap().0;

  std::fs::write(std::path::Path::new(folder).join("found.txt"), "what the run found").unwrap();

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
    .route("/slot-requests", post(ask_seat))
    .route("/slot-requests/{token}", patch(move_seat).get(read_seat))
    .route("/sessions", post(start))
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
      ("BMO_TURN_VIA".to_owned(), "telegram".to_owned()),
    ]),
    capabilities: ["room.tools".to_owned()].into(),
    namespaces: ["bmo".to_owned()].into(),
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
async fn a_slot_waits_in_line_and_the_roles_run_answers_with_its_words_and_the_files_it_made() {
  let home = tempfile::tempdir().unwrap();
  let scratch = home.path().join("agent-workspaces").join("bmo").join("scratch");

  std::fs::create_dir_all(&scratch).unwrap();
  std::fs::write(scratch.join("kept.txt"), "here before the task").unwrap();

  let (api, stub) = server(Stub {
    waits: Some(5),
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
  assert!(text.contains("found.txt"), "the file the run made is named: {text}");
  assert!(!text.contains("kept.txt"), "a file the run left alone is not: {text}");

  let stub = stub.lock().unwrap();
  let asked = stub.asked_seat.clone().unwrap();

  // No role named: agency takes it as the delegator's, on the asking supervisor's own road.
  assert!(asked.get("role").is_none(), "{asked}");
  assert!(asked.get("provider").is_none(), "the road is agency's to fix: {asked}");
  assert_eq!(asked["source"], "delegate");
  assert_eq!(asked["requestKey"], asked["owner"]);
  // It is asked under its title, the asking supervisor and the task's folder.
  assert_eq!(asked["title"], "Read the node's disk");
  assert_eq!(asked["askedBy"], "bmo");
  assert_eq!(asked["path"], scratch.display().to_string());

  // It asked for its admission as long as the line held it, and only then started the run.
  assert_eq!(stub.admissions, 6);

  // The run is the slot's role at the task's folder, on the road the slot was let in on.
  let started = stub.started.clone().unwrap();

  assert_eq!(started["scope"], "role");
  assert_eq!(started["slot"], "t1", "the run starts on the slot it was let in on");
  assert_eq!(started["supervisor"], "bmo", "the run is the asking supervisor's double");
  assert_eq!(started["cwd"], scratch.display().to_string());
  assert!(started.get("options").is_none(), "the role and the road are the slot's");
  assert_eq!(stub.sends, ["delegator@run:1"]);

  let turned = stub.turned.clone().unwrap();
  let work = &turned;
  let run = work["name"].as_str().unwrap();

  assert_eq!(stub.bound.clone().unwrap()["childId"], run, "the slot names the run it carries");
  assert!(
    stub.bound.clone().unwrap().get("agentName").is_none(),
    "the slot stays on the session its start bound it to"
  );
  assert!(
    turned["prompt"]
      .as_str()
      .unwrap()
      .starts_with(&format!("Work in {}.", scratch.display()))
  );
  assert!(turned["prompt"].as_str().unwrap().ends_with("ssh to the node and read its disk"));
  for retired in ["cwd", "provider", "model", "mapping", "workKey"] {
    assert!(
      work.get(retired).is_none(),
      "the turn names no {retired}: the session's road and folder stand"
    );
  }
  assert_eq!(work["chat"], -7);
  assert_eq!(
    work["variables"],
    json!({ "BMO_TURN_CHAT": "-7", "BMO_TURN_VIA": "telegram" }),
    "the double's turn stands in the asking turn's room on its road"
  );
  assert!(!answer.is_error, "the double held all its supervisor holds: {text}");

  // The slot is finished with what the run came to.
  assert_eq!(stub.finished, [json!({ "state": "finished", "outcome": "answered", "cancel": false })]);
}

#[tokio::test]
async fn a_named_folder_is_the_one_worked_in_and_a_role_the_store_does_not_hold_is_refused() {
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

  assert!(refused.contains("No role named nobody"), "{refused}");
  assert_eq!(stub.lock().unwrap().asked_seat.clone().unwrap()["role"], "helper", "no slot for nobody");
}

#[tokio::test]
async fn a_slot_given_back_before_it_was_let_in_ends_the_call_and_starts_nothing() {
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

  assert!(answer.contains("given back"), "{answer}");
  assert!(stub.lock().unwrap().started.is_none());
  assert!(stub.lock().unwrap().sends.is_empty());
}

#[tokio::test]
async fn a_run_that_answers_nothing_fails_and_its_slot_is_finished_so() {
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
async fn a_call_stopped_while_its_slot_waits_cancels_the_slot() {
  let home = tempfile::tempdir().unwrap();
  let (api, stub) = server(Stub::default()).await;
  let held = Held::default();
  let session = session(home.path());

  // The slot is never let in: the call waits on, and is stopped from outside.
  let waited =
    tokio::time::timeout(Duration::from_millis(300), delegate_with(&api, &session, input("look"), &held))
      .await;

  assert!(waited.is_err(), "the call waits for its slot however long the line takes");

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
async fn an_empty_instruction_is_refused_before_a_slot_is_asked() {
  let home = tempfile::tempdir().unwrap();
  let (api, stub) = server(Stub::default()).await;
  let answer =
    serde_json::to_string(&delegate_with(&api, &session(home.path()), input("  "), &Held::default()).await)
      .unwrap();

  assert!(answer.contains("instruction must not be empty"), "{answer}");
  assert!(stub.lock().unwrap().asked_seat.is_none());
}

#[tokio::test]
async fn an_empty_title_is_refused_before_a_slot_is_asked() {
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

// Agency's listing of the asking supervisor's delegations, as `GET /delegations` answers it.
async fn delegations(Query(asked): Query<BTreeMap<String, String>>) -> Json<Value> {
  assert_eq!(
    asked.get("askedBy").map(String::as_str),
    Some("bmo"),
    "only the asking supervisor's are read"
  );
  assert_eq!(asked.get("ended").map(String::as_str), Some("3"));

  Json(json!({ "items": [
    { "token": "t2", "title": "Read the node's disk", "askedBy": "bmo", "stage": "running",
      "role": "delegator", "provider": "deepseek", "model": "deepseek-flash", "askedAt": 1_780_000_000_000_i64,
      "runningSince": 1, "endedAt": null, "outcome": null },
    { "token": "t3", "title": "Find the slow query", "askedBy": "bmo", "stage": "waiting",
      "role": "helper", "provider": null, "model": null, "askedAt": 1_780_000_060_000_i64,
      "runningSince": null, "endedAt": null, "outcome": null },
    { "token": "t1", "title": "Check the backup", "askedBy": "bmo", "stage": "ended",
      "role": "delegator", "provider": "claude-code", "model": "claude-opus-5", "askedAt": 1_779_990_000_000_i64,
      "runningSince": 1, "endedAt": 2, "outcome": "answered" },
  ] }))
}

#[tokio::test]
async fn the_listing_names_each_delegation_by_its_title_stage_role_road_and_start() {
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
    lines[0]
      .starts_with("- Read the node's disk — running — delegator on deepseek deepseek-flash — asked 2026-"),
    "{text}"
  );
  assert!(
    lines[1].starts_with("- Find the slow query — waiting for a slot — helper — asked "),
    "{text}"
  );
  assert!(
    lines[2].starts_with("- Check the backup — ended: answered — delegator on claude-code claude-opus-5"),
    "{text}"
  );
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

// Where the double's session would not hold what the supervisor's own holds, the delegation still runs and its
// answer is a failure naming each missing capability and namespace before the run's words, so the turn's log
// shows a failed call (docs/tools.md, 「The delegate」).
#[tokio::test]
async fn what_the_double_lacked_of_its_supervisor_s_tools_is_a_failure_naming_it() {
  let home = tempfile::tempdir().unwrap();
  let (api, _stub) = server(Stub {
    waits: Some(0),
    answer: Some("I could not render the song: no tool for it.".into()),
    carries: Some(json!({ "capabilities": ["room.tools"], "namespaces": ["bmo", "worker"] })),
    ..Stub::default()
  })
  .await;
  let mut asking = session(home.path());

  asking.capabilities.insert("room.send".to_owned());
  asking.namespaces.insert("voice".to_owned());

  let answer = delegate_with(&api, &asking, input("render the song"), &Held::default()).await;
  let text = serde_json::to_string(&answer).unwrap();

  assert!(text.contains("\"isError\":true"), "a defect, never a polite refusal: {text}");
  assert!(text.contains("the capability room.send"), "{text}");
  assert!(text.contains("the namespace voice"), "{text}");
  assert!(
    !text.contains("the namespace worker"),
    "what the double holds beyond is no lack: {text}"
  );
  assert!(text.contains("I could not render the song"), "the run's words follow: {text}");
}
