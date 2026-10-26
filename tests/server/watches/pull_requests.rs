//! Pull request reviews (docs/pull-request-reviews.md): a delivery checked against the webhook secret and read,
//! a worker's remote read as its repository, the fallback's search read, a review item filed once for the
//! worker that holds the repository, and a merge opening the item that waited on it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agency_server::watches::pull_requests::{
  Delivered, PullRequest, act, read_delivery, repository_of, review_item, searched, signed,
};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::response::IntoResponse;
use policy::pull_requests::PULL_REQUESTS;
use serde_json::{Value, json};
use tokio::net::TcpListener;

#[test]
fn a_delivery_is_github_s_where_its_signature_is_the_body_s_under_the_secret() {
  // GitHub's own example of a signed delivery.
  let secret = "It's a Secret to Everybody";
  let signature = "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";

  assert!(signed(secret, b"Hello, World!", signature));
  assert!(!signed(secret, b"Hello, World?", signature));
  assert!(!signed("another secret", b"Hello, World!", signature));
  assert!(!signed(secret, b"Hello, World!", "sha1=757107"));
}

fn delivery(action: &str, merged: bool) -> Value {
  json!({
    "action": action,
    "number": 7,
    "repository": { "full_name": "org/base-ui" },
    "pull_request": {
      "html_url": "https://github.com/org/base-ui/pull/7",
      "number": 7,
      "title": "Document the theme tokens",
      "merged": merged,
      "head": { "sha": "abc123" },
    },
  })
}

fn request() -> PullRequest {
  PullRequest {
    url: "https://github.com/org/base-ui/pull/7".into(),
    repository: "org/base-ui".into(),
    number: 7,
    title: "Document the theme tokens".into(),
    head: Some("abc123".into()),
  }
}

#[test]
fn a_pull_request_event_reads_as_what_it_asks_and_every_other_event_as_nothing() {
  assert_eq!(
    read_delivery("pull_request", &delivery("opened", false)),
    Some(Delivered::Open(request()))
  );
  assert_eq!(
    read_delivery("pull_request", &delivery("synchronize", false)),
    Some(Delivered::Pushed(request()))
  );
  assert_eq!(
    read_delivery("pull_request", &delivery("closed", true)),
    Some(Delivered::Merged(request()))
  );
  assert_eq!(
    read_delivery("pull_request", &delivery("closed", false)),
    Some(Delivered::Closed(request()))
  );
  assert_eq!(read_delivery("pull_request", &delivery("labeled", false)), None);
  assert_eq!(read_delivery("push", &delivery("opened", false)), None);
}

#[test]
fn a_worker_s_remote_reads_as_the_github_repository_it_names() {
  // The SSH user is written apart, so no remote reads as an address in a tracked file.
  let user = "git";

  for remote in [
    format!("{user}@github.com:org/base-ui.git"),
    "https://github.com/org/base-ui".to_owned(),
    "https://github.com/org/base-ui.git\n".to_owned(),
    format!("ssh://{user}@github.com/org/base-ui.git"),
  ] {
    assert_eq!(repository_of(&remote).as_deref(), Some("org/base-ui"), "{remote}");
  }

  assert_eq!(repository_of(&format!("{user}@gitlab.com:org/base-ui.git")), None);
  assert_eq!(repository_of("https://github.com/org"), None);
}

#[test]
fn the_fallback_reads_every_open_pull_request_of_the_search_and_the_item_names_its_review() {
  let page = json!({ "items": [{
    "number": 7,
    "title": "Document the theme tokens",
    "repository_url": "https://api.github.com/repos/org/base-ui",
    "pull_request": { "html_url": "https://github.com/org/base-ui/pull/7" },
  }]});

  assert_eq!(
    searched(&page),
    vec![PullRequest {
      head: None,
      ..request()
    }]
  );

  let item = review_item(&request(), "base-ui");

  assert_eq!(item["assigneeName"], "base-ui");
  assert_eq!(item["queueName"], PULL_REQUESTS.queue.as_str());
  assert_eq!(
    item["labels"],
    json!({ "pr": "https://github.com/org/base-ui/pull/7", "lane": PULL_REQUESTS.lane })
  );
  assert!(item["title"].as_str().unwrap().contains("#7 of org/base-ui"));
  assert!(item["brief"].as_str().unwrap().contains("base-library rule"));
}

