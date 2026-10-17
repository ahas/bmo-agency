//! The brain (docs/api.md, 「Calls agency makes」): what a companion's reset and removal do to the agent's
//! brain folder and the rows it remembers by. The folder, its editing lock with the lock's queue and fence,
//! the day a person book starts reading and the recollectors' writing are the brain's; agency names the
//! agent and the brain finds its folder. And a watch whose unit agency is about to stop, which the brain
//! ends so its clock does not launch it again.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::json;

use super::{Asking, Caller, Patience, PeerAddress, Variables};

/// Where the deployment names the brain.
pub const ADDRESS: Variables = Variables {
  url: "AGENCY_BRAIN_URL",
  token: "AGENCY_BRAIN_TOKEN",
};

const PEER: &str = "brain";

// A removal under the folder's lock is waited on longer than a reading.
const ANSWER_WITHIN: Duration = Duration::from_secs(60);

// The brain's contract: a refused connection is offered again every two seconds for two minutes.
const PATIENCE: Patience = Patience {
  within: Duration::from_secs(120),
  every: Duration::from_secs(2),
};

#[derive(Deserialize)]
struct LockPath {
  path: Option<String>,
}

#[derive(Deserialize)]
struct WatchEnded {
  ended: bool,
}

// What the brain names a watch's unit: its prefix, then the watch's number and the moment it was armed.
const WATCH_UNIT: &str = "bmo-watch-";

/// Whether a unit is one the brain runs a watch in, `bmo-watch-<id>-<milliseconds>`.
pub fn holds_watch(unit: &str) -> bool {
  let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());

  unit
    .strip_prefix(WATCH_UNIT)
    .and_then(|rest| rest.split_once('-'))
    .is_some_and(|(id, at)| digits(id) && digits(at))
}

/// What a reset of an agent took of the brain, or would take: the count of the agent's rows per table
/// and whether its folder stands, as the brain says them, and what it could not do.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrainReset {
  #[serde(default)]
  pub took: serde_json::Value,
  #[serde(default)]
  pub could_not: Vec<String>,
}

/// What agency asks the brain.
pub trait Brain: Send + Sync {
  /// The agent's folder fenced now: no writer begun before this moment takes its editing lock again.
  fn fence<'a>(&'a self, agent: &'a str) -> Asking<'a, ()>;
  /// The agent's editing lock, its ticket queue and its fence removed, once nothing writes its folder.
  fn forget<'a>(&'a self, agent: &'a str) -> Asking<'a, ()>;
  /// The agent's folder's editing lock, relative to the data root, or none where it keeps no folder.
  fn lock_path<'a>(&'a self, agent: &'a str) -> Asking<'a, Option<PathBuf>>;
  /// The agent's person book of a room read from `at`, in epoch seconds, on.
  fn book_since<'a>(&'a self, agent: &'a str, chat_id: i64, at: i64) -> Asking<'a, ()>;
  /// What the agent's stopped recollector runs left unwritten in its folder undone.
  fn put_back<'a>(&'a self, agent: &'a str) -> Asking<'a, ()>;
  /// What the agent remembers taken — its rows and its folder, under its fence and lock — or, for a
  /// preview, counted with nothing taken.
  fn reset<'a>(&'a self, agent: &'a str, preview: bool) -> Asking<'a, BrainReset>;
  /// The standing watch a unit about to be stopped holds ended as cancelled; whether it held one.
  fn watch_stopped<'a>(&'a self, unit: &'a str) -> Asking<'a, bool>;
}

/// No brain deployed: no folder is kept, so nothing is fenced, removed, written or undone, no lock stands
/// and no watch is ended.
pub struct NoBrain;

impl Brain for NoBrain {
  fn fence<'a>(&'a self, _agent: &'a str) -> Asking<'a, ()> {
    Box::pin(async { Ok(()) })
  }

  fn forget<'a>(&'a self, _agent: &'a str) -> Asking<'a, ()> {
    Box::pin(async { Ok(()) })
  }

  fn lock_path<'a>(&'a self, _agent: &'a str) -> Asking<'a, Option<PathBuf>> {
    Box::pin(async { Ok(None) })
  }

  fn book_since<'a>(&'a self, _agent: &'a str, _chat_id: i64, _at: i64) -> Asking<'a, ()> {
    Box::pin(async { Ok(()) })
  }

  fn put_back<'a>(&'a self, _agent: &'a str) -> Asking<'a, ()> {
    Box::pin(async { Ok(()) })
  }

  fn reset<'a>(&'a self, _agent: &'a str, _preview: bool) -> Asking<'a, BrainReset> {
    Box::pin(async { Ok(BrainReset::default()) })
  }

  fn watch_stopped<'a>(&'a self, _unit: &'a str) -> Asking<'a, bool> {
    Box::pin(async { Ok(false) })
  }
}

/// The brain asked over HTTP.
pub struct HttpBrain {
  caller: Caller,
}

impl HttpBrain {
  pub fn new(address: PeerAddress) -> Self {
    Self {
      caller: Caller::new(PEER, address, ANSWER_WITHIN, PATIENCE),
    }
  }

  async fn done(&self, path: &str, body: serde_json::Value) -> Result<(), super::PeerFault> {
    self.caller.post::<IgnoredAny>(path, body).await.map(drop)
  }
}

impl Brain for HttpBrain {
  fn fence<'a>(&'a self, agent: &'a str) -> Asking<'a, ()> {
    Box::pin(self.done("agents/fence", json!({ "agent": agent })))
  }

  fn forget<'a>(&'a self, agent: &'a str) -> Asking<'a, ()> {
    Box::pin(self.done("agents/removed", json!({ "agent": agent })))
  }

  fn lock_path<'a>(&'a self, agent: &'a str) -> Asking<'a, Option<PathBuf>> {
    Box::pin(async move {
      let answer: LockPath = self.caller.post("agents/lock", json!({ "agent": agent })).await?;

      Ok(answer.path.map(PathBuf::from))
    })
  }

  fn book_since<'a>(&'a self, agent: &'a str, chat_id: i64, at: i64) -> Asking<'a, ()> {
    Box::pin(self.done("agents/book-since", json!({ "agent": agent, "chatId": chat_id, "at": at })))
  }

  fn put_back<'a>(&'a self, agent: &'a str) -> Asking<'a, ()> {
    Box::pin(self.done("agents/put-back", json!({ "agent": agent })))
  }

  fn reset<'a>(&'a self, agent: &'a str, preview: bool) -> Asking<'a, BrainReset> {
    Box::pin(self.caller.post("agents/reset", json!({ "agent": agent, "preview": preview })))
  }

  fn watch_stopped<'a>(&'a self, unit: &'a str) -> Asking<'a, bool> {
    Box::pin(async move {
      let answer: WatchEnded = self.caller.post("watches/stopped", json!({ "unit": unit })).await?;

      Ok(answer.ended)
    })
  }
}

/// The client the address chooses.
pub fn client(address: Option<PeerAddress>) -> Arc<dyn Brain> {
  match address {
    Some(address) => Arc::new(HttpBrain::new(address)),
    None => Arc::new(NoBrain),
  }
}
