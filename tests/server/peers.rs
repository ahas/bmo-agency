//! The services agency calls, asked through each client as the server asks them: over HTTP of a
//! stand-in of the test's own, which writes down each call and answers by its path, and by the defaults
//! where the deployment names no address.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agency_server::config::{Config, PeerAddresses};
use agency_server::peers::cache_siren::Window;
use agency_server::peers::node::{NodeRow, Room};
use agency_server::peers::{PeerFault, Peers};
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use tokio::net::TcpListener;

/// One call as the stand-in took it: its path, its authorization, and its body.
#[derive(Clone, Debug, PartialEq)]
struct Call {
  path: String,
  authorization: Option<String>,
  body: Value,
}

type Calls = Arc<Mutex<Vec<Call>>>;

async fn answer(
  State(calls): State<Calls>,
  uri: Uri,
  headers: HeaderMap,
  Json(body): Json<Value>,
) -> Response {
  let path = uri.path().to_owned();
  let authorization = headers.get("authorization").map(|value| value.to_str().unwrap().to_owned());

  calls.lock().unwrap().push(Call {
    path: path.clone(),
    authorization,
    body: body.clone(),
  });

  let answer = match path.strip_prefix("/base/").unwrap_or(&path) {
    "usage/windows" if body["account"] == "odd" => json!({ "windows": "none" }),
    "usage/windows" => json!({
      "windows": [{
        "name": "five_hour",
        "utilization": 0.25,
        "resetsAt": 1_700_003_600,
        "seenAt": 1_700_000_000,
        "durationMinutes": 300
      }],
      "freshSeconds": 1800
    }),
    "cache/lifetime" if body["model"] == "unjudged" => json!({ "minutes": null }),
    "cache/lifetime" => json!({ "minutes": 5 }),
    "machine/room" => json!({ "held": ["memory"] }),
    "links/state" => json!({ "up": body["node"] == "near" }),
    "nodes/list" => {
      json!({ "nodes": [
        { "node": "near", "label": "Near", "state": "up" },
        { "node": "far", "state": "down", "detail": "unheard" }
      ] })
    }
    "nodes/login" if body["node"] == "near" => json!({ "ssh": ["-o", "BatchMode=yes", "worker@near"] }),
    "nodes/login" => json!({ "ssh": null }),
    "agents/fence" | "agents/put-back" => json!({ "ok": true }),
    "agents/lock" if body["agent"] == "kept" => json!({ "path": "brains/kept/.lock" }),
    "agents/lock" => json!({ "path": null }),
    "agents/removed" => json!({
      "answered": false,
      "refusal": { "code": "folder-busy", "message": "The folder is being written." }
    }),
    "agents/book-since" => {
      return (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({ "code": "bad-room", "error": "No such room." })),
      )
        .into_response();
    }
    _ => return (StatusCode::NOT_FOUND, "nothing here").into_response(),
  };

  Json(answer).into_response()
}

/// The stand-in served on a free port; its address and the calls it took.
async fn stand_in() -> (String, Calls) {
  let calls = Calls::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}/base/", listener.local_addr().unwrap());
  let routes = axum::Router::new().fallback(answer).with_state(calls.clone());

  tokio::spawn(async move { axum::serve(listener, routes).await.unwrap() });

  (address, calls)
}

fn peers_at(address: &str, token: Option<&str>) -> Peers {
  let config = Config::read(|name| match name {
    "AGENCY_CACHE_SIREN_URL" | "AGENCY_NODE_URL" | "AGENCY_BRAIN_URL" => Some(address.to_owned()),
    "AGENCY_CACHE_SIREN_TOKEN" | "AGENCY_NODE_TOKEN" | "AGENCY_BRAIN_TOKEN" => token.map(str::to_owned),
    _ => None,
  })
  .unwrap();

  Peers::of(&config.peers)
}

fn taken(calls: &Calls) -> Vec<Call> {
  std::mem::take(&mut *calls.lock().unwrap())
}

#[tokio::test]
async fn the_cache_siren_is_asked_for_an_accounts_windows() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, Some("siren-token"));
  let reading = peers.cache_siren.usage_windows("claude-code", "main").await.unwrap();

  assert_eq!(
    reading.windows,
    vec![Window {
      name: "five_hour".into(),
      utilization: 0.25,
      resets_at: Some(1_700_003_600),
      seen_at: 1_700_000_000,
      duration_minutes: Some(300),
      limit_name: None,
    }]
  );
  assert_eq!(reading.fresh(), Duration::from_secs(1800));
  assert_eq!(
    taken(&calls),
    vec![Call {
      path: "/base/usage/windows".into(),
      authorization: Some("Bearer siren-token".into()),
      body: json!({ "mode": "claude-code", "account": "main" }),
    }]
  );

  let fault = peers.cache_siren.usage_windows("claude-code", "odd").await.unwrap_err();

  assert!(
    matches!(
      fault,
      PeerFault::Malformed {
        peer: "cache siren",
        ..
      }
    ),
    "{fault:?}"
  );
}

