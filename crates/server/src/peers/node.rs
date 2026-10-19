//! The node service (docs/api.md, 「Calls agency makes」): this machine's room for another launch, what
//! each place on it holds, the pressures it tells, whether a node's link stands, the nodes it keeps and
//! the SSH login to each.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sse_stream::SseStream;

use super::{Asking, Caller, Patience, PeerAddress, PeerFault, Variables};

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

/// What one agent place on this machine holds: its memory and swap together, and its browsers.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlaceHolding {
  pub place: String,
  pub megabytes: f64,
  pub browsers: u64,
}

/// The part of this machine's standing agency reads: how many browsers run, and what each place holds.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Standing {
  #[serde(default)]
  pub browsers: u64,
  #[serde(default)]
  pub places: Vec<PlaceHolding>,
}

/// One of the largest processes on this machine, with the place it stands in where it names one.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct LargeProcess {
  pub pid: i64,
  pub name: String,
  pub megabytes: f64,
  #[serde(default)]
  pub place: Option<String>,
}

/// A pressure on this machine as the node service tells it, once.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "cause", rename_all = "lowercase")]
pub enum Pressure {
  /// Memory `full` at or above the alert line, standing for the alert minutes.
  #[serde(rename_all = "camelCase")]
  Memory {
    full_pct: f64,
    alert_pct: f64,
    minutes: f64,
    #[serde(default)]
    reading: Option<Value>,
    #[serde(default)]
    places: Vec<PlaceHolding>,
    #[serde(default)]
    largest: Vec<LargeProcess>,
  },
  /// The data root's disk under the launch line.
  #[serde(rename_all = "camelCase")]
  Disk {
    free_gb: f64,
    limit_gb: f64,
    #[serde(default)]
    reading: Option<Value>,
  },
}

/// The pressures the node service tells, as they come; a stream that drops ends.
pub type Pressures = BoxStream<'static, Result<Pressure, PeerFault>>;

/// The kind of event the node service tells a pressure as.
const PRESSURE_EVENT: &str = "machine-pressure";

/// How a node is reached: over SSH, or through a hub its agent attaches to, which offers no shell.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
  Ssh,
  Jake,
}

impl NodeKind {
  /// The word the node service writes it as.
  pub fn word(self) -> &'static str {
    match self {
      Self::Ssh => "ssh",
      Self::Jake => "jake",
    }
  }

  /// Whether a shell, and so a job, reaches the node over SSH.
  pub fn over_ssh(self) -> bool {
    self == Self::Ssh
  }
}

/// One node the node service keeps, with the label it is shown under, how it is reached, its state and
/// what it says of it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct NodeRow {
  pub node: String,
  #[serde(default)]
  pub label: Option<String>,
  pub kind: NodeKind,
  pub state: String,
  #[serde(default)]
  pub detail: Option<String>,
}

/// One node as the node service's reads answer it, handed on as it stands.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NodeRead {
  pub name: String,
  pub label: String,
  pub kind: NodeKind,
  #[serde(default)]
  pub role: Option<String>,
  pub state: String,
  #[serde(default)]
  pub checked_at: Option<i64>,
}

/// How SSH reaches an ssh node, as the node service's reads answer it.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SshReach {
  pub host: String,
  pub port: u16,
  pub user: String,
  #[serde(default)]
  pub key_path: Option<String>,
  #[serde(default)]
  pub shell: Option<String>,
}

// The node service's refusals that say a node has no SSH reach: none of that name, or one reached
// through a hub.
const NO_REACH: [&str; 2] = ["no-node", "not-ssh"];

#[derive(Deserialize)]
struct NodeReads {
  nodes: Vec<NodeRead>,
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
  /// The nodes as the service's reads answer them, in the node list's order; none by default.
  fn node_reads(&self) -> Asking<'_, Vec<NodeRead>> {
    Box::pin(async { Ok(vec![]) })
  }
  /// How SSH reaches a node, or none where it has no SSH reach or is not listed; none by default.
  fn ssh_reach<'a>(&'a self, _node: &'a str) -> Asking<'a, Option<SshReach>> {
    Box::pin(async { Ok(None) })
  }
  /// How many browsers run on this machine and what each place holds; none of either by default.
  fn machine_standing(&self) -> Asking<'_, Standing> {
    Box::pin(async { Ok(Standing::default()) })
  }
  /// The pressures the service tells from now on; by default a stream that tells none and never ends.
  fn pressures(&self) -> Asking<'_, Pressures> {
    Box::pin(async { Ok(futures_util::stream::pending().boxed()) })
  }
}

/// No node service deployed: this machine has room, no browser runs and no pressure is told, no link
/// stands, no node is kept and none is logged in to.
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

  fn node_reads(&self) -> Asking<'_, Vec<NodeRead>> {
    Box::pin(async move {
      let reads: NodeReads = self.caller.get("nodes").await?;

      Ok(reads.nodes)
    })
  }

  fn ssh_reach<'a>(&'a self, node: &'a str) -> Asking<'a, Option<SshReach>> {
    Box::pin(async move {
      match self.caller.get(&format!("nodes/{node}/reach")).await {
        Ok(reach) => Ok(Some(reach)),
        Err(PeerFault::Refused { code, .. }) if NO_REACH.contains(&code.as_str()) => Ok(None),
        Err(fault) => Err(fault),
      }
    })
  }

  fn machine_standing(&self) -> Asking<'_, Standing> {
    Box::pin(self.caller.get("machine/standing"))
  }

  fn pressures(&self) -> Asking<'_, Pressures> {
    Box::pin(async move {
      let opened = self.caller.open("events", &[("kinds", PRESSURE_EVENT)]).await?;
      let told = SseStream::from_bytes_stream(opened.bytes_stream()).filter_map(|read| async move {
        match read {
          Err(why) => Some(Err(PeerFault::Unreachable {
            peer: PEER,
            detail: why.to_string(),
          })),
          Ok(event) if event.event.as_deref() != Some(PRESSURE_EVENT) => None,
          Ok(event) => Some(serde_json::from_str(event.data.as_deref().unwrap_or("null")).map_err(|why| {
            PeerFault::Malformed {
              peer: PEER,
              detail: format!("the {PRESSURE_EVENT} event's data: {why}"),
            }
          })),
        }
      });

      Ok(told.boxed())
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
