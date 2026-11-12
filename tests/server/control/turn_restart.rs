//! A turn across agency's own restart, proved on the server's own process (docs/sessions.md, 「A restart
//! hands the session over」; docs/api.md, 「sessions」, 「health」): a keyed turn the process before left in
//! flight is picked up at the start, answers the progress read under its own id and takes a repeat of its
//! send under its key; the server stopped by `SIGTERM` while that send waits answers it `503` `stopping` with
//! the key and ends; the server started again picks the turn up once more, the send asked again under the
//! key attaches to it and is answered with its end once its program writes it, the end published once under
//! its id and its key; and a repeat after the end is answered with it at once.

use std::path::Path;
use std::time::Duration;

use agency_server::lines::place::Place;
use agency_server::now_ms;
use agency_server::turn_pickup::{InFlight, keep};
use serde_json::{Value, json};

use crate::support::scratch::Scratch;
use crate::support::standing::{Standing, stand_server};

const TOKEN: &str = "restart-token";
const SESSION: &str = "agent-b@conversation:9";

// A line of the pane program's record, as bmo-tui appends it.
fn append(place: &Place, line: Value) {
  use std::io::Write as _;

  std::fs::create_dir_all(&place.path).unwrap();

  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(place.path.join("loop.jsonl"))
    .unwrap();

  writeln!(file, "{line}").unwrap();
}

// The keyed send, answered whenever the server answers it.
async fn send(port: u16) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .post(format!("http://127.0.0.1:{port}/sessions/{SESSION}/turns"))
    .bearer_auth(TOKEN)
    .json(&json!({ "prompt": "What is the disk?", "key": "round-7" }))
    .send()
    .await
    .unwrap();
  let status = answer.status().as_u16();

  (status, answer.json().await.unwrap_or(Value::Null))
}

async fn read(port: u16, path: &str) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .get(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(TOKEN)
    .header("Accept", "application/json")
    .send()
    .await
    .unwrap();
  let status = answer.status().as_u16();

  (status, answer.json().await.unwrap_or(Value::Null))
}

fn server(home: &Path, scratch: &Scratch) -> Standing {
  stand_server(home, &home.join("data"), &scratch.address(), TOKEN)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_keyed_turn_outlives_two_restarts_of_the_server_and_its_send_asked_again_gets_its_end() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let place = Place::of(&home.path().join("data"), "agent-b");
  let mut program = std::process::Command::new("sleep").arg("300").spawn().unwrap();
  let identity = agent_host::identity::ProcessIdentity::of(program.id() as i32).unwrap();

  // The turn a process before this one sent and its program took: kept in flight under its key, its
  // program standing and its record naming it.
  std::fs::create_dir_all(&place.path).unwrap();
  std::fs::write(
    place.path.join("running.json"),
    json!({ "identity": "one", "pid": identity.pid, "birth": identity.birth, "loop": identity,
            "starts": 1, "startedAt": now_ms(), "recent": [] })
    .to_string(),
  )
  .unwrap();
  keep(
    scratch.db(),
    &InFlight {
      session: SESSION.into(),
      turn: 43,
      run: "chat:5:3".into(),
      key: Some("round-7".into()),
      agent: "agent-b".into(),
      chat_id: Some(5),
      ledger: None,
      job: None,
      started_at: now_ms() - 1_000,
    },
  )
  .await;
  append(
    &place,
    json!({ "kind": "start", "id": "w", "name": "chat:5:3", "title": "a turn", "at": 1 }),
  );

  // The first server picks it up: the progress read answers it running under its id, and nothing is left
  // that a restart would cut.
  let mut first = server(home.path(), &scratch);
  let (status, reading) = read(first.port, &format!("/sessions/{SESSION}/turns/43")).await;

  assert_eq!(status, 200, "{reading}");
  assert_eq!((reading["id"].as_i64(), reading["state"].as_str()), (Some(43), Some("running")));
  assert_eq!(read(first.port, "/quiet").await.1, json!({ "quiet": true, "work": [] }));

  // A send under the turn's key attaches to it and waits; the server's stop answers it so its caller asks
  // again, and the server ends.
  let port = first.port;
  let waiting = tokio::spawn(async move { send(port).await });

  tokio::time::sleep(Duration::from_millis(500)).await;
  assert!(!waiting.is_finished(), "the send waits on the turn standing under its key");

  let ended = tokio::task::spawn_blocking(move || {
    let ended = first.stop(Duration::from_secs(30));

    (first, ended)
  });
  let (status, said) = tokio::time::timeout(Duration::from_secs(30), waiting).await.unwrap().unwrap();

  assert_eq!(status, 503, "{said}");
  assert_eq!((said["code"].as_str(), said["key"].as_str()), (Some("stopping"), Some("round-7")));

  let (first, ended) = ended.await.unwrap();

  assert!(ended.is_some_and(|ended| ended.success()), "the server drains and ends: {ended:?}");
  drop(first);

  // The program went on with the turn while no server stood: it is still working on it.
  let second = server(home.path(), &scratch);
  let port = second.port;
  let (status, reading) = read(port, &format!("/sessions/{SESSION}/turns/43")).await;

  assert_eq!((status, reading["state"].as_str()), (200, Some("running")), "{reading}");

  let waiting = tokio::spawn(async move { send(port).await });

  tokio::time::sleep(Duration::from_millis(500)).await;
  assert!(!waiting.is_finished(), "the send asked again attaches to the picked-up turn");

  append(
    &place,
    json!({ "kind": "answer", "id": "w", "text": "The disk is 81% full.",
            "model": "[\"deepseek\",\"deepseek-flash\"]",
            "usage": { "inputTokens": 10, "outputTokens": 5, "cacheReadTokens": 0 } }),
  );
  append(&place, json!({ "kind": "end", "id": "w" }));

  let (status, answered) = tokio::time::timeout(Duration::from_secs(30), waiting).await.unwrap().unwrap();

  assert_eq!(status, 201, "{answered}");
  assert_eq!(answered["id"], 43);
  assert_eq!(answered["answer"], "The disk is 81% full.");
  assert_eq!(answered["prompt"], "What is the disk?");

  // A repeat after the end is answered with it at once, and nothing new goes into the session.
  let (status, again) = send(port).await;

  assert_eq!(
    (status, again["id"].as_i64(), again["answer"].as_str()),
    (201, Some(43), Some("The disk is 81% full."))
  );

  // The end was published once, under the turn's own id and its caller's key.
  let (_, page) = read(port, "/events?kinds=turn.ended&limit=100").await;
  let ends: Vec<&Value> = page["items"]
    .as_array()
    .unwrap()
    .iter()
    .filter(|event| event["data"]["session"] == SESSION)
    .collect();

  assert_eq!(ends.len(), 1, "{page}");
  assert_eq!(
    (ends[0]["data"]["turn"].as_i64(), ends[0]["data"]["key"].as_str()),
    (Some(43), Some("round-7"))
  );

  let _ = program.kill();
  let _ = program.wait();
}
