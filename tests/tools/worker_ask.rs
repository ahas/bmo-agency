//! The worker ask: a free worker's turn carries the words he was given and the files it changed; an unknown
//! worker, a busy one, a worker's own checkout outside the owner's turn and a worker on a node are refused
//! (docs/tools.md, 「The worker ask」).

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use tool_call::Session;
use worker_ask_tool::{Api, AskInput, ask_with};

#[derive(Clone)]
struct Stub {
  folder: String,
}

async fn worker(State(stub): State<Stub>, Path(name): Path<String>) -> (StatusCode, Json<Value>) {
  let body = match name.as_str() {
    "helper" => json!({ "state": "idle", "sessionId": "s1", "cwd": "/work/helper" }),
    "busy" => json!({ "state": "working", "sessionId": "s1", "cwd": "/work/busy" }),
    "node" => {
      json!({ "state": "idle", "sessionId": "s1", "cwd": "/work/node", "nodeName": "n1" })
    }
    "own" => json!({ "state": "idle", "sessionId": "s1", "cwd": stub.folder }),
    _ => return (StatusCode::NOT_FOUND, Json(json!({ "error": "no worker" }))),
  };

  (StatusCode::OK, Json(body))
}

async fn turn(State(stub): State<Stub>) -> (StatusCode, Json<Value>) {
  (
    StatusCode::OK,
    Json(json!({ "answer": format!("Done in {}.", stub.folder), "state": "ended" })),
  )
}

async fn lines(State(stub): State<Stub>) -> Json<Value> {
  Json(json!({ "lines": [
    { "kind": "bashEditDiff", "path": format!("{}/a.rs", stub.folder) },
    { "kind": "edit", "path": format!("{}/b.rs", stub.folder) }
  ] }))
}

async fn server(stub: Stub) -> u16 {
  let app = Router::new()
    .route("/workers/{name}", get(worker))
    .route("/sessions/{id}/turns", post(turn))
    .route("/log/agents/{name}/lines", get(lines))
    .with_state(stub);
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  port
}

fn session(root: &std::path::Path) -> Session {
  Session {
    variables: BTreeMap::from([
      ("BMO_DATA_DIR".to_owned(), root.to_string_lossy().into_owned()),
      ("BMO_TURN_AGENT".to_owned(), "bmo".to_owned()),
    ]),
    ..Session::default()
  }
}

#[tokio::test]
async fn a_round_trip_answers_with_the_words_and_the_files_the_turn_changed() {
  let home = tempfile::tempdir().unwrap();
  let folder = home.path().join("scratch");
  std::fs::create_dir_all(&folder).unwrap();
  let port = server(Stub {
    folder: folder.to_string_lossy().into_owned(),
  })
  .await;
  let answer = ask_with(
    &Api { port, token: None },
    &session(home.path()),
    AskInput {
      worker: "helper".into(),
      words: "add a file".into(),
      folder: Some(folder.to_string_lossy().into_owned()),
      time_bound_seconds: Some(30),
    },
  )
  .await;

  let answer = serde_json::to_string(&answer).unwrap();

  assert!(answer.contains("Done in"), "{answer}");
  assert!(answer.contains("a.rs"), "{answer}");
  assert!(answer.contains("b.rs"), "{answer}");
}

#[tokio::test]
async fn a_busy_unknown_or_noded_worker_is_refused() {
  let home = tempfile::tempdir().unwrap();
  let folder = home.path().join("scratch");
  std::fs::create_dir_all(&folder).unwrap();
  let port = server(Stub {
    folder: folder.to_string_lossy().into_owned(),
  })
  .await;
  let asked = |worker: &str| AskInput {
    worker: worker.into(),
    words: "do it".into(),
    folder: Some(folder.to_string_lossy().into_owned()),
    time_bound_seconds: Some(30),
  };

  for (worker, said) in [
    ("busy", "busy"),
    ("ghost", "no worker"),
    ("node", "stands on a node"),
    ("own", "own checkout"),
  ] {
    let answer = serde_json::to_string(
      &ask_with(&Api { port, token: None }, &session(home.path()), asked(worker)).await,
    )
    .unwrap();

    assert!(answer.contains(said), "{worker}: {answer}");
  }
}
