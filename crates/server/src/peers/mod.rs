//! The services agency calls (docs/api.md, 「Calls agency makes」): the cache siren, the cache warmer, the
//! node service and the brain. Each is a client trait of its own, answered over HTTP where the deployment
//! names the service's address and by a default that calls nothing where it names none. Every call is one
//! POST of a JSON body, read by the chat wire protocol's conventions.

pub mod brain;
pub mod cache_siren;
pub mod cache_warmer;
pub mod node;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

use self::brain::Brain;
use self::cache_siren::CacheSiren;
use self::cache_warmer::CacheWarmer;
use self::node::NodeService;
use crate::config::PeerAddresses;

/// A call's answer as a client hands it back.
pub type Asking<'a, T> = Pin<Box<dyn Future<Output = Result<T, PeerFault>> + Send + 'a>>;

/// The variables a service's address and its token are named in.
#[derive(Clone, Copy, Debug)]
pub struct Variables {
  pub url: &'static str,
  pub token: &'static str,
}

/// Where a service is reached, and the token it is called with where one is named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerAddress {
  pub url: Url,
  pub token: Option<String>,
}

impl PeerAddress {
  /// The address `variables` name through `variable`: none where the address is unset or blank, and a
  /// fault naming the variable where it does not read as an HTTP URL.
  pub fn read(
    variables: &Variables,
    variable: impl Fn(&str) -> Option<String>,
  ) -> Result<Option<Self>, String> {
    let set =
      |name: &str| variable(name).map(|value| value.trim().to_owned()).filter(|value| !value.is_empty());
    let Some(named) = set(variables.url) else {
      return Ok(None);
    };
    let url =
      Url::parse(&named).map_err(|why| format!("{}: {named} is not an address: {why}", variables.url))?;

    if !matches!(url.scheme(), "http" | "https") {
      return Err(format!("{}: {named} is not an HTTP address", variables.url));
    }

    Ok(Some(Self {
      url,
      token: set(variables.token),
    }))
  }
}

/// How long a call is offered again while the service refuses the connection, and how often.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patience {
  pub within: Duration,
  pub every: Duration,
}

impl Patience {
  /// A refused connection is a fault at once.
  pub const NONE: Self = Self {
    within: Duration::ZERO,
    every: Duration::ZERO,
  };
}

/// Why a call came back without its answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeerFault {
  /// The service was not reached, or its answer did not arrive.
  Unreachable { peer: &'static str, detail: String },
  /// The service understood the call and would not carry it out.
  Refused {
    peer: &'static str,
    code: String,
    message: String,
  },
  /// The service answered with a status that is not a success.
  Failed {
    peer: &'static str,
    status: u16,
    code: String,
    error: String,
  },
  /// The service answered in a shape the call does not read.
  Malformed { peer: &'static str, detail: String },
}

impl std::fmt::Display for PeerFault {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Unreachable { peer, detail } => write!(out, "the {peer} could not be reached: {detail}"),
      Self::Refused {
        peer,
        code,
        message,
      } => write!(out, "the {peer} refused: {message} ({code})"),
      Self::Failed {
        peer,
        status,
        code,
        error,
      } => write!(out, "the {peer} failed with {status} {code}: {error}"),
      Self::Malformed { peer, detail } => write!(out, "the {peer} answered out of shape: {detail}"),
    }
  }
}

impl std::error::Error for PeerFault {}

// An error with every cause under it, since the transport's own words name only the outermost.
fn said(error: &dyn std::error::Error) -> String {
  let mut words = error.to_string();
  let mut cause = error.source();

  while let Some(under) = cause {
    words.push_str(": ");
    words.push_str(&under.to_string());
    cause = under.source();
  }

  words
}

fn text_of(value: &Value, field: &str) -> Option<String> {
  value.get(field).and_then(Value::as_str).map(str::to_owned)
}

/// One service's caller: its address, its token, how long an answer is waited for, and its patience.
#[derive(Clone, Debug)]
pub struct Caller {
  peer: &'static str,
  address: PeerAddress,
  client: reqwest::Client,
  patience: Patience,
}

impl Caller {
  pub fn new(
    peer: &'static str,
    address: PeerAddress,
    answer_within: Duration,
    patience: Patience,
  ) -> Self {
    let client = reqwest::Client::builder().timeout(answer_within).build().unwrap_or_default();

    Self {
      peer,
      address,
      client,
      patience,
    }
  }

  /// The address of one of the service's paths, under its base address's own path.
  pub fn url(&self, path: &str) -> Url {
    let mut url = self.address.url.clone();
    let joined = format!("{}/{}", url.path().trim_end_matches('/'), path.trim_start_matches('/'));

    url.set_path(&joined);
    url
  }

  fn unreachable(&self, detail: String) -> PeerFault {
    PeerFault::Unreachable {
      peer: self.peer,
      detail,
    }
  }

  /// The service asked at `path` with `body`, its answer read as `T`. A refused connection is offered
  /// again within the service's patience; a call that reached a listener never is.
  pub async fn post<T: DeserializeOwned>(&self, path: &str, body: Value) -> Result<T, PeerFault> {
    let url = self.url(path);
    let began = Instant::now();
    let response = loop {
      let mut request = self.client.post(url.clone()).json(&body);

      if let Some(token) = &self.address.token {
        request = request.bearer_auth(token);
      }

      match request.send().await {
        Ok(response) => break response,
        Err(error)
          if error.is_connect() && began.elapsed() + self.patience.every < self.patience.within =>
        {
          tokio::time::sleep(self.patience.every).await;
        }
        Err(error) => return Err(self.unreachable(said(&error))),
      }
    };
    let status = response.status();
    let text = response.text().await.map_err(|error| self.unreachable(said(&error)))?;
    let answer = serde_json::from_str::<Value>(&text);

    if !status.is_success() {
      let answer = answer.unwrap_or(Value::Null);

      return Err(PeerFault::Failed {
        peer: self.peer,
        status: status.as_u16(),
        code: text_of(&answer, "code").unwrap_or_default(),
        error: text_of(&answer, "error").unwrap_or_else(|| text.trim().to_owned()),
      });
    }

    let answer = answer.map_err(|error| PeerFault::Malformed {
      peer: self.peer,
      detail: error.to_string(),
    })?;

    if answer.get("answered") == Some(&Value::Bool(false)) {
      let refusal = answer.get("refusal").cloned().unwrap_or(Value::Null);

      return Err(PeerFault::Refused {
        peer: self.peer,
        code: text_of(&refusal, "code").unwrap_or_default(),
        message: text_of(&refusal, "message").unwrap_or_default(),
      });
    }

    serde_json::from_value(answer).map_err(|error| PeerFault::Malformed {
      peer: self.peer,
      detail: error.to_string(),
    })
  }
}

/// The services agency calls, each by the client its address chose.
#[derive(Clone)]
pub struct Peers {
  pub cache_siren: Arc<dyn CacheSiren>,
  pub cache_warmer: Arc<dyn CacheWarmer>,
  pub node: Arc<dyn NodeService>,
  pub brain: Arc<dyn Brain>,
}

impl Peers {
  /// Each service called over HTTP where its address is named, and answered by its default where not.
  pub fn of(addresses: &PeerAddresses) -> Self {
    Self {
      cache_siren: cache_siren::client(addresses.cache_siren.clone()),
      cache_warmer: cache_warmer::client(addresses.cache_warmer.clone()),
      node: node::client(addresses.node.clone()),
      brain: brain::client(addresses.brain.clone()),
    }
  }
}
