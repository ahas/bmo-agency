//! A running job's lease kept on the harness's behalf (docs/api.md, 「assignments and jobs」), whatever became of
//! the harness that took it, a restart of agency under the job among them: a lease that stands is renewed, one
//! run out is taken again on the same item and the job keeps the new id, an item another holder took or one no
//! longer open is left; a running job's checkout reads as held by it and its item waits on no free worker.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agency_server::job_leases::{Kept, pass};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::rows::repository;
use crate::support::scratch::Scratch;

type Calls = Arc<Mutex<Vec<(String, String, Value)>>>;

// The job service: lease 7 stands; leases 8 and 9 have run out; item 21 is open with an assignee, item 22 is
// another holder's; a new lease on item 21 is 99.
async fn jobs(State(calls): State<Calls>, method: Method, uri: Uri, body: Bytes) -> impl IntoResponse {
  let body = serde_json::from_slice(&body).unwrap_or(Value::Null);

  calls.lock().unwrap().push((method.to_string(), uri.path().to_owned(), body));

  match (method.as_str(), uri.path()) {
    ("PATCH", "/leases/7") => (StatusCode::OK, axum::Json(json!({ "id": 7 }))),
    ("PATCH", _) => (StatusCode::GONE, axum::Json(json!({ "code": "gone" }))),
    ("GET", "/items/21") => (
      StatusCode::OK,
      axum::Json(json!({ "id": 21, "state": "open", "queueName": "work", "assigneeName": "worker-a" })),
    ),
    ("GET", "/items/22") => {
      (StatusCode::OK, axum::Json(json!({ "id": 22, "state": "leased", "queueName": "work" })))
    }
    ("POST", "/leases") => (StatusCode::CREATED, axum::Json(json!({ "id": 99 }))),
    _ => (StatusCode::NOT_FOUND, axum::Json(json!({}))),
  }
}

async fn seeded(scratch: &Scratch) {
  let now = agency_server::now_ms();

  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
                             "started_at", "prompt")
         VALUES (1, 20, 7, 'harness', 1, 'worker-a', 'running', $1, 'work'),
                (2, 21, 8, 'harness', 2, 'worker-a', 'running', $1, 'work'),
                (3, 22, 9, 'harness', 3, 'worker-b', 'starting', $1, 'work'),
                (4, 23, 10, 'harness', 4, 'worker-b', 'ended', $1, 'work')"#,
      vec![now.into()],
    )
    .await;
}

#[tokio::test]
async fn a_running_job_s_lease_is_renewed_or_taken_again_and_one_another_holds_is_left() {
  let scratch = Scratch::new().await;
  let calls: Calls = Arc::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(jobs).with_state(calls.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  seeded(&scratch).await;

  let vars: BTreeMap<String, String> = [
    ("BMO_JOB_URL".to_owned(), address),
    ("BMO_JOB_TOKEN".to_owned(), "job-token".to_owned()),
  ]
  .into();
  let kept = pass(scratch.db(), &vars).await.unwrap();

  assert_eq!(kept[0], (1, Kept::Renewed));
  assert_eq!(kept[1], (2, Kept::Taken(99)));
  assert!(matches!(&kept[2], (3, Kept::Left(why)) if why.contains("leased")), "{kept:?}");
  assert_eq!(kept.len(), 3, "an ended job keeps no lease: {kept:?}");

  let lease = store::one(scratch.db(), r#"SELECT "lease_id" FROM "jobs" WHERE "id" = 2"#, vec![])
    .await
    .unwrap()
    .and_then(|row| row.try_get::<i64>("", "lease_id").ok());

  assert_eq!(lease, Some(99), "the job keeps the lease taken again, which its end gives back");

  let asked = calls.lock().unwrap().clone();
  let renewal = asked.iter().find(|(method, path, _)| method == "PATCH" && path == "/leases/7").unwrap();
  let taken = asked.iter().find(|(method, path, _)| method == "POST" && path == "/leases").unwrap();

  assert_eq!(renewal.2["ttlMs"], policy::job_leases::JOB_LEASES.ttl_ms);
  assert_eq!(
    taken.2,
    json!({ "queueName": "work", "itemId": 21, "holderName": "harness", "assigneeName": "worker-a",
            "ttlMs": policy::job_leases::JOB_LEASES.ttl_ms })
  );

  // A deployment that names no job service keeps nothing.
  assert_eq!(pass(scratch.db(), &BTreeMap::new()).await.unwrap(), []);
}

#[tokio::test]
async fn an_ended_job_s_lease_kept_for_its_close_is_renewed_until_the_close_ends_it_and_never_taken_again()
{
  let scratch = Scratch::new().await;
  let calls: Calls = Arc::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(jobs).with_state(calls.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
                             "started_at", "prompt", "lease_kept")
         VALUES (5, 20, 7, 'harness', 5, 'worker-a', 'ended', 1, 'work', true),
                (6, 21, 8, 'harness', 6, 'worker-a', 'ended', 1, 'work', true),
                (7, 22, 9, 'harness', 7, 'worker-b', 'stopped', 1, 'work', false)"#,
      vec![],
    )
    .await;

  let vars: BTreeMap<String, String> = [
    ("BMO_JOB_URL".to_owned(), address),
    ("BMO_JOB_TOKEN".to_owned(), "job-token".to_owned()),
  ]
  .into();

  // Lease 7 stands for its item's close and is renewed; lease 8 has ended with its item's close, and its item,
  // open or not, is not leased again.
  assert_eq!(pass(scratch.db(), &vars).await.unwrap(), [(5, Kept::Renewed), (6, Kept::Closed)]);
  assert!(
    !calls.lock().unwrap().iter().any(|(method, ..)| method == "POST"),
    "no lease is taken again"
  );
  assert_eq!(pass(scratch.db(), &vars).await.unwrap(), [(5, Kept::Renewed)]);
}

#[tokio::test]
async fn a_running_job_s_item_waits_on_no_free_worker() {
  let scratch = Scratch::new().await;

  seeded(&scratch).await;

  let open = |id: i64| agency_server::watches::queue_idle::Open {
    id,
    title: format!("item {id}"),
    assignee: None,
    room: None,
  };
  let standing = agency_server::watches::queue_idle::held(scratch.db(), vec![open(21), open(23), open(30)])
    .await
    .unwrap();
  let ids: Vec<i64> = standing.open.iter().map(|item| item.id).collect();

  assert_eq!(ids, [23, 30], "item 21's job is still at work: {ids:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_worker_with_a_running_job_reads_as_working_on_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");

  std::fs::create_dir_all(&data).unwrap();
  for worker in ["worker-a", "worker-c"] {
    repository(scratch.db(), worker, &format!("/checkouts/{worker}"), &[]).await;
  }
  seeded(&scratch).await;

  let server = crate::support::standing::stand_server(home.path(), &data, &scratch.address(), "secret");
  let read = |name: &'static str| async move {
    reqwest::Client::new()
      .get(format!("http://127.0.0.1:{}/repositories/{name}", server.port))
      .bearer_auth("secret")
      .send()
      .await
      .unwrap()
      .json::<Value>()
      .await
      .unwrap()["checkouts"][0]["job"]
      .clone()
  };
  let working = read("worker-a").await;
  let idle = read("worker-c").await;

  assert_eq!(
    (working["jobId"].as_i64(), working["itemId"].as_i64()),
    (Some(2), Some(21)),
    "{working}"
  );
  assert_eq!(idle, Value::Null);
}
