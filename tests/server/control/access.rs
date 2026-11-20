//! Who may read or change agent management (docs/api.md, 「Who a caller acts for」): a member's five kinds of
//! route asked with the console's permission for the section of each, read and write apart, and the agent list
//! read under any of the five; the administrator and a caller carrying no key unchecked; and a permission that
//! could not be read refused, never guessed.

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

/// The five kinds of route agent management is read and changed through, each with its console section.
const PARTS: [(&str, &str); 5] = [
  ("/supervisors", "supervisors"),
  ("/roles", "roles"),
  ("/repositories", "repositories"),
  ("/models", "models"),
  ("/model-catalogue", "model-files"),
];

/// The kinds the agent list is read from.
const AGENT_LIST: [&str; 3] = ["/supervisors", "/roles", "/repositories"];

/// One section a stub grants: its name, its read and its write.
type Grant = (&'static str, bool, bool);

#[derive(Clone)]
struct Stub {
  grants: Arc<Mutex<Vec<Grant>>>,
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

  // The retired `agents` section stands granted beside every answer, so a gate still reading it passes a call
  // the tests expect refused.
  let rows = [("live", true, true), ("agents", true, true)]
    .into_iter()
    .chain(stub.grants.lock().unwrap().iter().copied())
    .map(|(section, read, write)| json!({ "section": section, "read": read, "write": write }))
    .collect::<Vec<_>>();

  (stub.status, Json(json!({ "sections": rows })))
}

/// What a test holds of its account service: the sections it grants, which the test may change between calls,
/// and how often it was asked.
struct Granting {
  grants: Arc<Mutex<Vec<Grant>>>,
  asked: Arc<Mutex<usize>>,
}

impl Granting {
  fn grant(&self, grants: &[Grant]) {
    *self.grants.lock().unwrap() = grants.to_vec();
  }

  fn asked(&self) -> usize {
    *self.asked.lock().unwrap()
  }
}

// The account service on a loopback port of its own, answering one user's sections as the test says.
async fn account_service(grants: &[Grant], status: StatusCode) -> (String, Granting) {
  let granting = Granting {
    grants: Arc::new(Mutex::new(grants.to_vec())),
    asked: Arc::new(Mutex::new(0)),
  };
  let state = Stub {
    grants: granting.grants.clone(),
    status,
    asked: granting.asked.clone(),
  };
  let app = Router::new().route("/users/{user}/sections", get(sections)).with_state(state);
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  (format!("http://127.0.0.1:{port}"), granting)
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

// The front on a fresh ground whose account service grants a member what the test says.
async fn granted(scratch: &Scratch, home: &std::path::Path, grants: &[Grant]) -> (u16, Granting) {
  let (address, granting) = account_service(grants, StatusCode::OK).await;

  (front(ground(scratch, home, &address).await).await, granting)
}

async fn member(port: u16, method: reqwest::Method, path: &str) -> (u16, Value) {
  call(port, method, path, Some("7"), Some("member")).await
}

#[tokio::test]
async fn a_member_reads_each_kind_under_its_own_section_and_the_models_under_no_other() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (port, granting) = granted(&scratch, home.path(), &[]).await;

  for (path, section) in PARTS {
    granting.grant(&[(section, true, false)]);

    let (status, body) = member(port, reqwest::Method::GET, path).await;

    assert_eq!(status, 200, "{section} reads {path}: {body}");

    for (other, _) in PARTS.into_iter().filter(|(other, _)| *other != path && !AGENT_LIST.contains(other)) {
      let (status, body) = member(port, reqwest::Method::GET, other).await;

      assert_eq!(status, 403, "{section} does not read {other}: {body}");
      assert!(body["error"].as_str().unwrap().contains("read"), "{body}");
    }
  }
}

#[tokio::test]
async fn the_agent_list_is_read_under_any_part_and_under_none_is_refused() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (port, granting) = granted(&scratch, home.path(), &[("model-files", true, false)]).await;

  for path in AGENT_LIST {
    let (status, body) = member(port, reqwest::Method::GET, path).await;

    assert_eq!(status, 200, "{path}: {body}");
  }

  granting.grant(&[("rooms", true, true)]);

  let (status, body) = member(port, reqwest::Method::GET, "/supervisors").await;

  assert_eq!(status, 403, "{body}");
  assert!(body["error"].as_str().unwrap().contains("any section of agent management"), "{body}");
}

#[tokio::test]
async fn a_change_needs_its_own_sections_write() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let mut grants: Vec<Grant> = PARTS.iter().map(|(_, section)| (*section, true, false)).collect();

  grants.push(("roles", true, true));

  let (port, granting) = granted(&scratch, home.path(), &grants).await;
  let (status, body) = member(port, reqwest::Method::POST, "/roles").await;

  assert_ne!(status, 403, "the roles write lets the change reach the route: {body}");

  for (path, section) in PARTS.into_iter().filter(|(path, _)| *path != "/roles") {
    let (status, body) = member(port, reqwest::Method::POST, path).await;

    assert_eq!(status, 403, "{path}: {body}");
    assert!(
      body["error"]
        .as_str()
        .unwrap()
        .contains(&format!("write access to the {section} section")),
      "{body}"
    );
  }

  assert_eq!(granting.asked(), PARTS.len());
}

#[tokio::test]
async fn the_administrator_and_a_caller_carrying_no_key_are_not_checked() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (port, granting) = granted(&scratch, home.path(), &[]).await;

  let (administrator, _) =
    call(port, reqwest::Method::POST, "/roles", Some("7"), Some("administrator")).await;
  let (nobody, _) = call(port, reqwest::Method::POST, "/roles", None, None).await;

  assert_ne!(administrator, 403, "the owner keeps every access");
  assert_ne!(nobody, 403, "a unit's own call keeps the route's present behaviour");
  assert_eq!(granting.asked(), 0, "neither is asked of the account service");
}

#[tokio::test]
async fn a_permission_that_cannot_be_read_is_no_access() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (address, _) = account_service(&[], StatusCode::INTERNAL_SERVER_ERROR).await;
  let port = front(ground(&scratch, home.path(), &address).await).await;
  let (status, body) = member(port, reqwest::Method::GET, "/supervisors").await;

  assert_eq!(status, 503, "{body}");
  assert_eq!(body["code"], "access-unchecked");
}
