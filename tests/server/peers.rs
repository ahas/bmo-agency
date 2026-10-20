//! The services agency calls, asked through the clients their repositories publish: over HTTP of a
//! stand-in of the test's own, which writes down each call and answers by its path, and by the defaults
//! where the deployment names no address.

use std::sync::{Arc, Mutex};

use agency_server::config::{Config, PeerAddresses};
use agency_server::peers::brain::BrainReset;
use agency_server::peers::node::NodeKind;
use agency_server::peers::{PeerFault, Peers};
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio::net::TcpListener;

/// One call as the stand-in took it: its method, its path and query, its authorization, and its body.
#[derive(Clone, Debug, PartialEq)]
struct Call {
  method: String,
  path: String,
  query: Option<String>,
  authorization: Option<String>,
  body: Value,
}

type Calls = Arc<Mutex<Vec<Call>>>;

fn node(name: &str, label: &str, kind: &str, role: Option<&str>, state: &str, up: bool) -> Value {
  json!({
    "name": name,
    "label": label,
    "kind": kind,
    "role": role,
    "address": format!("worker@{name}.lan"),
    "state": state,
    "tone": if up { "good" } else { "bad" },
    "checkedAt": 1_700_000_000_000_i64,
    "up": up,
    "thisMachine": false
  })
}

fn machine() -> Value {
  json!({
    "name": "local",
    "label": "This machine",
    "kind": "local",
    "tracking": true,
    "readAt": 1_700_000_000_000_i64,
    "standing": {
      "reading": {
        "at": 1_700_000_000_000_i64,
        "load1": 3.0,
        "threads": 24,
        "memTotalGb": 32.0,
        "memAvailableGb": 8.0,
        "swapTotalGb": 16.0,
        "swapFreeGb": 8.0,
        "diskFreeGb": 100.0,
        "memory": null,
        "cpu": null
      },
      "alerts": [],
      "browsers": 2,
      "scopes": [{ "scope": "worker", "megabytes": 900, "browsers": 2 }],
      "largest": []
    }
  })
}

const CHANGES: &str = "event: machine.changed\ndata: not-json\n\n\
event: machine.changed\ndata: {\"id\":2,\"kind\":\"machine.changed\",\"at\":1700000000000,\"data\":{\"name\":\"local\",\"label\":\"This machine\",\"kind\":\"local\",\"tracking\":true,\"standing\":{\"reading\":null,\"alerts\":[{\"cause\":\"memory\",\"fullPct\":12.5,\"alertPct\":10,\"minutes\":2}],\"browsers\":2,\"scopes\":[],\"largest\":[]}}}\n\n";

fn count(scope: &str, memories: i64) -> Value {
  json!({
    "scope": scope,
    "counts": {
      "memories": memories,
      "memoryMatches": 0,
      "notes": 0,
      "watches": 0,
      "reminders": 0,
      "fragments": 0,
      "fragmentHits": 0
    },
    "state": "remembered"
  })
}

async fn answer(
  State(calls): State<Calls>,
  method: Method,
  uri: Uri,
  headers: HeaderMap,
  body: axum::body::Bytes,
) -> Response {
  let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
  let path = uri.path().to_owned();
  let query = uri.query().map(str::to_owned);
  let authorization = headers.get("authorization").map(|value| value.to_str().unwrap().to_owned());

  calls.lock().unwrap().push(Call {
    method: method.to_string(),
    path: path.clone(),
    query: query.clone(),
    authorization,
    body: body.clone(),
  });

  let answer = match (method.as_str(), path.strip_prefix("/base/").unwrap_or(&path)) {
    ("GET", "nodes") => json!({
      "items": [
        node("near", "Near", "ssh", Some("jobs"), "connected", true),
        node("far", "Far", "hub", None, "unreachable", false)
      ],
      "more": false
    }),
    ("GET", "nodes/near") => node("near", "Near", "ssh", Some("jobs"), "connected", true),
    ("GET", "nodes/far") => node("far", "Far", "hub", None, "unreachable", false),
    ("GET", "nodes/near/target") => json!({
      "nodeName": "near",
      "kind": "ssh",
      "role": "jobs",
      "host": "near.example",
      "port": 2222,
      "user": "worker",
      "keyPath": "/keys/near",
      "shell": "posix",
      "sshOptions": ["-o", "BatchMode=yes"],
      "sshArgs": ["-o", "BatchMode=yes", "worker@near.example"]
    }),
    ("GET", "nodes/far/target") => json!({
      "nodeName": "far",
      "kind": "hub",
      "sshOptions": [],
      "sshArgs": []
    }),
    ("GET", "nodes/gone/target") => {
      return (
        StatusCode::NOT_FOUND,
        Json(json!({ "code": "not-found", "error": "No node of that name." })),
      )
        .into_response();
    }
    ("GET", "nodes/odd/target") => {
      return (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "code": "failed", "error": "The check failed." })),
      )
        .into_response();
    }
    ("GET", "machines/local") => machine(),
    ("GET", "events") if query.as_deref().is_some_and(|query| query.contains("kinds=machine.changed")) => {
      return ([("content-type", "text/event-stream")], CHANGES).into_response();
    }
    ("DELETE", "scopes/kept") if query.as_deref() == Some("dryRun=true") => count("kept", 3),
    ("DELETE", "scopes/kept") => count("kept", 0),
    ("DELETE", "scopes/gone") => {
      return (StatusCode::NOT_FOUND, Json(json!({ "code": "not-found", "error": "Nothing is held." })))
        .into_response();
    }
    ("DELETE", "scopes/busy") => {
      return (
        StatusCode::CONFLICT,
        Json(json!({ "code": "conflict", "error": "The scope is being written." })),
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
    "AGENCY_NODE_URL" | "AGENCY_BRAIN_URL" => Some(address.to_owned()),
    "AGENCY_NODE_TOKEN" | "AGENCY_BRAIN_TOKEN" => token.map(str::to_owned),
    _ => None,
  })
  .unwrap();

  Peers::of(&config.peers)
}

