//! Who may read or change agent management (docs/api.md, 「The control port」): a member's five kinds of route
//! asked with the console's permission for their `agents` section, read and write apart; the administrator and
//! a caller carrying no key unchecked; and a permission that could not be read refused, never guessed.

use std::sync::{Arc, Mutex};

use agency_server::control::{Front, router};
use agency_server::places::Ground;
use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::grounds::seeded;
use crate::support::scratch::Scratch;

const TOKEN: &str = "secret";

/// The five kinds of route agent management is read and changed through.
const KINDS: [&str; 5] = ["/supervisors", "/roles", "/repositories", "/models", "/model-catalogue"];

#[derive(Clone)]
struct Stub {
  read: bool,
  write: bool,
  status: StatusCode,
  asked: Arc<Mutex<usize>>,
}

async fn sections(
  State(stub): State<Stub>,
  Path(user): Path<String>,
  headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
  *stub.asked.lock().unwrap() += 1;

  assert_eq!(headers.get("owner-key").unwrap(), user.as_str());
  assert_eq!(headers.get("owner-role").unwrap(), "member");
  assert_eq!(headers.get("authorization").unwrap(), format!("Bearer {TOKEN}").as_str());

  let body = json!({ "sections": [
    { "section": "live", "read": true, "write": true },
    { "section": "agents", "read": stub.read, "write": stub.write },
  ] });

  (stub.status, Json(body))
}

// The account service on a loopback port of its own, answering one user's sections as the test says.
async fn account_service(read: bool, write: bool, status: StatusCode) -> (String, Arc<Mutex<usize>>) {
  let asked = Arc::new(Mutex::new(0));
  let state = Stub {
    read,
    write,
    status,
    asked: asked.clone(),
  };
  let app = Router::new().route("/users/{user}/sections", get(sections)).with_state(state);
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  (format!("http://127.0.0.1:{port}"), asked)
}

// A ground seeded as the deployment seeds it, with the account service named in the data root's env file.
async fn ground(scratch: &Scratch, home: &std::path::Path, address: &str) -> Arc<Ground> {
  let served = seeded(scratch.db(), home).await;
  let env = home.join("data/env");

  std::fs::create_dir_all(&env).unwrap();
  std::fs::write(env.join("web.env"), format!("WEB_ADDRESS={address}\nWEB_TOKEN={TOKEN}\n")).unwrap();

  Arc::new(served)
}

async fn front(ground: Arc<Ground>) -> u16 {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();
  let app = router(Front::new(TOKEN.to_owned(), ground).unwrap());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  port
}

async fn call(
  port: u16,
  method: reqwest::Method,
  path: &str,
  key: Option<&str>,
  role: Option<&str>,
) -> (u16, Value) {
  let mut request = reqwest::Client::new()
    .request(method, format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(TOKEN);

  if let Some(key) = key {
    request = request.header("Owner-Key", key);
  }

  if let Some(role) = role {
    request = request.header("Owner-Role", role);
  }

  let answer = request.send().await.unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn a_member_with_the_agents_read_reads_each_of_the_five_kinds() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (address, asked) = account_service(true, false, StatusCode::OK).await;
  let port = front(ground(&scratch, home.path(), &address).await).await;

  for path in KINDS {
    let (status, body) = call(port, reqwest::Method::GET, path, Some("7"), Some("member")).await;

    assert_eq!(status, 200, "{path}: {body}");
  }

  assert_eq!(*asked.lock().unwrap(), KINDS.len());
}

#[tokio::test]
async fn a_member_with_the_agents_write_may_change() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (address, _) = account_service(true, true, StatusCode::OK).await;
  let port = front(ground(&scratch, home.path(), &address).await).await;
  let (status, body) = call(port, reqwest::Method::POST, "/roles", Some("7"), Some("member")).await;

  assert_ne!(status, 403, "a write lets the change reach the route: {body}");
}

#[tokio::test]
async fn a_change_without_the_agents_write_is_refused_before_the_route_reads_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (address, asked) = account_service(true, false, StatusCode::OK).await;
  let port = front(ground(&scratch, home.path(), &address).await).await;

  for path in KINDS {
    let (status, body) = call(port, reqwest::Method::POST, path, Some("7"), Some("member")).await;

    assert_eq!(status, 403, "{path}: {body}");
    assert!(body["error"].as_str().unwrap().contains("agents"), "{body}");
  }

  assert_eq!(*asked.lock().unwrap(), KINDS.len());
}

#[tokio::test]
async fn a_read_without_the_agents_read_is_refused() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (address, _) = account_service(false, false, StatusCode::OK).await;
  let port = front(ground(&scratch, home.path(), &address).await).await;
  let (status, body) = call(port, reqwest::Method::GET, "/supervisors", Some("7"), Some("member")).await;

  assert_eq!(status, 403, "{body}");
  assert!(body["error"].as_str().unwrap().contains("read"), "{body}");
}

#[tokio::test]
async fn the_administrator_and_a_caller_carrying_no_key_are_not_checked() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (address, asked) = account_service(false, false, StatusCode::OK).await;
  let port = front(ground(&scratch, home.path(), &address).await).await;

  let (administrator, _) =
    call(port, reqwest::Method::POST, "/roles", Some("7"), Some("administrator")).await;
  let (nobody, _) = call(port, reqwest::Method::POST, "/roles", None, None).await;

  assert_ne!(administrator, 403, "the owner keeps every access");
  assert_ne!(nobody, 403, "a unit's own call keeps the route's present behaviour");
  assert_eq!(*asked.lock().unwrap(), 0, "neither is asked of the account service");
}

#[tokio::test]
async fn a_permission_that_cannot_be_read_is_no_access() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (address, _) = account_service(false, false, StatusCode::INTERNAL_SERVER_ERROR).await;
  let port = front(ground(&scratch, home.path(), &address).await).await;
  let (status, body) = call(port, reqwest::Method::GET, "/supervisors", Some("7"), Some("member")).await;

  assert_eq!(status, 503, "{body}");
  assert_eq!(body["code"], "access-unchecked");
}
