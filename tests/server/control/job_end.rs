//! A job's end reaching the room and returning its lease (docs/api.md, 「assignments and jobs」): the
//! order the room's chat agent is handed, and the two calls the job runner makes as a job ends.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agency_server::job_end::{delivered_with, instruction_body, order};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use tokio::net::TcpListener;

fn member(report: Option<&str>, outcome: Option<&str>) -> Value {
  json!({
    "id": 906369,
    "itemId": 2224,
    "leaseId": 64,
    "workerName": "bmo-agency",
    "runId": 5938,
    "runName": "job-906369",
    "roomId": 90000001,
    "state": "ended",
    "outcome": outcome,
    "report": report,
  })
}

#[test]
fn the_order_carries_the_item_the_worker_and_the_report_whole() {
  let said = order(&member(Some("The change landed in commit abc123.\n\nWhat is left: a deploy."), None));

  assert!(said.contains("item #2224"));
  assert!(said.contains("worker bmo-agency"));
  assert!(said.contains("The change landed in commit abc123."));
  assert!(said.contains("What is left: a deploy."));
  // The room is told the job's end in a few lines; the report stays whole on the item.
  assert!(said.contains("Relay its end in a few lines in this room"));
  assert!(said.contains("never paste it into the room"));
}

#[test]
fn a_job_that_left_no_report_says_so_and_a_stop_carries_its_outcome() {
  let said = order(&member(None, Some("failed")));

  assert!(said.contains("ended, its outcome failed"));
  assert!(!said.contains("ended ended"));
  assert!(said.contains("The run left no report; its end is all there is to tell."));
}

#[test]
fn the_instruction_names_the_item_as_queue_news_and_the_room_the_job_answers() {
  let body = instruction_body(&member(Some("done"), None));

  assert_eq!(body["room"], 90000001);
  assert_eq!(body["sort"], "notice");
  assert_eq!(body["kind"], "job-ended:2224");
  assert_eq!(body["events"], json!(["job-ended:#2224"]));
  assert_eq!(body["run"], 5938);
  assert_eq!(body["summary"], "The job of item #2224 ended");
  assert!(body["said"]["order"].as_str().unwrap().contains("done"));
}

#[derive(Clone, Debug)]
struct Call {
  method: Method,
  path: String,
  authorization: Option<String>,
  body: Value,
}

type Calls = Arc<Mutex<Vec<Call>>>;

async fn stand_in(
  State(calls): State<Calls>,
  method: Method,
  uri: Uri,
  headers: HeaderMap,
  body: Bytes,
) -> impl IntoResponse {
  calls.lock().unwrap().push(Call {
    method,
    path: uri.path().to_owned(),
    authorization: headers.get("authorization").and_then(|value| value.to_str().ok()).map(str::to_owned),
    body: serde_json::from_slice(&body).unwrap_or(Value::Null),
  });

  StatusCode::NO_CONTENT
}

#[tokio::test]
async fn a_job_s_end_gives_its_lease_back_and_queues_its_report_in_the_room() {
  let calls: Calls = Arc::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(stand_in).with_state(calls.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let vars: BTreeMap<String, String> = [
    ("BMO_JOB_URL".to_owned(), address.clone()),
    ("BMO_JOB_TOKEN".to_owned(), "job-token".to_owned()),
    ("BMO_RECEIVER_URL".to_owned(), address.clone()),
    ("BMO_RECEIVER_TOKEN".to_owned(), "receiver-token".to_owned()),
  ]
  .into();

  delivered_with(&vars, &member(Some("The work is done."), None)).await;

  let taken = calls.lock().unwrap().clone();
  let lease = taken
    .iter()
    .find(|call| call.method == Method::DELETE)
    .expect("the lease is given back");
  let told = taken.iter().find(|call| call.path == "/instructions").expect("the room is told");

  assert_eq!(lease.path, "/leases/64");
  assert_eq!(lease.authorization.as_deref(), Some("Bearer job-token"));
  assert_eq!(told.method, Method::POST);
  assert_eq!(told.authorization.as_deref(), Some("Bearer receiver-token"));
  assert_eq!(told.body["room"], 90000001);
  assert_eq!(told.body["events"], json!(["job-ended:#2224"]));
  assert!(told.body["said"]["order"].as_str().unwrap().contains("The work is done."));
}
