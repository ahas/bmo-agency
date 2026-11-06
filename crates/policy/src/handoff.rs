//! The handoff a supervisor's session going away is asked for (docs/sessions.md, 「A restart hands the session
//! over」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
  /// How long agency waits before asking again while a handoff is under way, in seconds.
  pub retry_seconds: u64,
  pub reasons: Reasons,
}

/// The reasons a handoff is asked with.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reasons {
  /// The kept session gone.
  pub lost: String,
  /// A fresh session replacing the one the agent had.
  pub replaced: String,
  /// The documents the session's prompt is composed from changed.
  pub moved: String,
}

crate::part!(pub HANDOFF: Handoff = "handoff.toml");
