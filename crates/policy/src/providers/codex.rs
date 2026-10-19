//! What Codex declares (docs/provider-presence.md).

use serde::Deserialize;

use super::Declared;

#[derive(Debug, Deserialize)]
pub struct Codex {
  #[serde(flatten)]
  pub declared: Declared,
  pub presence: Presence,
  pub catalogue: Catalogue,
  pub cache: Cache,
  pub usage: Usage,
}

/// How the subscription's windows are read on a clock (docs/usage-windows.md, 「The Codex reader」).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
  pub pass_seconds: u64,
  pub fresh_passes: u32,
  pub rollout_fresh_seconds: i64,
  pub rollout_days: usize,
  pub ask_seconds: u64,
  pub stderr_seconds: u64,
  pub app_server_args: Vec<String>,
}

/// How long a prefix its models hold stays cached, by the model names they begin.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cache {
  pub models: Vec<ModelCache>,
}

/// One entry of the cache's lifetimes: the start of the model names it covers, the minutes a prefix
/// stays cached and the minutes a thread may stand idle before a ping warms it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCache {
  pub prefix: String,
  pub ttl_minutes: i64,
  pub keepalive_idle_minutes: i64,
}

impl Cache {
  /// The lifetime a model is given: the entry with the longest start the model begins with, and none
  /// where no entry's start begins it.
  pub fn of(&self, model: &str) -> Option<&ModelCache> {
    self
      .models
      .iter()
      .filter(|entry| model.starts_with(&entry.prefix))
      .max_by_key(|entry| entry.prefix.len())
  }
}

/// The executable this machine holds for it, looked up on the asking process's PATH.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
  pub program: String,
  pub missing: String,
}

/// How the catalogue bundled with the executable is asked for, and how long it is given.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
  pub args: Vec<String>,
  pub seconds: u64,
}

crate::part!(pub CODEX: Codex = "providers/codex.toml");