#[tokio::test]
async fn the_node_service_is_asked_for_room_links_nodes_and_logins() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, None);

  assert_eq!(
    peers.node.machine_room().await.unwrap(),
    Room {
      held: vec!["memory".into()]
    }
  );
  assert!(peers.node.link_up("near").await.unwrap());
  assert!(!peers.node.link_up("far").await.unwrap());
  assert_eq!(
    peers.node.node_rows().await.unwrap(),
    vec![
      NodeRow {
        node: "near".into(),
        label: Some("Near".into()),
        state: "up".into(),
        detail: None,
      },
      NodeRow {
        node: "far".into(),
        label: None,
        state: "down".into(),
        detail: Some("unheard".into()),
      },
    ]
  );
  assert_eq!(
    peers.node.ssh_login("near").await.unwrap(),
    Some(vec!["-o".into(), "BatchMode=yes".into(), "worker@near".into()])
  );
  assert_eq!(peers.node.ssh_login("far").await.unwrap(), None);

  let calls = taken(&calls);
  let paths: Vec<&str> = calls.iter().map(|call| call.path.as_str()).collect();

  assert_eq!(
    paths,
    [
      "/base/machine/room",
      "/base/links/state",
      "/base/links/state",
      "/base/nodes/list",
      "/base/nodes/login",
      "/base/nodes/login"
    ]
  );
  assert_eq!(calls[4].body, json!({ "node": "near" }));
  assert!(calls.iter().all(|call| call.authorization.is_none()), "no token named, none sent");
  assert_eq!(calls[1].body, json!({ "node": "near" }));
}

#[tokio::test]
async fn the_brain_is_asked_by_the_agent_and_its_refusals_come_back_as_faults() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, Some("brain-token"));

  peers.brain.fence("kept").await.unwrap();
  peers.brain.put_back("kept").await.unwrap();
  assert_eq!(peers.brain.lock_path("kept").await.unwrap(), Some("brains/kept/.lock".into()));
  assert_eq!(peers.brain.lock_path("gone").await.unwrap(), None);

  let refused = peers.brain.forget("kept").await.unwrap_err();

  assert_eq!(
    refused,
    PeerFault::Refused {
      peer: "brain",
      code: "folder-busy".into(),
      message: "The folder is being written.".into(),
    }
  );

  let failed = peers.brain.book_since("kept", -100, 1_700_000_000).await.unwrap_err();

  assert_eq!(
    failed,
    PeerFault::Failed {
      peer: "brain",
      status: 422,
      code: "bad-room".into(),
      error: "No such room.".into(),
    }
  );

  let calls = taken(&calls);

  assert!(calls.iter().all(|call| call.authorization.as_deref() == Some("Bearer brain-token")));
  assert_eq!(
    calls[0],
    Call {
      path: "/base/agents/fence".into(),
      authorization: Some("Bearer brain-token".into()),
      body: json!({ "agent": "kept" }),
    }
  );
  assert_eq!(
    calls.last().unwrap().body,
    json!({ "agent": "kept", "chatId": -100, "at": 1_700_000_000 })
  );
}

#[tokio::test]
async fn a_service_with_no_listener_is_unreachable() {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}/", listener.local_addr().unwrap());

  drop(listener);

  let fault = peers_at(&address, None).node.machine_room().await.unwrap_err();

  assert!(
    matches!(
      fault,
      PeerFault::Unreachable {
        peer: "node service",
        ..
      }
    ),
    "{fault:?}"
  );
}

#[tokio::test]
async fn the_cache_siren_is_asked_how_long_a_session_on_a_provider_and_model_stays_warm() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, None);

  assert_eq!(peers.cache_siren.cache_lifetime("claude-code", "model-a").await.unwrap(), Some(5));
  assert_eq!(peers.cache_siren.cache_lifetime("claude-code", "unjudged").await.unwrap(), None);
  assert_eq!(
    taken(&calls)[0],
    Call {
      path: "/base/cache/lifetime".into(),
      authorization: None,
      body: json!({ "mode": "claude-code", "model": "model-a" }),
    }
  );
}

#[tokio::test]
async fn a_service_with_no_address_answers_its_default() {
  let peers = Peers::of(&PeerAddresses::default());
  let reading = peers.cache_siren.usage_windows("claude-code", "main").await.unwrap();

  assert!(reading.windows.is_empty());
  assert_eq!(reading.fresh(), Duration::ZERO);
  assert_eq!(peers.cache_siren.cache_lifetime("claude-code", "model-a").await.unwrap(), None);
  assert_eq!(peers.node.machine_room().await.unwrap(), Room::default());
  assert!(!peers.node.link_up("near").await.unwrap());
  assert!(peers.node.node_rows().await.unwrap().is_empty());
  assert_eq!(peers.node.ssh_login("near").await.unwrap(), None);
  peers.brain.fence("kept").await.unwrap();
  peers.brain.forget("kept").await.unwrap();
  peers.brain.book_since("kept", 1, 2).await.unwrap();
  peers.brain.put_back("kept").await.unwrap();
  assert_eq!(peers.brain.lock_path("kept").await.unwrap(), None);
}
