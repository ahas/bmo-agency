//! What Claude Code declares (docs/provider-presence.md).

use std::collections::BTreeMap;

use serde::Deserialize;

use super::Declared;

#[derive(Debug, Deserialize)]
pub struct ClaudeCode {
  #[serde(flatten)]
  pub declared: Declared,
  pub catalogue: Catalogue,
  pub launch: Launch,
  pub cache: Cache,
  pub login: Login,
  pub node_login: NodeLogin,
}

/// Each account's token for the work nodes: where this machine keeps it, the verb that makes one, and
/// how one that stopped answering, or is about to, is told to the owner.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeLogin {
  /// The directory under the data root's env directory, one file in it per account, named by its id.
  pub token_dir: String,
  pub command: String,
  pub head: String,
  pub gone: String,
  pub missing: String,
  pub ending_in: String,
  pub after: String,
  pub gone_after: String,
}

/// How a login's end is told to the owner: how long before it, the year a token stands for, and the
/// message, one entry to a line, with its blanks named in braces.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
  pub notice_ahead_hours: i64,
  pub token_life_days: i64,
  pub head: String,
  pub gone: String,
  pub ending: String,
  pub ending_in: String,
  pub minutes: String,
  pub hours: String,
  pub run: String,
  pub token_command: String,
  pub directory_command: String,
  pub token_after: String,
  pub directory_after: String,
  pub gone_after: String,
  pub ending_after: String,
}

/// How long a prefix it holds stays cached, and how long an agent's session may stand idle before it
/// is warmed again.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cache {
  pub ttl_minutes: i64,
  pub keepalive_idle_minutes: i64,
}

/// What every line of it is started with beside its words.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Launch {
  /// The variables every line sets for its part.
  pub variables: BTreeMap<String, String>,
  /// The variables the program reads a login off.
  pub login_variables: Vec<String>,
  /// What the program writes itself into a session's conversation.
  pub own_words: Vec<String>,
  /// The program's tools whose calls wait long.
  pub long_tools: Vec<String>,
}

/// Where under a login's configuration directory the CLI keeps its model catalogue.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
  pub dir: String,
  /// The ending of every file of the surface agency runs on.
  pub suffix: String,
}

crate::part!(pub CLAUDE_CODE: ClaudeCode = "providers/claude-code.toml");
