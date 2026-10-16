//! The cache siren (docs/api.md, 「Calls agency makes」): an account's usage windows, and how long a
//! reading of them stands.

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use super::{Asking, Caller, Patience, PeerAddress, Variables};

/// Where the deployment names the cache siren.
pub const ADDRESS: Variables = Variables {
  url: "AGENCY_CACHE_SIREN_URL",
  token: "AGENCY_CACHE_SIREN_TOKEN",
};

const PEER: &str = "cache siren";

// A reading asked on a clock or for a judgement in hand is not waited on for long.
const ANSWER_WITHIN: Duration = Duration::from_secs(15);

/// One window of an account's limit: the fraction used, when it resets and when it was read in epoch
/// seconds, and how long it runs and what its provider calls it where the provider says.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Window {
  pub name: String,
  pub utilization: f64,
  #[serde(default)]
  pub resets_at: Option<i64>,
  pub seen_at: i64,
  #[serde(default)]
  pub duration_minutes: Option<i64>,
  #[serde(default)]
  pub limit_name: Option<String>,
}

/// An account's windows, with how long a reading stands after it was read.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindows {
  pub windows: Vec<Window>,
  pub fresh_seconds: u64,
}

impl UsageWindows {
  /// How long a reading stands: none stands where no reader keeps the windows.
  pub fn fresh(&self) -> Duration {
    Duration::from_secs(self.fresh_seconds)
  }
}

/// What agency asks the cache siren.
pub trait CacheSiren: Send + Sync {
  /// One account's windows under one provider.
  fn usage_windows<'a>(&'a self, mode: &'a str, account: &'a str) -> Asking<'a, UsageWindows>;
}

/// No cache siren deployed: no windows, and no reading fresh.
pub struct NoCacheSiren;

impl CacheSiren for NoCacheSiren {
  fn usage_windows<'a>(&'a self, _mode: &'a str, _account: &'a str) -> Asking<'a, UsageWindows> {
    Box::pin(async { Ok(UsageWindows::default()) })
  }
}

/// The cache siren asked over HTTP.
pub struct HttpCacheSiren {
  caller: Caller,
}

impl HttpCacheSiren {
  pub fn new(address: PeerAddress) -> Self {
    Self {
      caller: Caller::new(PEER, address, ANSWER_WITHIN, Patience::NONE),
    }
  }
}

impl CacheSiren for HttpCacheSiren {
  fn usage_windows<'a>(&'a self, mode: &'a str, account: &'a str) -> Asking<'a, UsageWindows> {
    Box::pin(self.caller.post("usage/windows", json!({ "mode": mode, "account": account })))
  }
}

/// The client the address chooses.
pub fn client(address: Option<PeerAddress>) -> Arc<dyn CacheSiren> {
  match address {
    Some(address) => Arc::new(HttpCacheSiren::new(address)),
    None => Arc::new(NoCacheSiren),
  }
}
