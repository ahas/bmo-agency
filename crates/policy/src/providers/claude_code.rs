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
  pub console_login: ConsoleLogin,
  pub usage: Usage,
}

/// How every account's own windows are read on a clock (docs/usage-windows.md, 「The Claude account
/// reader」).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
  pub url: String,
  pub pass_seconds: u64,
  pub fresh_passes: u32,
  /// How long an account the endpoint refused for asking too often is left unasked, in seconds, where the
  /// refusal names no moment of its own.
  pub refused_pause_seconds: u64,
  /// The model and effort of the run of the CLI that renews a directory account's login found expired or
  /// refused, and the words that are its turn.
  pub renewal_model: String,
  pub renewal_effort: String,
  pub renewal_words: String,
  /// How long, in seconds, an account that still cannot be read waits after one renewal was asked before
  /// another is.
  pub renewal_pause_seconds: u64,
  /// How close to the moment its credential file states a login is near its end, in seconds: the CLI's
  /// own margin, inside which a run of it renews the token as it goes out.
  pub renewal_margin_seconds: u64,
  pub windows: Vec<String>,
}

/// A directory account signed in from the console through the CLI's own sign-in.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsoleLogin {
  /// The CLI's sign-in command and its arguments.
  pub command: Vec<String>,
  /// How long the CLI is given to print the sign-in's address, in seconds.
  pub link_seconds: u64,
  /// How long the CLI is given to end once the code is handed over, in seconds.
  pub finish_seconds: u64,
  /// How long a sign-in may stand before it is ended, in minutes.
  pub keep_minutes: u64,
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

/// How a login's end is told to the owner: how long before it, the year a token stands for, how long a
/// login that refused a run rests, and the message, one entry to a line, with its blanks named in braces.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
  pub notice_ahead_hours: i64,
  pub token_life_days: i64,
  pub refused_rest_minutes: i64,
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
  /// The variable the program reads its MCP tool idle window from, in milliseconds.
  pub mcp_idle_timeout: String,
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
