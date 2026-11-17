//! A role session's turn is a run of the ledger (docs/run-records.md, 「The run ledger」; docs/agent-control.md,
//! 「Delegating a task」): opened as the turn goes out, under the name its log lines are written under, with
//! its title and its room; listed while it goes and its log read by its id; closed as its turn ends; and
//! named by its name and id on the delegation it ran for, once that delegation has ended.

use std::sync::Arc;
use std::time::Duration;

use agency_server::control::{Front, router};
use agency_server::herdr_backend::HerdrSessions;
use agency_server::order_run::dispatch::{self, Asked, DELEGATE, Wanted, acquire, enqueue, finish};
use agency_server::places::Places;
use agency_server::run_ledger;
use agency_server::run_log::lines::Line;
use agency_server::run_log::{RunLog, Target};
use agency_server::turn_pickup;
use agent_session::{
  Apart, Mapping, Resume, Scope, SessionId, Sessions, Start, Turn, TurnContext, WorkParts,
};
use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::grounds::{deepseek_balance, seeded};
use crate::support::scratch::Scratch;

const TOKEN: &str = "secret";
const ROLE: &str = "doppelganger";

// The double's session at a folder, on Claude Code, as a delegation starts it.
async fn double_session(sessions: &HerdrSessions, folder: &std::path::Path) -> SessionId {
  sessions
    .start(Start {
      agent: ROLE.into(),
      scope: Scope::Apart(Apart {
        provider: "claude-code".into(),
        model: "claude-sonnet-5".into(),
        cwd: folder.to_string_lossy().into_owned(),
        role: Some(ROLE.into()),
        ..Apart::default()
      }),
    })
    .await
    .unwrap()
    .id
}

// The work parts of a delegated turn as the session's route lays them on it.
fn delegated(name: &str, unit: Option<&str>) -> WorkParts {
  WorkParts {
    provider: "claude-code".into(),
    model: "claude-sonnet-5".into(),
    mapping: Mapping {
      model: "claude-sonnet-5".into(),
      effort: None,
      tier: None,
      answer_tokens: None,
    },
    resume: Resume::Auto,
    cwd: String::new(),
    name: name.into(),
    title: "delegated by tester: draw the figures".into(),
    chat: -5,
    unit: unit.map(str::to_owned),
    user: None,
    work_key: ROLE.into(),
    hears_words: true,
    variables: Default::default(),
  }
}

async fn runs_of(scratch: &Scratch) -> Vec<run_ledger::Run> {
  let search = run_ledger::Search {
    limit: 10,
    agent: Some(ROLE.into()),
    ..run_ledger::Search::default()
  };

  run_ledger::search(scratch.db(), &search).await.unwrap().runs
}

#[tokio::test]
async fn a_role_session_s_turn_is_a_run_under_its_title_and_room_closed_as_its_turn_ends() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let sessions = HerdrSessions::new(Places::new(seeded(scratch.db(), home.path()).await, Arc::default()));
  let session = double_session(&sessions, home.path()).await;
  let turn = |work: WorkParts| Turn {
    session: session.clone(),
    prompt: "Draw the figures.".into(),
    context: TurnContext {
      work: Some(work),
      ..TurnContext::default()
    },
  };

  // The turn goes nowhere on a machine with no herdr, and its run still stands and closes.
  let _ = sessions.send_turn(turn(delegated("delegate-tester-1", None))).await;

  let runs = runs_of(&scratch).await;

  assert_eq!(runs.len(), 1, "{runs:?}");
  assert_eq!(runs[0].unit, "delegate-tester-1");
  assert_eq!(runs[0].title.as_deref(), Some("delegated by tester: draw the figures"));
  assert_eq!(runs[0].chat_id, Some(-5));
  assert!(runs[0].finished_at.is_some(), "the run closes as its turn ends");
  assert!(runs[0].outcome.is_some());

  // A turn naming the unit its lines stand under is a run under that unit.
  let _ = sessions
    .send_turn(turn(delegated("delegate-tester-2", Some("a-unit-of-its-own"))))
    .await;

  assert_eq!(runs_of(&scratch).await[0].unit, "a-unit-of-its-own");
}

