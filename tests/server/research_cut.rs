//! A research run cut under its pane or by agency's own restart (docs/research-runs.md, 「One run, step by
//! step」): closed in the ledger as cut, the room handed what it had written or one line that the work was
//! cut off, and a server's start sweeping what the process before it left under way.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agency_server::research::{CUT, IN_FLIGHT_KEY, InFlight, cut_instruction, sweep_cut, written_so_far};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{StatusCode, Uri};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::scratch::Scratch;

// A session's record whose last answer is `text`.
fn record_with(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
  let path = dir.join("session.jsonl");
  let lines = [
    json!({ "type": "user", "message": { "role": "user", "content": "the task" } }),
    json!({
      "type": "assistant",
      "message": { "id": "m1", "role": "assistant", "model": "a-model", "content": [{ "type": "text", "text": text }] }
    }),
  ];

  std::fs::write(&path, lines.iter().map(|line| format!("{line}\n")).collect::<String>()).unwrap();
  path
}

#[test]
fn a_cut_run_hands_back_what_it_wrote_or_says_in_one_line_that_the_work_was_cut_off() {
  let dir = tempfile::tempdir().unwrap();
  let record = record_with(dir.path(), "ANSWERED: no :: half way\nSUMMARY:\n1. a finding");

  assert_eq!(
    written_so_far(&record).as_deref(),
    Some("ANSWERED: no :: half way\nSUMMARY:\n1. a finding")
  );
  assert_eq!(written_so_far(&dir.path().join("none.jsonl")), None);

  let handed = cut_instruction(7, "search:basic", Some("the rate"), Some(5), Some("a finding"));
  let order = handed["said"]["order"].as_str().unwrap();

  assert!(order.contains("search:basic") && order.contains("\"the rate\"") && order.contains("a finding"));
  assert!(order.contains("unfinished"));
  assert_eq!(handed["room"], 5);
  assert_eq!(handed["kind"], "research-ended:7");

  let nothing = cut_instruction(8, "browse", None, None, None);
  let said = nothing["said"]["order"].as_str().unwrap();

  assert!(said.contains("in one line that the work was cut off"), "{said}");
  assert_eq!(nothing["room"], Value::Null);
}

type Calls = Arc<Mutex<Vec<(String, Value)>>>;

async fn stand_in(State(calls): State<Calls>, uri: Uri, body: Bytes) -> impl IntoResponse {
  calls
    .lock()
    .unwrap()
    .push((uri.path().to_owned(), serde_json::from_slice(&body).unwrap_or(Value::Null)));

  (StatusCode::CREATED, axum::Json(json!({ "id": 1 })))
}

#[tokio::test]
async fn a_server_s_start_closes_the_runs_the_last_process_left_under_way_as_cut_and_tells_their_rooms() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let dir = tempfile::tempdir().unwrap();
  let record = record_with(dir.path(), "what it had found");
  let calls: Calls = Arc::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(stand_in).with_state(calls.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let vars: BTreeMap<String, String> = [
    ("BMO_RECEIVER_URL".to_owned(), address),
    ("BMO_RECEIVER_TOKEN".to_owned(), "receiver-token".to_owned()),
  ]
  .into();

  // A run kept under way with its record, one the ledger alone holds open, one already closed, and one
  // started after this process did.
  scratch
    .run(
      r#"INSERT INTO "runs" ("id", "agent", "chat_id", "unit", "depth", "started_at", "finished_at", "outcome")
         VALUES (1, 'search', 5, 'research-1', 0, 100, NULL, NULL),
                (2, 'search', 6, 'research-2', 1, 100, NULL, NULL),
                (3, 'search', 6, 'research-3', 0, 100, 150, 'answered'),
                (4, 'browse', 6, 'research-4', 0, 900, NULL, NULL)"#,
      vec![],
    )
    .await;

  let kept = InFlight {
    key: "search:basic".into(),
    query: "the rate".into(),
    room: Some(5),
    model: "a-model".into(),
    record: Some(record.to_string_lossy().into_owned()),
  };

  agency_server::queue::settings::set(
    db,
    &format!("{IN_FLIGHT_KEY}1"),
    &serde_json::to_string(&kept).unwrap(),
  )
  .await
  .unwrap();

  assert_eq!(sweep_cut(db, &vars, 500).await.unwrap(), 2);

  assert_eq!(scratch_outcome(db, 1).await, Some(CUT.to_owned()));
  assert_eq!(scratch_outcome(db, 2).await, Some(CUT.to_owned()));
  assert_eq!(scratch_outcome(db, 3).await, Some("answered".to_owned()));
  assert_eq!(scratch_outcome(db, 4).await, None, "a run of this process is not swept");
  assert!(
    agency_server::queue::settings::get(db, &format!("{IN_FLIGHT_KEY}1"))
      .await
      .unwrap()
      .is_none(),
    "the kept run is let go"
  );

  let told = calls.lock().unwrap().clone();

  assert_eq!(told.len(), 2, "{told:?}");
  assert!(told.iter().all(|(path, _)| path == "/instructions"));

  let first = told[0].1["said"]["order"].as_str().unwrap();
  let second = told[1].1["said"]["order"].as_str().unwrap();

  assert!(first.contains("what it had found") && first.contains("\"the rate\""), "{first}");
  assert!(second.contains("search:thorough") && second.contains("cut off"), "{second}");
  assert_eq!(told[1].1["room"], 6);

  // A second start finds nothing left.
  assert_eq!(sweep_cut(db, &vars, 500).await.unwrap(), 0);
}

async fn scratch_outcome(db: &sea_orm::DatabaseConnection, id: i64) -> Option<String> {
  store::one(db, r#"SELECT "outcome" FROM "runs" WHERE "id" = $1"#, vec![id.into()])
    .await
    .unwrap()
    .and_then(|row| row.try_get::<Option<String>>("", "outcome").ok().flatten())
}
