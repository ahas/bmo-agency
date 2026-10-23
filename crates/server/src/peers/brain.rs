//! The brain (docs/api.md, 「Calls agency makes」) through the client it publishes: the brain a companion is
//! given as it is added, and what its reset and removal take of what it remembers. Everything an agent
//! remembers is its own brain, found by the agent's key, so agency lets that brain go whole; a preview only
//! finds it.

use std::sync::Arc;
use std::time::Duration;

use bmo_brain_client::{Brain as BrainMember, BrainClient, BrainsQuery, ClientFailure, NewBrain};
use serde::Deserialize;
use serde_json::Value;

use super::{Asking, PeerAddress, PeerFault, Variables};

/// Where the deployment names the brain.
pub const ADDRESS: Variables = Variables {
  url: "AGENCY_BRAIN_URL",
  token: "AGENCY_BRAIN_TOKEN",
};

/// The variable naming the account key a brain agency makes is filed under.
pub const ACCOUNT: &str = "AGENCY_BRAIN_ACCOUNT";

const PEER: &str = "brain";

// A reset lets a whole brain go, which may hold many rows.
const ANSWER_WITHIN: Duration = Duration::from_secs(60);

/// What a reset of an agent took of the brain, or would take: the brains let go, counted, and what it could
/// not do.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BrainReset {
  #[serde(default)]
  pub took: Value,
  #[serde(default)]
  pub could_not: Vec<String>,
}

/// What became of the ask to make an agent's brain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Made {
  /// The brain made one.
  Made,
  /// The brain already kept one under the agent's key, so nothing was made.
  Kept,
  /// No brain is deployed, so none was asked for.
  Undeployed,
}

/// What agency asks the brain.
pub trait Brain: Send + Sync {
  /// The agent's own brain made under its key, named as given, where the brain keeps none yet.
  fn make<'a>(&'a self, agent: &'a str, name: &'a str) -> Asking<'a, Made>;
  /// The agent's brain let go whole, once nothing writes there.
  fn forget<'a>(&'a self, agent: &'a str) -> Asking<'a, ()>;
  /// The agent's brain taken whole or, for a preview, found and counted with nothing taken.
  fn reset<'a>(&'a self, agent: &'a str, preview: bool) -> Asking<'a, BrainReset>;
  /// The discipline a chat agent's system prompt carries: how it reads and writes its brain every turn.
  fn discipline(&self) -> Asking<'_, String>;
}

/// No brain deployed: no agent has a brain, so nothing is counted or taken.
pub struct NoBrain;

impl Brain for NoBrain {
  fn make<'a>(&'a self, _agent: &'a str, _name: &'a str) -> Asking<'a, Made> {
    Box::pin(async { Ok(Made::Undeployed) })
  }

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

/// The brain asked through its published client, and the account key a brain agency makes is filed under
/// where the deployment names one.
pub struct PublishedBrain {
  client: BrainClient,
  account: Option<String>,
}

impl PublishedBrain {
  pub fn new(address: PeerAddress, account: Option<String>) -> Result<Self, String> {
    let http = reqwest::Client::builder().timeout(ANSWER_WITHIN).build().unwrap_or_default();
    let client = BrainClient::new(address.url.as_str(), address.token.unwrap_or_default())?.with_http(http);

    Ok(Self { client, account })
  }

  /// The agent's brain made where none is kept under its key: refused where the deployment names no
  /// account to file it under.
  async fn made(&self, agent: &str, name: &str) -> Result<Made, PeerFault> {
    if self.find(agent).await?.is_some() {
      return Ok(Made::Kept);
    }

    let Some(account) = &self.account else {
      return Err(PeerFault::Refused {
        peer: PEER,
        code: "no-account".into(),
        message: format!("{ACCOUNT} names no account key to file the brain of {agent} under"),
      });
    };
    let asked = NewBrain {
      account_key: account.clone(),
      agent_key: agent.to_owned(),
      name: Some(name.to_owned()).filter(|name| !name.is_empty()),
    };

    self.client.brain_add(&asked).await.map_err(fault)?;

    Ok(Made::Made)
  }

  /// The one brain registered for the agent's key, where one is.
  async fn find(&self, agent: &str) -> Result<Option<BrainMember>, PeerFault> {
    let query = BrainsQuery {
      agent_key: Some(agent.to_owned()),
      limit: Some(1),
      ..BrainsQuery::default()
    };
    let page = self.client.brains(&query).await.map_err(fault)?;

    Ok(page.items.into_iter().next())
  }

  /// The agent's brain found and, where asked, let go: how many brains went, none where it has none.
  async fn let_go(&self, agent: &str, preview: bool) -> Result<BrainReset, PeerFault> {
    let Some(brain) = self.find(agent).await? else {
      return Ok(BrainReset::default());
    };

    if !preview {
      match self.client.brain_let_go(brain.id).await.map_err(fault) {
        Ok(_) | Err(PeerFault::Failed { status: 404, .. }) => {}
        Err(why) => return Err(why),
      }
    }

    Ok(BrainReset {
      took: serde_json::json!({ "brains": 1 }),
      could_not: vec![],
    })
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

impl Brain for PublishedBrain {
  fn make<'a>(&'a self, agent: &'a str, name: &'a str) -> Asking<'a, Made> {
    Box::pin(async move { self.made(agent, name).await })
  }

  fn forget<'a>(&'a self, agent: &'a str) -> Asking<'a, ()> {
    Box::pin(async move { self.let_go(agent, false).await.map(|_| ()) })
  }

  fn reset<'a>(&'a self, agent: &'a str, preview: bool) -> Asking<'a, BrainReset> {
    Box::pin(async move { self.let_go(agent, preview).await })
  }

  fn discipline(&self) -> Asking<'_, String> {
    Box::pin(async move { self.client.discipline().await.map_err(fault) })
  }
}

/// The client the address chooses, filing a brain it makes under the account named.
pub fn client(address: Option<PeerAddress>, account: Option<String>) -> Arc<dyn Brain> {
  let Some(address) = address else {
    return Arc::new(NoBrain);
  };

  match PublishedBrain::new(address, account) {
    Ok(brain) => Arc::new(brain),
    Err(why) => {
      eprintln!("agency: the brain's address did not read: {why}");

      Arc::new(NoBrain)
    }
  }
}