async fn call(port: u16, method: reqwest::Method, path: &str, body: Option<Value>) -> (u16, Value) {
  let mut request = reqwest::Client::new()
    .request(method, format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(TOKEN);

  if let Some(body) = body {
    request = request.json(&body);
  }

  let answer = request.send().await.unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_delegation_is_listed_among_the_runs_and_its_log_read_by_its_id() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let places = Places::new(seeded(scratch.db(), home.path()).await, Arc::default());
  let front = Front::new(TOKEN.to_owned(), places.ground.clone()).unwrap().with_places(places, None);
  let session = double_session(&front.sessions().unwrap(), home.path()).await;
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();
  let app = router(front);

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  // The turn is held just past its run's opening: the key it is kept in flight under is taken by a write
  // the test has not committed, so the turn goes on only once the test lets that write go.
  let run = "delegate-tester-1";
  let hold = scratch.db().begin().await.unwrap();

  hold
    .execute(Statement::from_sql_and_values(
      DbBackend::Postgres,
      r#"INSERT INTO "settings" ("key", "value") VALUES ($1, '{}')"#,
      vec![format!("{}{run}", turn_pickup::PREFIX).into()],
    ))
    .await
    .unwrap();

  // The turn as the delegate sends it.
  let body = json!({
    "prompt": "Work in the folder.\n\nDraw the figures.",
    "resume": { "kind": "auto" },
    "name": run,
    "title": "delegated by tester: draw the figures",
    "chat": -5,
    "hearsWords": true,
    "variables": {},
  });
  let path = format!("/sessions/{}/turns", session.0);
  let sent = tokio::spawn(async move { call(port, reqwest::Method::POST, &path, Some(body)).await });
  let mut listed = Value::Null;

  for _ in 0..200 {
    let (_, page) = call(port, reqwest::Method::GET, "/runs?live=true&agentName=doppelganger", None).await;

    if page["items"].as_array().is_some_and(|items| !items.is_empty()) {
      listed = page["items"][0].clone();
      break;
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
  }

  assert_eq!(
    listed["unitName"], run,
    "the running delegation is listed among the open runs: {listed}"
  );
  assert_eq!(listed["title"], "delegated by tester: draw the figures");
  assert_eq!(listed["roomId"], -5);
  assert_eq!(listed["state"], "open");

  // A line of its transcript, written under its run as the runner writes it, is read by the run's id.
  let id = listed["id"].as_i64().unwrap();

  RunLog::new(Target {
    agent: ROLE.into(),
    run: run.into(),
    chat_id: Some(-5),
    job_id: None,
  })
  .write(scratch.db(), Line::note("drew the first figure"))
  .await;

  let (status, lines) = call(port, reqwest::Method::GET, &format!("/log-lines?runId={id}"), None).await;

  assert_eq!(status, 200, "{lines}");
  assert!(
    lines["items"]
      .as_array()
      .unwrap()
      .iter()
      .any(|line| line["text"] == "drew the first figure" && line["runId"] == id),
    "{lines}"
  );

  // Let go, the turn ends on a machine with no herdr, and its run is closed with how it ended.
  hold.rollback().await.unwrap();
  tokio::time::timeout(Duration::from_secs(60), sent)
    .await
    .expect("the turn ended")
    .unwrap();

  let (_, closed) = call(port, reqwest::Method::GET, &format!("/runs/{id}"), None).await;

  assert_eq!(closed["state"], "closed", "{closed}");
  assert!(closed["outcome"].is_string(), "{closed}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_ended_delegation_is_listed_with_the_run_its_turn_went_out_as() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;

  // The slot as the delegate asks it, let in, laid on its role session and bound under the run's name.
  let asked = Asked {
    request_key: "delegate:tester:1".into(),
    owner: "delegate:tester:1".into(),
    title: Some("draw the figures"),
    asked_by: Some("tester"),
    wanted: Wanted {
      role: ROLE,
      path: Some("/scratch/tester"),
      fixed: Some(agency_server::order::Entry {
        provider: "deepseek".into(),
        model: "deepseek-flash".into(),
        effort: None,
        thinking: None,
        answer_tokens: None,
      }),
      agent: None,
    },
    ..Asked::default()
  };
  let held = enqueue(&ground.db, &ground.events, &asked, DELEGATE, async |_, _| Ok(()))
    .await
    .unwrap();

  assert!(acquire(&ground, &held).await.unwrap().is_some(), "the delegation is let in");

  let run = "delegate-tester-1";

  dispatch::bind_session(&ground.db, &ground.events, &held.token, "role-session-1")
    .await
    .unwrap();
  dispatch::bind(
    &ground.db,
    &ground.events,
    &dispatch::request(&ground.db, &held.token).await.unwrap().unwrap(),
    run,
    None,
  )
  .await
  .unwrap();

  // Its turn opens the run in the ledger, and the delegation ends.
  let id = run_ledger::open(
    scratch.db(),
    &run_ledger::Opening {
      agent: ROLE.into(),
      chat_id: None,
      unit: run.into(),
      depth: 0,
      cap_ms: None,
      started_at: 1,
      started_by: None,
      search_id: None,
      round: None,
      message_id: None,
      title: Some("delegated by tester: draw the figures".into()),
      carried: true,
    },
  )
  .await
  .unwrap();

  assert!(finish(&ground.db, &ground.events, &held.token, "answered", false).await.unwrap());

  let front = Front::new(TOKEN.to_owned(), Arc::new(ground)).unwrap();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();
  let app = router(front);

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let (status, page) = call(port, reqwest::Method::GET, "/delegations?askedBy=tester", None).await;

  assert_eq!(status, 200, "{page}");

  let listed = &page["items"][0];

  assert_eq!(listed["stage"], "ended", "{page}");
  assert_eq!(listed["runName"], run, "the run its log is read by stays once it ends: {page}");
  assert_eq!(listed["runId"], id, "{page}");
}
