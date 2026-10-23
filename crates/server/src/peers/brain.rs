//! The brain (docs/api.md, 「Calls agency makes」) through the client it publishes: what a companion's reset
//! and removal take of what it remembers. Everything the brain holds is held under the caller's scope, so
//! agency names the companion as the scope and lets the whole of it go, counted first for a preview.

use std::sync::Arc;
use std::time::Duration;

use bmo_brain_client::{BrainClient, ClientFailure, ScopeHolding};
use serde::Deserialize;
use serde_json::Value;

use super::{Asking, PeerAddress, PeerFault, Variables};

/// Where the deployment names the brain.
pub const ADDRESS: Variables = Variables {
  url: "AGENCY_BRAIN_URL",
  token: "AGENCY_BRAIN_TOKEN",
};

const PEER: &str = "brain";

// A reset lets a whole scope go, which may hold many rows.
const ANSWER_WITHIN: Duration = Duration::from_secs(60);

/// What a reset of an agent took of the brain, or would take: the count of the agent's rows per kind, and
/// what it could not do.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrainReset {
  #[serde(default)]
  pub took: Value,
  #[serde(default)]
  pub could_not: Vec<String>,
}

/// What agency asks the brain.
pub trait Brain: Send + Sync {
  /// Everything the brain holds under the agent's scope let go, once nothing writes there.
  fn forget<'a>(&'a self, agent: &'a str) -> Asking<'a, ()>;
  /// What the agent's scope holds taken — its rows of every kind — or, for a preview, counted with nothing
  /// taken.
  fn reset<'a>(&'a self, agent: &'a str, preview: bool) -> Asking<'a, BrainReset>;
  /// The discipline a chat agent's system prompt carries: how it reads and writes its brain every turn.
  fn discipline(&self) -> Asking<'_, String>;
}

/// No brain deployed: nothing is held under any scope, so nothing is counted or taken.
pub struct NoBrain;

impl Brain for NoBrain {
  fn forget<'a>(&'a self, _agent: &'a str) -> Asking<'a, ()> {
    Box::pin(async { Ok(()) })
  }

  fn reset<'a>(&'a self, _agent: &'a str, _preview: bool) -> Asking<'a, BrainReset> {
    Box::pin(async { Ok(BrainReset::default()) })
  }

  fn discipline(&self) -> Asking<'_, String> {
    Box::pin(async { Ok(String::new()) })
  }
}

/// The brain asked through its published client.
pub struct PublishedBrain {
  client: BrainClient,
}

impl PublishedBrain {
  pub fn new(address: PeerAddress) -> Result<Self, String> {
    let http = reqwest::Client::builder().timeout(ANSWER_WITHIN).build().unwrap_or_default();
    let client = BrainClient::new(address.url.as_str(), address.token.unwrap_or_default())?.with_http(http);

    Ok(Self { client })
  }

  async fn let_go(&self, agent: &str, preview: bool) -> Result<ScopeHolding, PeerFault> {
    self.client.scope_let_go(agent, preview).await.map_err(fault)
  }
}

fn fault(why: ClientFailure) -> PeerFault {
  match why {
    ClientFailure::Answered {
      status,
      answer: Some(error),
    } => PeerFault::Failed {
      peer: PEER,
      status,
      code: error.code,
      error: error.error,
    },
    ClientFailure::Answered {
      status,
      answer: None,
    } => PeerFault::Failed {
      peer: PEER,
      status,
      code: String::new(),
      error: String::new(),
    },
    ClientFailure::Unreachable(detail) => PeerFault::Unreachable { peer: PEER, detail },
    ClientFailure::Unreadable(detail) => PeerFault::Malformed { peer: PEER, detail },
  }
}

fn taken(holding: ScopeHolding) -> BrainReset {
  BrainReset {
    took: serde_json::to_value(holding.counts).unwrap_or(Value::Null),
    could_not: vec![],
  }
}

impl Brain for PublishedBrain {
  fn forget<'a>(&'a self, agent: &'a str) -> Asking<'a, ()> {
    Box::pin(async move {
      match self.let_go(agent, false).await {
        Ok(_) => Ok(()),
        Err(PeerFault::Failed { status: 404, .. }) => Ok(()),
        Err(fault) => Err(fault),
      }
    })
  }

  fn reset<'a>(&'a self, agent: &'a str, preview: bool) -> Asking<'a, BrainReset> {
    Box::pin(async move {
      match self.let_go(agent, preview).await {
        Ok(holding) => Ok(taken(holding)),
        Err(PeerFault::Failed { status: 404, .. }) => Ok(BrainReset::default()),
        Err(fault) => Err(fault),
      }
    })
  }

  fn discipline(&self) -> Asking<'_, String> {
    Box::pin(async move { self.client.discipline().await.map_err(fault) })
  }
}

/// The client the address chooses.
pub fn client(address: Option<PeerAddress>) -> Arc<dyn Brain> {
  let Some(address) = address else {
    return Arc::new(NoBrain);
  };

  match PublishedBrain::new(address) {
    Ok(brain) => Arc::new(brain),
    Err(why) => {
      eprintln!("agency: the brain's address did not read: {why}");

      Arc::new(NoBrain)
    }
  }
}
