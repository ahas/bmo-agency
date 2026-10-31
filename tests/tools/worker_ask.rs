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
use tool_call::room_gate::{AccountService, room_turn_of_place};
use worker_ask_tool::{Api, AskInput, ask_with, may_ask};

#[derive(Clone)]
struct Stub {
  folder: String,
}

async fn worker(State(stub): State<Stub>, Path(name): Path<String>) -> (StatusCode, Json<Value>) {
  let body = match name.as_str() {
    "helper" => json!({ "state": "idle", "sessionId": "s1", "cwd": "/work/helper" }),
    "fresh" => json!({ "state": "idle", "sessionId": null, "cwd": "/work/fresh" }),
    "busy" => json!({ "state": "working", "sessionId": "s1", "cwd": "/work/busy" }),
    "node" => {
      json!({ "state": "idle", "sessionId": "s1", "cwd": "/work/node", "nodeName": "n1" })
    }
    "own" => json!({ "state": "idle", "sessionId": "s1", "cwd": stub.folder }),
    _ => return (StatusCode::NOT_FOUND, Json(json!({ "error": "no worker" }))),
  };

  (StatusCode::OK, Json(body))
}

async fn turn(Path(id): Path<String>, State(stub): State<Stub>) -> (StatusCode, Json<Value>) {
  (
    StatusCode::OK,
    Json(json!({ "answer": format!("Done in {} via {id}.", stub.folder), "state": "ended" })),
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
async fn a_worker_that_holds_no_session_yet_is_given_a_turn_into_its_own() {
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
      worker: "fresh".into(),
      words: "add a file".into(),
      folder: Some(folder.to_string_lossy().into_owned()),
      time_bound_seconds: Some(30),
    },
  )
  .await;

  let answer = serde_json::to_string(&answer).unwrap();

  assert!(answer.contains("via fresh@worker"), "{answer}");
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

// An account service answering `POST /checks/message`: the messages it allows, every check it was asked kept.
async fn accounts(
  allowed: &'static [i64],
) -> (AccountService, std::sync::Arc<std::sync::Mutex<Vec<Value>>>) {
  let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
  let kept = asked.clone();
  let app = Router::new().route(
    "/checks/message",
    post(move |Json(check): Json<Value>| {
      let kept = kept.clone();

      async move {
        let message = check["message"].as_i64().unwrap_or_default();

        kept.lock().unwrap().push(check);
        Json(json!({ "allowed": allowed.contains(&message), "grant": "bmo_room:-7#launch" }))
      }
    }),
  );
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  (
    AccountService {
      address,
      token: "web-token".into(),
    },
    asked,
  )
}

// A session whose place carries a run with these turn variables, or a person's run where none are named.
fn turn_session(root: &std::path::Path, env: Option<Value>) -> Session {
  let place = root.join("place");

  std::fs::create_dir_all(&place).unwrap();

  let run = match env {
    Some(env) => json!({ "name": "chat:-7:1", "person": false, "env": env }),
    None => json!({ "name": null, "person": true }),
  };

  std::fs::write(place.join("run.json"), run.to_string()).unwrap();

  let mut session = session(root);

  session.labels.place = Some(place);
  session
}

#[tokio::test]
async fn a_worker_is_asked_for_a_room_s_words_only_past_the_owner_s_gate() {
  let home = tempfile::tempdir().unwrap();
  let room = |messages: &str, privileged: &str| {
    Some(json!({ "BMO_TURN_CHAT": "-7", "BMO_TURN_MESSAGES": messages, "BMO_TURN_PRIVILEGED": privileged }))
  };

  // The owner's turn, and a person's typed words, are never asked about: no account service is even named.
  let none = AccountService::default();

  assert_eq!(may_ask(&none, &turn_session(home.path(), room("11", "1"))).await, Ok(()));
  assert_eq!(may_ask(&none, &turn_session(home.path(), None)).await, Ok(()));
  assert_eq!(room_turn_of_place(&home.path().join("place")), None);

  // Anybody else's turn: every message checked for `launch` in the room.
  let (service, asked) = accounts(&[11, 12]).await;

  assert_eq!(may_ask(&service, &turn_session(home.path(), room("11,12", "0"))).await, Ok(()));
  assert_eq!(
    *asked.lock().unwrap(),
    [
      json!({ "room": -7, "message": 11, "relation": "launch" }),
      json!({ "room": -7, "message": 12, "relation": "launch" }),
    ]
  );

  // One message whose sender holds no grant refuses the whole ask, saying whose.
  let refused = may_ask(&service, &turn_session(home.path(), room("11,13", "0"))).await.unwrap_err();

  assert!(refused.contains("message 13") && refused.contains("`launch`"), "{refused}");

  // A check that cannot be made refuses: no service named, or none answering, or no message to check.
  for (service, messages) in [
    (AccountService::default(), "11"),
    (
      AccountService {
        address: "http://127.0.0.1:9".into(),
        token: String::new(),
      },
      "11",
    ),
    (service.clone(), ""),
  ] {
    let refused = may_ask(&service, &turn_session(home.path(), room(messages, "0"))).await.unwrap_err();

    assert!(refused.starts_with("Asking a worker is refused"), "{refused}");
  }
}

#[test]
fn the_account_service_is_read_from_the_environment_or_the_web_unit_s_env_file() {
  let root = tempfile::tempdir().unwrap();

  std::fs::create_dir_all(root.path().join("env")).unwrap();
  std::fs::write(
    root.path().join("env").join("web.env"),
    "WEB_ADDRESS=http://127.0.0.1:4/\nWEB_TOKEN=from-file\n",
  )
  .unwrap();

  assert_eq!(
    AccountService::read(root.path(), |_| None),
    AccountService {
      address: "http://127.0.0.1:4".into(),
      token: "from-file".into(),
    }
  );

  let token = root.path().join("web.token");

  std::fs::write(&token, "in-a-file\n").unwrap();

  let token = token.to_string_lossy().into_owned();
  let named = AccountService::read(root.path(), |name| match name {
    "WEB_ADDRESS" => Some("http://127.0.0.1:5".into()),
    "WEB_TOKEN_FILE" => Some(token.clone()),
    _ => None,
  });

  assert_eq!(named.address, "http://127.0.0.1:5");
  assert_eq!(named.token, "from-file", "a token by value in the env file stands before a file named");
}
