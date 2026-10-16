//! The note a closed session leaves for the run that replaces it (docs/sessions.md, 「Session
//! replacement」; docs/api.md, 「Hooks」, the session summariser): agency hands the summariser what closed
//! and carries the note it answers. How a session is summarised, how long the note may be and how it is
//! worded are the summariser's; with none registered, nothing is carried over.

use serde::Serialize;

use crate::hooks::{HookPoint, Hooks};

/// What kind of session closed, which the note is written for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
  /// A room's session.
  Chat,
  /// A job worker's session, replaced or handed over.
  Worker,
  /// A job worker's session set aside as the worker turns to another model.
  Switch,
}

/// A closed session as the summariser is asked about it: the provider it stood on, its session id, its
/// folder, the login it was opened under, what was said on a road that keeps no session of its own, the
/// note the worker wrote itself where it wrote one, and the room.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Closed {
  pub kind: Kind,
  pub mode: String,
  pub session_id: Option<String>,
  pub cwd: String,
  pub account: Option<String>,
  pub transcript: Option<String>,
  pub own_note: Option<String>,
  pub chat_id: Option<i64>,
}

/// The point the summariser is asked at.
pub struct Summariser;

impl HookPoint for Summariser {
  type Ask = Closed;
  type Answer = Option<String>;
  const NAME: &'static str = "session-summariser";

  fn default_answer(_closed: Closed) -> Option<String> {
    None
  }
}

/// The note the closed session leaves, as the summariser words it, or none where it answers none: an
/// empty note is no note.
pub async fn handover(hooks: &Hooks, closed: Closed) -> Option<String> {
  hooks.ask::<Summariser>(closed).await.filter(|note| !note.trim().is_empty())
}
