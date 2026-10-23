//! A chat turn's run in the ledger (docs/run-records.md, 「The run ledger」): a turn into a room's session
//! opens a run under the turn's run name with the room it runs for, the round its context names and the
//! latest message it answers, closes it however the turn ends, and the runs and log-lines routes answer it
//! by its room, its round and its id; the message its answer went out as is attached apart from its closing.

use std::collections::HashMap;
use std::sync::Arc;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::herdr_backend::HerdrSessions;
use agency_server::places::{Ground, Places};
use agency_server::run_ledger;
use agency_server::seeds::deployment;
use agent_session::{RoomBlock, Scope, Sessions, Start, Turn, TurnContext};
use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

fn machine(home: &std::path::Path) -> harness_providers::Machine {
  let at = |path: std::path::PathBuf| path.to_string_lossy().into_owned();

  harness_providers::Machine::with_env(
    [
      ("HOME".to_owned(), at(home.to_path_buf())),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
    ],
    home.join("data"),
  )
}

fn ground(scratch: &Scratch, machine: &harness_providers::Machine) -> Ground {
  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());

  Ground::new(&service, machine, None)
}

#[tokio::test]
async fn a_chat_turn_s_run_carries_its_room_its_round_and_the_message_it_answers() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let machine = machine(home.path());

  store::seed::settle(scratch.db(), &deployment(&machine)).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "session_scope", "turn_shape", "default_answerer")
         VALUES ('chat-agent', 'chat', 'room', 'window', 1)"#,
      vec![],
    )
    .await;

  let sessions = HerdrSessions::new(Places::new(ground(&scratch, &machine), Arc::default()));
  let session = sessions
    .start(Start {
      agent: "chat-agent".into(),
      scope: Scope::Room("-5".into()),
    })
    .await
    .unwrap();

  // The turn goes nowhere on a machine with no herdr, and its run still stands and closes.
  let _ = sessions
    .send_turn(Turn {
      session: session.id.clone(),
      prompt: "hello".into(),
      context: TurnContext {
        room: Some(RoomBlock {
          chat_id: -5,
          text: "the room's block".into(),
          mark: "[a]".into(),
        }),
        round: Some("round:-5:1000:1".into()),
        answers: vec![7, 9, 8],
        ..TurnContext::default()
      },
    })
    .await;

  let runs = run_ledger::search(
    scratch.db(),
    &run_ledger::Search {
      limit: 10,
      round: Some("round:-5:1000:1".into()),
      ..run_ledger::Search::default()
    },
  )
  .await
  .unwrap();

  assert_eq!(runs.len(), 1, "{runs:?}");

  let run = &runs[0];

  assert_eq!(run.agent, "chat-agent");
  assert!(run.unit.starts_with("chat:-5:"), "{}", run.unit);
  assert_eq!(run.chat_id, Some(-5));
  assert_eq!(run.round.as_deref(), Some("round:-5:1000:1"));
  assert_eq!(run.message_id, Some(9), "the latest message the turn answers");
  assert!(run.finished_at.is_some(), "the run closes as its turn ends");
  assert!(run.outcome.is_some());
}

async fn ask(
  port: u16,
  token: &str,
  method: reqwest::Method,
  path: &str,
  body: Option<Value>,
) -> (u16, Value) {
  let mut request = reqwest::Client::new()
    .request(method, format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token);

  if let Some(body) = body {
    request = request.json(&body);
  }

  let answer = request.send().await.unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn the_routes_answer_a_chat_run_by_its_room_and_round_and_its_lines_by_its_id() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let id = run_ledger::open(
    scratch.db(),
    &run_ledger::Opening {
      agent: "chat-agent".into(),
      chat_id: Some(-5),
      unit: "chat:-5:1000".into(),
      depth: 0,
      cap_ms: None,
      started_at: 1,
      started_by: None,
      search_id: None,
      round: Some("round:-5:1000:1".into()),
      message_id: Some(9),
    },
  )
  .await
  .unwrap();

  run_ledger::open(
    scratch.db(),
    &run_ledger::Opening {
      agent: "chat-agent".into(),
      chat_id: Some(-6),
      unit: "chat:-6:1000".into(),
      depth: 0,
      cap_ms: None,
      started_at: 1,
      started_by: None,
      search_id: None,
      round: Some("round:-6:1000:2".into()),
      message_id: None,
    },
  )
  .await
  .unwrap();
  scratch
    .run(
      r#"INSERT INTO "agent_log" ("agent", "run", "chat_id", "at", "kind", "text")
         VALUES ('chat-agent', 'chat:-5:1000', -5, $1, 'tool', 'called'),
                ('chat-agent', 'chat:-6:1000', -6, $1, 'tool', 'elsewhere')"#,
      // Written now, so the server's prune of old lines as it starts leaves them.
      vec![(agency_server::now_ms() / 1000).into()],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let get = |path: &'static str| ask(port, token, reqwest::Method::GET, path, None);

  let (status, by_round) = get("/runs?round=round:-5:1000:1").await;

  assert_eq!(status, 200, "{by_round}");
  assert_eq!(by_round["items"].as_array().unwrap().len(), 1);
  assert_eq!(by_round["items"][0]["id"], id);
  assert_eq!(by_round["items"][0]["roomId"], -5);
  assert_eq!(by_round["items"][0]["round"], "round:-5:1000:1");
  assert_eq!(by_round["items"][0]["messageId"], 9);

  let (_, by_room) = get("/runs?roomId=-6").await;

  assert_eq!(by_room["items"].as_array().unwrap().len(), 1);
  assert_eq!(by_room["items"][0]["round"], "round:-6:1000:2");

  let (_, by_names) = get("/runs?names=round:-5:1000:1&limit=1").await;

  assert_eq!(by_names["items"][0]["id"], id, "a run's names are its unit and its round");

  let lines_path = format!("/log-lines?runId={id}&agentName=chat-agent");
  let (_, lines) = ask(port, token, reqwest::Method::GET, &lines_path, None).await;

  assert_eq!(lines["items"].as_array().unwrap().len(), 1, "{lines}");
  assert_eq!(lines["items"][0]["text"], "called");
  assert_eq!(lines["items"][0]["runId"], id);
  assert_eq!(lines["items"][0]["roomId"], -5);
  assert_eq!(lines["items"][0]["round"], "round:-5:1000:1");
  assert_eq!(lines["items"][0]["messageId"], 9);

  // The answer's message attached apart from the closing, on a closed run alike.
  let run_path = format!("/runs/{id}");
  let (status, closed) =
    ask(port, token, reqwest::Method::PATCH, &run_path, Some(json!({ "outcome": "answered" }))).await;

  assert_eq!(status, 200, "{closed}");

  let (status, attached) =
    ask(port, token, reqwest::Method::PATCH, &run_path, Some(json!({ "messageId": 12 }))).await;

  assert_eq!(status, 200, "{attached}");
  assert_eq!(attached["messageId"], 12);
  assert_eq!(attached["outcome"], "answered");
  assert_eq!(attached["state"], "closed");
  assert_eq!(
    ask(port, token, reqwest::Method::PATCH, "/runs/999999", Some(json!({ "messageId": 12 })))
      .await
      .0,
    404
  );
}
