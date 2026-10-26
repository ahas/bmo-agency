//! Every research result leaves its document (docs/research-runs.md, 「One run, step by step」): filed in
//! bmo-artifact and shared, and the room told the answer in brief with the one link, never the grounds.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agency_server::research::{RESEARCH_ACTOR, brief_of, document_of, file_result, linked_instruction};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{StatusCode, Uri};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use tokio::net::TcpListener;

const BRIEF: &str = "ANSWERED: yes :: the base rate\nTITLE: The base rate stays at 3.00%\nTOPIC: the base rate\n\
KEYWORDS: base rate\n\nSUMMARY:\n1. The base rate is 3.00% a year as of 2026-10-26 [1].\n\nSECTION: Grounds\n\
The bank's own history lists no change after 2026-08-27 [1].\nSOURCE [1]: https://example.org/rates :: the history\n\n\
UNCONFIRMED:\n1. Nothing.";

#[test]
fn a_brief_result_gives_its_summary_and_a_deep_one_its_verdict() {
  assert_eq!(
    brief_of(BRIEF).as_deref(),
    Some("1. The base rate is 3.00% a year as of 2026-10-26 [1].")
  );

  let deep = "TITLE: A pick\n\nVERDICT:\nBuy the second one.\n\nSUMMARY:\n1. a\n2. b\n3. c";

  assert_eq!(brief_of(deep).as_deref(), Some("Buy the second one."));
  assert_eq!(brief_of("TITLE: Only a title").as_deref(), Some("Only a title"));
}

#[test]
fn a_result_is_filed_whole_titled_by_its_head_and_labelled_with_its_run() {
  let document = document_of(7, "search:basic", " the rate ", Some(5), BRIEF);

  assert_eq!(document["kind"], "record");
  assert_eq!(document["title"], "The base rate stays at 3.00%");
  assert_eq!(document["body"], BRIEF);
  assert_eq!(document["scope"], "5");
  assert_eq!(document["actorName"], RESEARCH_ACTOR);
  assert_eq!(document["labels"], json!({ "research": 7, "key": "search:basic", "query": "the rate" }));

  let learning = document_of(8, "search:thorough", "teach me", None, "KIND: learning\nTITLE: A book");

  assert_eq!(learning["kind"], "learning");
  assert_eq!(learning["scope"], Value::Null);
  assert_eq!(document_of(9, "browse", "the task", None, "no head")["title"], "the task");
}

#[test]
fn the_room_is_told_the_answer_in_brief_and_one_link_and_no_grounds() {
  let told =
    linked_instruction(7, "search:basic", "the rate", Some(5), "1. 3.00%", "https://share.example/t");
  let order = told["said"]["order"].as_str().unwrap();

  assert_eq!(order.matches("https://").count(), 1, "{order}");
  assert!(order.contains("1. 3.00%") && order.contains("https://share.example/t"));
  assert!(
    order.contains("one or two short lines") && order.contains("no grounds, sources or document text")
  );
  assert!(order.contains("never how"));
  assert_eq!(told["room"], 5);
  assert_eq!(told["kind"], "research-ended:7");
}

type Calls = Arc<Mutex<Vec<(String, Value)>>>;

async fn artifact(State(calls): State<Calls>, uri: Uri, body: Bytes) -> impl IntoResponse {
  calls
    .lock()
    .unwrap()
    .push((uri.path().to_owned(), serde_json::from_slice(&body).unwrap_or(Value::Null)));

  match uri.path() {
    "/documents" => (StatusCode::CREATED, axum::Json(json!({ "id": 41, "kind": "record" }))),
    "/shares" => (
      StatusCode::CREATED,
      axum::Json(json!({ "token": "t", "documentId": 41, "url": "https://share.example/t" })),
    ),
    _ => (StatusCode::NOT_FOUND, axum::Json(json!({ "code": "not-found", "error": "not found" }))),
  }
}

#[tokio::test]
async fn a_result_is_filed_and_shared_and_a_deployment_naming_no_artifact_service_files_nothing() {
  let calls: Calls = Arc::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(artifact).with_state(calls.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let vars: BTreeMap<String, String> = [
    ("BMO_ARTIFACT_URL".to_owned(), address),
    ("BMO_ARTIFACT_TOKEN".to_owned(), "artifact-token".to_owned()),
  ]
  .into();
  let document = document_of(7, "search:basic", "the rate", Some(5), BRIEF);

  assert_eq!(file_result(&vars, &document).await.unwrap().as_deref(), Some("https://share.example/t"));

  let made = calls.lock().unwrap().clone();

  assert_eq!(made.len(), 2);
  assert_eq!((made[0].0.as_str(), &made[0].1), ("/documents", &document));
  assert_eq!((made[1].0.as_str(), &made[1].1), ("/shares", &json!({ "documentId": 41 })));
  assert_eq!(file_result(&BTreeMap::new(), &document).await.unwrap(), None);
}
