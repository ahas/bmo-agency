//! The node service (docs/api.md, 「Calls agency makes」): this machine's room for another launch, whether
//! a node's link stands, the nodes it keeps and the SSH login to each.

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use super::{Asking, Caller, Patience, PeerAddress, Variables};

/// Where the deployment names the node service.
pub const ADDRESS: Variables = Variables {
  url: "AGENCY_NODE_URL",
  token: "AGENCY_NODE_TOKEN",
};

const PEER: &str = "node service";

// A reading asked on a clock or before a launch is not waited on for long.
const ANSWER_WITHIN: Duration = Duration::from_secs(15);

/// This machine's room: what it is short of for another launch, each named, and nothing where it has
/// room.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Room {
  pub held: Vec<String>,
}

/// One node the node service keeps, with the label it is shown under, its state and what it says of it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct NodeRow {
  pub node: String,
  #[serde(default)]
  pub label: Option<String>,
  pub state: String,
  #[serde(default)]
  pub detail: Option<String>,
}

#[derive(Deserialize)]
struct LinkState {
  up: bool,
}

#[derive(Deserialize)]
struct NodeRows {
  nodes: Vec<NodeRow>,
}

#[derive(Deserialize)]
struct Login {
  ssh: Option<Vec<String>>,
}

/// What agency asks the node service.
pub trait NodeService: Send + Sync {
  /// This machine's room for another launch.
  fn machine_room(&self) -> Asking<'_, Room>;
  /// Whether a node's link is connected and was heard from lately.
  fn link_up<'a>(&'a self, node: &'a str) -> Asking<'a, bool>;
  /// Every node the service keeps, in the node list's order.
  fn node_rows(&self) -> Asking<'_, Vec<NodeRow>>;
  /// The arguments that open a batch SSH login to a node, ahead of the command, or none where the node
  /// has no SSH route.
  fn ssh_login<'a>(&'a self, node: &'a str) -> Asking<'a, Option<Vec<String>>>;
}

/// No node service deployed: this machine has room, no link stands, no node is kept and none is logged
/// in to.
pub struct NoNodeService;

impl NodeService for NoNodeService {
  fn machine_room(&self) -> Asking<'_, Room> {
    Box::pin(async { Ok(Room::default()) })
  }

  fn link_up<'a>(&'a self, _node: &'a str) -> Asking<'a, bool> {
    Box::pin(async { Ok(false) })
  }

  fn node_rows(&self) -> Asking<'_, Vec<NodeRow>> {
    Box::pin(async { Ok(vec![]) })
  }

  fn ssh_login<'a>(&'a self, _node: &'a str) -> Asking<'a, Option<Vec<String>>> {
    Box::pin(async { Ok(None) })
  }
}

/// The node service asked over HTTP.
pub struct HttpNodeService {
  caller: Caller,
}

impl HttpNodeService {
  pub fn new(address: PeerAddress) -> Self {
    Self {
      caller: Caller::new(PEER, address, ANSWER_WITHIN, Patience::NONE),
    }
  }
}

impl NodeService for HttpNodeService {
  fn machine_room(&self) -> Asking<'_, Room> {
    Box::pin(self.caller.post("machine/room", json!({})))
  }

  fn link_up<'a>(&'a self, node: &'a str) -> Asking<'a, bool> {
    Box::pin(async move {
      let state: LinkState = self.caller.post("links/state", json!({ "node": node })).await?;

      Ok(state.up)
    })
  }

  fn node_rows(&self) -> Asking<'_, Vec<NodeRow>> {
    Box::pin(async move {
      let rows: NodeRows = self.caller.post("nodes/list", json!({})).await?;

      Ok(rows.nodes)
    })
  }

  fn ssh_login<'a>(&'a self, node: &'a str) -> Asking<'a, Option<Vec<String>>> {
    Box::pin(async move {
      let login: Login = self.caller.post("nodes/login", json!({ "node": node })).await?;

      Ok(login.ssh)
    })
  }
}

/// The client the address chooses.
pub fn client(address: Option<PeerAddress>) -> Arc<dyn NodeService> {
  match address {
    Some(address) => Arc::new(HttpNodeService::new(address)),
    None => Arc::new(NoNodeService),
  }
}