fn taken(calls: &Calls) -> Vec<Call> {
  std::mem::take(&mut *calls.lock().unwrap())
}

#[tokio::test]
async fn the_node_service_is_asked_for_its_links_nodes_and_logins() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, Some("node-token"));

  assert!(peers.node.link_up("near").await.unwrap());
  assert!(!peers.node.link_up("far").await.unwrap());

  let rows = peers.node.node_rows().await.unwrap();

  assert_eq!(rows.len(), 2);
  assert_eq!(rows[0].node, "near");
  assert_eq!(rows[0].label.as_deref(), Some("Near"));
  assert_eq!(rows[0].kind, NodeKind::Ssh);
  assert_eq!(rows[0].state, "connected");
  assert_eq!(rows[1].node, "far");
  assert_eq!(rows[1].kind, NodeKind::Jake);

  assert_eq!(
    peers.node.ssh_login("near").await.unwrap(),
    Some(vec!["-o".to_owned(), "BatchMode=yes".to_owned(), "worker@near.example".to_owned()])
  );
  assert_eq!(peers.node.ssh_login("far").await.unwrap(), None);
  assert_eq!(
    peers.node.ssh_login("gone").await.unwrap(),
    None,
    "a node that is not listed has no route"
  );

  let calls = taken(&calls);
  let paths: Vec<&str> = calls.iter().map(|call| call.path.as_str()).collect();

  assert_eq!(
    paths,
    [
      "/base/nodes/near",
      "/base/nodes/far",
      "/base/nodes",
      "/base/nodes/near/target",
      "/base/nodes/far/target",
      "/base/nodes/gone/target"
    ]
  );
  assert!(calls.iter().all(|call| call.authorization.as_deref() == Some("Bearer node-token")));
}

#[tokio::test]
async fn the_node_service_is_asked_for_its_standing_and_heard_telling_changes() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, Some("node-token"));

  let standing = peers.node.machine_standing().await.unwrap();

  assert_eq!(standing.browsers, 2);
  assert_eq!(standing.scopes.len(), 1);
  assert_eq!(standing.scopes[0].scope, "worker");
  assert_eq!(standing.scopes[0].megabytes, 900);

  let told: Vec<_> = peers.node.pressures().await.unwrap().collect().await;

  assert_eq!(told.len(), 2, "the event of another shape is not passed over: {told:?}");
  assert!(matches!(told[0], Err(PeerFault::Malformed { .. })), "{:?}", told[0]);

  let machine = told[1].as_ref().unwrap();
  let alerts = &machine.standing.as_ref().unwrap().alerts;

  assert_eq!(alerts.len(), 1);
  assert!(matches!(alerts[0], agency_server::peers::node::Alert::Memory { full_pct: 12.5, .. }));

  let calls = taken(&calls);

  assert_eq!(
    calls.iter().map(|call| call.path.as_str()).collect::<Vec<_>>(),
    ["/base/machines/local", "/base/events"]
  );
  assert_eq!(
    calls[1].query.as_deref(),
    Some("kinds=machine.changed&machineName=local"),
    "only this machine's changes are read"
  );
  assert!(calls.iter().all(|call| call.authorization.as_deref() == Some("Bearer node-token")));
}

