//! What the Anthropic API declares (docs/provider-presence.md).

use serde::Deserialize;

use super::Declared;

#[derive(Debug, Deserialize)]
pub struct Anthropic {
  #[serde(flatten)]
  pub declared: Declared,
  pub presence: Presence,
  /// The Claude Code effort word a run of each word goes out with.
  #[serde(default)]
  pub claude: Option<super::Claude>,
}

/// The key file its member names, under the data root's env directory.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
  pub key_file: String,
  pub missing_code: String,
  pub missing: String,
  pub not_held: String,
}

crate::part!(pub ANTHROPIC: Anthropic = "providers/anthropic.toml");