type Calls = Arc<Mutex<Vec<(String, String, String, Value)>>>;

// A job service holding the items the test names under one pull request's label, every call kept.
async fn jobs(
  State((calls, standing)): State<(Calls, Value)>,
  method: Method,
  uri: Uri,
  body: Bytes,
) -> impl IntoResponse {
  calls.lock().unwrap().push((
    method.to_string(),
    uri.path().to_owned(),
    uri.query().unwrap_or_default().to_owned(),
    serde_json::from_slice(&body).unwrap_or(Value::Null),
  ));

  match (method.as_str(), uri.path()) {
    ("GET", "/items") => (StatusCode::OK, axum::Json(json!({ "items": standing, "more": false }))),
    ("POST", "/items") => (StatusCode::CREATED, axum::Json(json!({ "id": 99 }))),
    _ => (StatusCode::OK, axum::Json(json!({}))),
  }
}

async fn stand(standing: Value) -> (Calls, BTreeMap<String, String>) {
  let calls: Calls = Arc::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(jobs).with_state((calls.clone(), standing));

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let vars = [
    ("BMO_JOB_URL".to_owned(), address),
    ("BMO_JOB_TOKEN".to_owned(), "job-token".to_owned()),
  ]
  .into();

  (calls, vars)
}

#[tokio::test]
async fn an_open_pull_request_is_filed_once_for_the_worker_that_holds_its_repository() {
  let held = BTreeMap::from([("org/base-ui".to_owned(), "base-ui".to_owned())]);
  let (calls, vars) = stand(json!([])).await;

  act(&vars, &held, &Delivered::Open(request())).await.unwrap();

  let made = calls.lock().unwrap().clone();

  assert!(made[0].2.contains("label=pr%3Dhttps"), "{made:?}");
  assert_eq!((made[1].0.as_str(), made[1].1.as_str()), ("POST", "/items"));
  assert_eq!(made[1].3["assigneeName"], "base-ui");

  // Filed already: nothing more.
  let (again, vars) = stand(
    json!([{ "id": 5, "state": "draft", "labels": { "pr": request().url, "lane": PULL_REQUESTS.lane } }]),
  )
  .await;

  act(&vars, &held, &Delivered::Open(request())).await.unwrap();
  assert!(again.lock().unwrap().iter().all(|(method, ..)| method == "GET"));

  // A repository no worker holds is let be.
  let (none, vars) = stand(json!([])).await;

  act(&vars, &BTreeMap::new(), &Delivered::Open(request())).await.unwrap();
  assert!(none.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_merge_opens_the_held_item_that_waited_on_it_and_tells_the_review_item() {
  let standing = json!([
    { "id": 5, "state": "open", "labels": { "pr": request().url, "lane": PULL_REQUESTS.lane } },
    { "id": 6, "state": "held", "labels": { "pr": request().url } },
    { "id": 8, "state": "open", "labels": { "pr": request().url } },
  ]);
  let (calls, vars) = stand(standing).await;

  act(&vars, &BTreeMap::new(), &Delivered::Merged(request())).await.unwrap();

  let made: Vec<(String, String, Value)> = calls
    .lock()
    .unwrap()
    .iter()
    .skip(1)
    .map(|(method, path, _, body)| (method.clone(), path.clone(), body.clone()))
    .collect();

  assert_eq!(made[0].1, "/items/5/notes");
  assert!(made[0].2["text"].as_str().unwrap().contains("merged without this review"));
  assert_eq!(made[1].1, "/items/6/notes");
  assert_eq!((made[2].0.as_str(), made[2].1.as_str()), ("PATCH", "/items/6"));
  assert_eq!(made[2].2, json!({ "state": "open" }));
  assert_eq!(made.len(), 3, "an item that was not held is left as it stands: {made:?}");
}