#[tokio::test]
async fn the_brain_is_asked_by_the_agent_and_its_refusals_come_back_as_faults() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, Some("brain-token"));

  let preview = peers.brain.reset("kept", true).await.unwrap();

  assert_eq!(preview.took["memories"], json!(3));
  assert_eq!(preview.could_not, Vec::<String>::new());

  let took = peers.brain.reset("kept", false).await.unwrap();

  assert_eq!(took.took["memories"], json!(0));

  peers.brain.forget("kept").await.unwrap();
  peers.brain.forget("gone").await.unwrap();
  assert_eq!(peers.brain.reset("gone", false).await.unwrap(), BrainReset::default());

  let refused = peers.brain.forget("busy").await.unwrap_err();

  assert_eq!(
    refused,
    PeerFault::Failed {
      peer: "brain",
      status: 409,
      code: "conflict".into(),
      error: "The scope is being written.".into(),
    }
  );

  let calls = taken(&calls);

  assert_eq!(
    calls
      .iter()
      .map(|call| (call.method.as_str(), call.path.as_str(), call.query.as_deref()))
      .collect::<Vec<_>>(),
    [
      ("DELETE", "/base/scopes/kept", Some("dryRun=true")),
      ("DELETE", "/base/scopes/kept", None),
      ("DELETE", "/base/scopes/kept", None),
      ("DELETE", "/base/scopes/gone", None),
      ("DELETE", "/base/scopes/gone", None),
      ("DELETE", "/base/scopes/busy", None)
    ]
  );
  assert!(calls.iter().all(|call| call.authorization.as_deref() == Some("Bearer brain-token")));
}

#[tokio::test]
async fn a_service_with_no_listener_is_unreachable() {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}/", listener.local_addr().unwrap());

  drop(listener);

  let fault = peers_at(&address, None).node.machine_standing().await.unwrap_err();

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

  let fault = peers_at(&address, None).brain.forget("kept").await.unwrap_err();

  assert!(matches!(fault, PeerFault::Unreachable { peer: "brain", .. }), "{fault:?}");
}

#[tokio::test]
async fn the_node_service_s_reads_are_asked_and_a_node_with_no_reach_is_none() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, None);

  let reads = peers.node.node_reads().await.unwrap();

  assert_eq!(reads.len(), 2);
  assert_eq!(reads[0].name, "near");
  assert_eq!(reads[0].kind, NodeKind::Ssh);
  assert_eq!(reads[0].role.as_deref(), Some("jobs"));
  assert_eq!(reads[0].checked_at, Some(1_700_000_000_000));
  assert_eq!(reads[1].name, "far");
  assert_eq!(reads[1].kind, NodeKind::Jake);

  let near = peers.node.ssh_reach("near").await.unwrap().unwrap();

  assert_eq!(near.host, "near.example");
  assert_eq!(near.port, 2222);
  assert_eq!(near.user, "worker");
  assert_eq!(near.key_path.as_deref(), Some("/keys/near"));
  assert_eq!(near.shell.as_deref(), Some("posix"));
  assert_eq!(peers.node.ssh_reach("far").await.unwrap(), None, "reached through a hub");
  assert_eq!(peers.node.ssh_reach("gone").await.unwrap(), None, "not listed");
  assert!(matches!(peers.node.ssh_reach("odd").await, Err(PeerFault::Failed { status: 500, .. })));

  let paths: Vec<String> = taken(&calls).into_iter().map(|call| call.path).collect();

  assert_eq!(
    paths,
    [
      "/base/nodes",
      "/base/nodes/near/target",
      "/base/nodes/far/target",
      "/base/nodes/gone/target",
      "/base/nodes/odd/target"
    ]
  );
}

#[tokio::test]
async fn a_service_with_no_address_answers_its_default() {
  let peers = Peers::of(&PeerAddresses::default());

  assert!(!peers.node.link_up("near").await.unwrap());
  assert!(peers.node.node_rows().await.unwrap().is_empty());
  assert_eq!(peers.node.ssh_login("near").await.unwrap(), None);
  assert_eq!(peers.node.machine_standing().await.unwrap().browsers, 0);
  assert!(peers.node.pressures().await.is_ok(), "a stream that tells none is opened");
  peers.brain.forget("kept").await.unwrap();
  assert_eq!(peers.brain.reset("kept", false).await.unwrap(), BrainReset::default());
}
