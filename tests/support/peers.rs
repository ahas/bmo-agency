//! A stand-in for the job service and the receiver in one, as the queue's notices reach them: items by their
//! number, the queues, the leases that stand, the events past a cursor and the window's oldest, and every call
//! heard; the receiver answers each instruction with an id of its own.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use tokio::net::TcpListener;

/// One call the stand-in heard.
#[derive(Clone, Debug)]
pub struct Heard {
  pub method: Method,
  pub path: String,
  pub query: String,
  pub body: Value,
}

/// What the stand-in holds; a test sets it and reads the calls back.
#[derive(Default)]
pub struct Held {
  pub items: BTreeMap<i64, Value>,
  pub queues: Vec<Value>,
  /// The leases that stand and are renewed; any other is gone.
  pub leases: BTreeSet<i64>,
  /// The lease a take answers, where one is taken; a conflict where none.
  pub next_lease: Option<i64>,
  pub events: Vec<Value>,
  /// The oldest event the window holds; a cursor below it is answered 410.
  pub window_from: i64,
  pub calls: Vec<Heard>,
  pub instructions: i64,
}

pub type Peers = Arc<Mutex<Held>>;

impl Held {
  /// The bodies of the instructions queued, in order.
  pub fn instructed(&self) -> Vec<Value> {
    self.posted("/instructions")
  }

  /// The bodies of the owner notices queued, in order.
  pub fn noticed(&self) -> Vec<Value> {
    self.posted("/owner/notices")
  }

  fn posted(&self, path: &str) -> Vec<Value> {
    self
      .calls
      .iter()
      .filter(|call| call.method == Method::POST && call.path == path)
      .map(|call| call.body.clone())
      .collect()
  }

  /// The instructions withdrawn, by their id.
  pub fn withdrawn(&self) -> Vec<String> {
    self
      .calls
      .iter()
      .filter(|call| call.method == Method::DELETE && call.path.starts_with("/instructions/"))
      .map(|call| call.path.trim_start_matches("/instructions/").to_owned())
      .collect()
  }
}

fn query_of(uri: &Uri) -> BTreeMap<String, String> {
  uri
    .query()
    .unwrap_or_default()
    .split('&')
    .filter_map(|pair| pair.split_once('='))
    .map(|(key, value)| (key.to_owned(), value.replace("%2C", ",").replace("%2c", ",")))
    .collect()
}

async fn answer(
  State(peers): State<Peers>,
  method: Method,
  uri: Uri,
  body: Bytes,
) -> axum::response::Response {
  let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
  let mut held = peers.lock().unwrap();
  let query = query_of(&uri);

  held.calls.push(Heard {
    method: method.clone(),
    path: uri.path().to_owned(),
    query: uri.query().unwrap_or_default().to_owned(),
    body: body.clone(),
  });

  let path = uri.path().to_owned();

  match (method.as_str(), path.as_str()) {
    ("POST", "/instructions") => {
      held.instructions += 1;
      (StatusCode::CREATED, axum::Json(json!({ "id": held.instructions }))).into_response()
    }
    ("DELETE", path) if path.starts_with("/instructions/") => StatusCode::NO_CONTENT.into_response(),
    ("POST", "/owner/notices") => (StatusCode::CREATED, axum::Json(json!({ "id": 1 }))).into_response(),
    ("GET", "/queues") => {
      axum::Json(json!({ "items": held.queues.clone(), "more": false })).into_response()
    }
    ("GET", "/items") => {
      let states: Vec<&str> =
        query.get("state").map(|states| states.split(',').collect()).unwrap_or_default();
      let items: Vec<Value> = held
        .items
        .values()
        .filter(|item| states.is_empty() || states.contains(&item["state"].as_str().unwrap_or_default()))
        .cloned()
        .collect();

      axum::Json(json!({ "items": items, "more": false })).into_response()
    }
    ("GET", path) if path.starts_with("/items/") => {
      let id: i64 = path.trim_start_matches("/items/").parse().unwrap_or_default();

      match held.items.get(&id) {
        Some(item) => axum::Json(item.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, axum::Json(json!({ "code": "not_found" }))).into_response(),
      }
    }
    ("PATCH", path) if path.starts_with("/leases/") => {
      let id: i64 = path.trim_start_matches("/leases/").parse().unwrap_or_default();

      if held.leases.contains(&id) {
        axum::Json(json!({ "id": id })).into_response()
      } else {
        (StatusCode::GONE, axum::Json(json!({ "code": "gone" }))).into_response()
      }
    }
    ("POST", "/leases") => match held.next_lease {
      Some(lease) => (StatusCode::CREATED, axum::Json(json!({ "id": lease }))).into_response(),
      None => (StatusCode::CONFLICT, axum::Json(json!({ "code": "conflict" }))).into_response(),
    },
    ("GET", "/events") => {
      let after: i64 = query.get("after").and_then(|after| after.parse().ok()).unwrap_or(0);

      if after > 0 && after < held.window_from {
        return (StatusCode::GONE, axum::Json(json!({ "code": "gone" }))).into_response();
      }

      let kinds: Vec<&str> = query.get("kinds").map(|kinds| kinds.split(',').collect()).unwrap_or_default();
      let events: Vec<Value> = held
        .events
        .iter()
        .filter(|event| event["id"].as_i64().unwrap_or_default() > after)
        .filter(|event| event["id"].as_i64().unwrap_or_default() >= held.window_from)
        .filter(|event| kinds.is_empty() || kinds.contains(&event["kind"].as_str().unwrap_or_default()))
        .cloned()
        .collect();

      axum::Json(json!({ "items": events, "more": false })).into_response()
    }
    _ => StatusCode::NOT_FOUND.into_response(),
  }
}

/// The stand-in served, with the settings that name it as both the job service and the receiver.
pub async fn stand() -> (Peers, BTreeMap<String, String>) {
  let peers: Peers = Arc::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(answer).with_state(peers.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let vars = [
    ("BMO_JOB_URL".to_owned(), address.clone()),
    ("BMO_JOB_TOKEN".to_owned(), "job-token".to_owned()),
    ("BMO_RECEIVER_URL".to_owned(), address),
    ("BMO_RECEIVER_TOKEN".to_owned(), "receiver-token".to_owned()),
  ]
  .into();

  (peers, vars)
}

/// An item as the job service answers it.
pub fn item(id: i64, state: &str, extra: Value) -> Value {
  let mut item = json!({ "id": id, "title": format!("item {id}"), "state": state, "queueName": "todo" });

  if let (Some(item), Some(extra)) = (item.as_object_mut(), extra.as_object()) {
    item.extend(extra.clone());
  }

  item
}
