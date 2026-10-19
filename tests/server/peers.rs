//! The services agency calls, asked through each client as the server asks them: over HTTP of a
//! stand-in of the test's own, which writes down each call and answers by its path, and by the defaults
//! where the deployment names no address.

use std::sync::{Arc, Mutex};

use agency_server::config::{Config, PeerAddresses};
use agency_server::peers::brain::{BrainReset, holds_watch};
use agency_server::peers::node::{
  LargeProcess, NodeKind, NodeRead, NodeRow, PlaceHolding, Pressure, Room, SshReach, Standing,
};
use agency_server::peers::{PeerFault, Peers};
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
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

// The node service's pressures as its stream tells them: an event of another kind, one whose data does not
// read, and one pressure of each cause.
const PRESSURES: &str = "event: nodes-changed\ndata: {}\n\n\
event: machine-pressure\ndata: {\"cause\":\"heat\"}\n\n\
event: machine-pressure\ndata: {\"cause\":\"memory\",\"fullPct\":12.5,\"alertPct\":10,\"minutes\":2,\"reading\":{\"load1\":30},\"places\":[{\"place\":\"worker\",\"megabytes\":900.5,\"browsers\":1}],\"largest\":[{\"pid\":7,\"name\":\"chrome\",\"megabytes\":400,\"place\":\"worker\"}]}\n\n\
event: machine-pressure\ndata: {\"cause\":\"disk\",\"freeGb\":12,\"limitGb\":20}\n\n";

async fn answer(
  State(calls): State<Calls>,
  uri: Uri,
  headers: HeaderMap,
  body: axum::body::Bytes,
) -> Response {
  let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
  let path = uri.path().to_owned();
  let authorization = headers.get("authorization").map(|value| value.to_str().unwrap().to_owned());

  calls.lock().unwrap().push(Call {
    path: path.clone(),
    authorization,
    body: body.clone(),
  });

  let answer = match path.strip_prefix("/base/").unwrap_or(&path) {
    "machine/room" => json!({ "held": ["memory"] }),
    "machine/standing" => json!({
      "reading": { "load1": 3 },
      "held": [],
      "browsers": 2,
      "places": [{ "place": "worker", "megabytes": 900.5, "browsers": 2 }],
      "largest": []
    }),
    "events" if uri.query() == Some("kinds=machine-pressure") => {
      return ([("content-type", "text/event-stream")], PRESSURES).into_response();
    }
    "links/state" => json!({ "up": body["node"] == "near" }),
    "nodes/list" => {
      json!({ "nodes": [
        { "node": "near", "label": "Near", "kind": "ssh", "state": "up" },
        { "node": "far", "kind": "jake", "state": "down", "detail": "unheard" }
      ] })
    }
    "nodes/login" if body["node"] == "near" => json!({ "ssh": ["-o", "BatchMode=yes", "worker@near"] }),
    "nodes/login" => json!({ "ssh": null }),
    "nodes" => json!({ "nodes": [
      { "name": "near", "label": "Near", "kind": "ssh", "role": "jobs", "state": "up", "checkedAt": 1_700_000_000 },
      { "name": "far", "label": "Far", "kind": "jake", "state": "down" }
    ] }),
    "nodes/near/reach" => {
      json!({ "host": "near.lan", "port": 2222, "user": "worker", "keyPath": "/keys/near" })
    }
    "nodes/far/reach" => json!({
      "answered": false,
      "refusal": { "code": "not-ssh", "message": "The node is reached through a hub." }
    }),
    "nodes/gone/reach" => json!({
      "answered": false,
      "refusal": { "code": "no-node", "message": "No node of that name is listed." }
    }),
    "nodes/odd/reach" => json!({
      "answered": false,
      "refusal": { "code": "incomplete", "message": "The node lacks a field." }
    }),
    "agents/fence" | "agents/put-back" => json!({ "ok": true }),
    "watches/stopped" => json!({ "ended": body["unit"] == "bmo-watch-12-1700000000000" }),
    "agents/lock" if body["agent"] == "kept" => json!({ "path": "brains/kept/.lock" }),
    "agents/lock" => json!({ "path": null }),
    "agents/reset" => json!({
      "took": { "memories": if body["preview"] == true { 3 } else { 0 }, "folder": true },
      "couldNot": ["brain commit: nothing to push"]
    }),
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
        kind: NodeKind::Ssh,
        state: "up".into(),
        detail: None,
      },
      NodeRow {
        node: "far".into(),
        label: None,
        kind: NodeKind::Jake,
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
async fn the_node_service_is_asked_for_its_standing_and_heard_telling_pressures() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, Some("node-token"));

  assert_eq!(
    peers.node.machine_standing().await.unwrap(),
    Standing {
      browsers: 2,
      places: vec![PlaceHolding {
        place: "worker".into(),
        megabytes: 900.5,
        browsers: 2
      }],
    }
  );

  let told: Vec<_> = peers.node.pressures().await.unwrap().collect().await;

  assert_eq!(told.len(), 3, "the event of another kind is passed over: {told:?}");
  assert!(matches!(told[0], Err(PeerFault::Malformed { .. })), "{:?}", told[0]);
  assert_eq!(
    told[1].as_ref().unwrap(),
    &Pressure::Memory {
      full_pct: 12.5,
      alert_pct: 10.0,
      minutes: 2.0,
      reading: Some(json!({ "load1": 30 })),
      places: vec![PlaceHolding {
        place: "worker".into(),
        megabytes: 900.5,
        browsers: 1
      }],
      largest: vec![LargeProcess {
        pid: 7,
        name: "chrome".into(),
        megabytes: 400.0,
        place: Some("worker".into())
      }],
    }
  );
  assert_eq!(
    told[2].as_ref().unwrap(),
    &Pressure::Disk {
      free_gb: 12.0,
      limit_gb: 20.0,
      reading: None
    }
  );

  let calls = taken(&calls);

  assert_eq!(
    calls.iter().map(|call| call.path.as_str()).collect::<Vec<_>>(),
    ["/base/machine/standing", "/base/events"]
  );
  assert!(calls.iter().all(|call| call.authorization.as_deref() == Some("Bearer node-token")));
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
async fn the_node_service_s_reads_are_asked_and_a_node_with_no_reach_is_none() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, None);

  assert_eq!(
    peers.node.node_reads().await.unwrap(),
    vec![
      NodeRead {
        name: "near".into(),
        label: "Near".into(),
        kind: NodeKind::Ssh,
        role: Some("jobs".into()),
        state: "up".into(),
        checked_at: Some(1_700_000_000),
      },
      NodeRead {
        name: "far".into(),
        label: "Far".into(),
        kind: NodeKind::Jake,
        role: None,
        state: "down".into(),
        checked_at: None,
      },
    ]
  );
  assert_eq!(
    peers.node.ssh_reach("near").await.unwrap(),
    Some(SshReach {
      host: "near.lan".into(),
      port: 2222,
      user: "worker".into(),
      key_path: Some("/keys/near".into()),
      shell: None,
    })
  );
  assert_eq!(peers.node.ssh_reach("far").await.unwrap(), None, "reached through a hub");
  assert_eq!(peers.node.ssh_reach("gone").await.unwrap(), None, "not listed");
  assert!(matches!(peers.node.ssh_reach("odd").await, Err(PeerFault::Refused { .. })));

  let paths: Vec<String> = taken(&calls).into_iter().map(|call| call.path).collect();

  assert_eq!(
    paths,
    [
      "/base/nodes",
      "/base/nodes/near/reach",
      "/base/nodes/far/reach",
      "/base/nodes/gone/reach",
      "/base/nodes/odd/reach"
    ]
  );

  let none = Peers::of(&PeerAddresses::default());

  assert!(none.node.node_reads().await.unwrap().is_empty());
  assert_eq!(none.node.ssh_reach("near").await.unwrap(), None);
}

