//! What DeepSeek declares (docs/provider-presence.md): its own API, reached by Claude Code through agency's
//! gateway member.

use serde::Deserialize;

use super::Declared;

#[derive(Debug, Deserialize)]
pub struct DeepSeek {
  #[serde(flatten)]
  pub declared: Declared,
  pub presence: Presence,
  pub balance: super::Balance,
  /// The Claude Code effort word a run of each DeepSeek word goes out with.
  #[serde(default)]
  pub claude: Option<super::Claude>,
}

/// The road's key file, and what the provider says where it cannot serve a run.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
  pub key_file: String,
  pub missing_code: String,
  pub missing: String,
  pub not_held: String,
}

crate::part!(pub DEEPSEEK: DeepSeek = "providers/deepseek.toml");
