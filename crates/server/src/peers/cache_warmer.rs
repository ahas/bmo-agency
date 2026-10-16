//! The cache warmer (docs/api.md, 「Calls agency makes」): the keepalive schedule, asked whether pings of a
//! kind go on a provider now. Which places are due, the ping itself and its record are agency's.

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use super::{Asking, Caller, Patience, PeerAddress, Variables};

/// Where the deployment names the cache warmer.
pub const ADDRESS: Variables = Variables {
  url: "AGENCY_CACHE_WARMER_URL",
  token: "AGENCY_CACHE_WARMER_TOKEN",
};

/// What the co-worker pool's reserved places are warmed as.
pub const COWORKER_POOL: &str = "coworker-pool";

const PEER: &str = "cache warmer";

// A schedule asked on a pass is not waited on for long.
const ANSWER_WITHIN: Duration = Duration::from_secs(15);

#[derive(Deserialize)]
struct Schedule {
  go: bool,
}

/// What agency asks the cache warmer.
pub trait CacheWarmer: Send + Sync {
  /// Whether keepalive pings of `kind` on the provider `mode` go now.
  fn pings_go<'a>(&'a self, kind: &'a str, mode: &'a str) -> Asking<'a, bool>;
}

/// No cache warmer deployed: no ping goes.
pub struct NoCacheWarmer;

impl CacheWarmer for NoCacheWarmer {
  fn pings_go<'a>(&'a self, _kind: &'a str, _mode: &'a str) -> Asking<'a, bool> {
    Box::pin(async { Ok(false) })
  }
}

/// The cache warmer asked over HTTP.
pub struct HttpCacheWarmer {
  caller: Caller,
}

impl HttpCacheWarmer {
  pub fn new(address: PeerAddress) -> Self {
    Self {
      caller: Caller::new(PEER, address, ANSWER_WITHIN, Patience::NONE),
    }
  }
}

impl CacheWarmer for HttpCacheWarmer {
  fn pings_go<'a>(&'a self, kind: &'a str, mode: &'a str) -> Asking<'a, bool> {
    Box::pin(async move {
      let schedule: Schedule =
        self.caller.post("pings/schedule", json!({ "kind": kind, "mode": mode })).await?;

      Ok(schedule.go)
    })
  }
}

/// The client the address chooses.
pub fn client(address: Option<PeerAddress>) -> Arc<dyn CacheWarmer> {
  match address {
    Some(address) => Arc::new(HttpCacheWarmer::new(address)),
    None => Arc::new(NoCacheWarmer),
  }
}