#[tokio::test]
async fn a_service_with_no_address_answers_its_default() {
  let peers = Peers::of(&PeerAddresses::default());

  assert_eq!(peers.node.machine_room().await.unwrap(), Room::default());
  assert_eq!(peers.node.machine_standing().await.unwrap(), Standing::default());
  assert!(!peers.node.link_up("near").await.unwrap());
  assert!(peers.node.node_rows().await.unwrap().is_empty());
  assert_eq!(peers.node.ssh_login("near").await.unwrap(), None);
  peers.brain.fence("kept").await.unwrap();
  peers.brain.forget("kept").await.unwrap();
  peers.brain.book_since("kept", 1, 2).await.unwrap();
  peers.brain.put_back("kept").await.unwrap();
  assert_eq!(peers.brain.lock_path("kept").await.unwrap(), None);
  assert_eq!(peers.brain.reset("kept", false).await.unwrap(), BrainReset::default());
  assert!(!peers.brain.watch_stopped("bmo-watch-12-1700000000000").await.unwrap());
}

#[tokio::test]
async fn the_brain_is_told_a_watchs_unit_stopped_and_answers_whether_it_held_one() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, None);

  assert!(holds_watch("bmo-watch-12-1700000000000"));
  assert!(!holds_watch("bmo-watch-12"));
  assert!(!holds_watch("bmo-digest"));
  assert!(peers.brain.watch_stopped("bmo-watch-12-1700000000000").await.unwrap());
  assert!(!peers.brain.watch_stopped("bmo-watch-13-1700000000000").await.unwrap());
  let told = taken(&calls);

  assert_eq!(told[0].path, "/base/watches/stopped");
  assert_eq!(told[0].body, json!({ "unit": "bmo-watch-12-1700000000000" }));
}

#[tokio::test]
async fn the_brain_is_asked_to_reset_an_agent_or_to_preview_it() {
  let (address, calls) = stand_in().await;
  let peers = peers_at(&address, None);
  let preview = peers.brain.reset("kept", true).await.unwrap();

  assert_eq!(preview.took, json!({ "memories": 3, "folder": true }));
  assert_eq!(preview.could_not, ["brain commit: nothing to push"]);
  assert_eq!(
    taken(&calls)[0],
    Call {
      path: "/base/agents/reset".into(),
      authorization: None,
      body: json!({ "agent": "kept", "preview": true }),
    }
  );
}
