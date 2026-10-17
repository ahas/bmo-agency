//! The agent-session contract: the seven operations every backend answers, and their request and answer
//! types (docs/api.md, 「Sessions」). A caller reaches an agent's session through this contract alone and
//! names no backend; herdr is one backend of it, implemented by the server over its door, places and
//! lines.
//!
//! What a turn carries, whom an agent answers and which model it stands on are the caller's and the
//! agent's row's, decided above this contract; a backend supplies, carries and controls, and judges
//! none of it.

use std::fmt;
use std::future::Future;

/// What a session is known by, as the backend that started it named it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(pub String);

impl fmt::Display for SessionId {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.0)
  }
}

/// The scope a session is kept for, the caller's declaration: one harness session is kept per scope
/// (docs/agents.md, 「The session's scope」).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
  /// One session across every room the agent answers.
  Agent,
  /// One session for each room, named by the room's id.
  Room(String),
  /// A worker's session, kept for the work it is given.
  Worker,
  /// An editor work key's runs, each a run apart on a session of its own that nothing keeps.
  Editor,
}

/// A session asked for: the agent, by its stable name, and the scope it is kept for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Start {
  pub agent: String,
  pub scope: Scope,
}

/// A session as a list or a start answers it: what it is known by, whose it is and its scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
  pub id: SessionId,
  pub agent: String,
  pub scope: Scope,
}

/// One turn: the session it goes into, its prompt, and the context its caller composed around it; the
/// session keeps the conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
  pub session: SessionId,
  pub prompt: String,
  pub context: TurnContext,
}

/// A room's block as the caller composed it for a turn: the room it is, its text, and the mark the turn's
/// answer carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomBlock {
  pub chat_id: i64,
  pub text: String,
  pub mark: String,
}

/// What the caller composed around a turn's prompt, every part of it optional: the room it answers, which a
/// session across rooms needs; the road it answers on where that is not the room's own; the persona that
/// follows the line's system prompt; whether every new message it answers is the owner's or no person's
/// message opened it; the messages it answers; the moment it took its words, in epoch milliseconds,
/// which is the moment it is sent where none is named; and an editor run's parts. A backend carries each
/// part as the line declares and judges none of it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnContext {
  pub room: Option<RoomBlock>,
  pub via: Option<String>,
  pub base_persona: Option<String>,
  pub privileged: bool,
  pub answers: Vec<i64>,
  pub began: Option<i64>,
  pub run: Option<RunParts>,
}

/// An editor run's parts as its caller names them: the run's name and the unit it runs under, the agent
/// whose memory it writes and the room it reads, which batch it is of how many, the agent's folder under
/// the brain, the fragment desk its batch opened, and the registration it runs on, none for its work key's
/// own road.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunParts {
  pub name: String,
  pub parent: Option<String>,
  pub agent: Option<String>,
  pub chat_id: Option<i64>,
  pub step: Option<i64>,
  pub steps: Option<i64>,
  pub workdir: Option<String>,
  pub desk: Option<String>,
  pub registration: Option<String>,
}

/// What became of a turn sent. Its lifecycle after it is taken is published as events, not answered
/// here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
  /// The program took the words.
  Taken,
  /// The program is waiting on a person and refused the words with its pane left as it was; not a
  /// failure.
  Blocked,
}

/// The context a session's last request carried against the window of the registration it stands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
  pub carried: u64,
  pub window: u64,
}

/// A session's state, read to show it; a caller builds no run from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
  pub session: SessionId,
  pub provider: String,
  pub model: String,
  /// Where the session's program stands, as its backend names the place.
  pub place: String,
  /// How many times the session has been opened afresh: a reset or a hand-over opens a new generation.
  pub generation: u64,
  /// Nothing before the session's first request has been read.
  pub context: Option<Context>,
}

/// Why an operation was not carried out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultKind {
  /// No session is known by the id named.
  Unknown,
  /// The session is in a turn and takes no other operation until it ends.
  Busy,
  /// The backend did not answer.
  Unreachable,
  /// The backend answered with a refusal of its own, told in the message.
  Refused,
}

impl FaultKind {
  /// The kebab-case outcome code the fault is answered with.
  pub fn code(self) -> &'static str {
    match self {
      Self::Unknown => "unknown-session",
      Self::Busy => "session-busy",
      Self::Unreachable => "backend-unreachable",
      Self::Refused => "backend-refused",
    }
  }
}

/// An operation not carried out: its kind and an English sentence saying why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
  pub kind: FaultKind,
  pub message: String,
}

impl Fault {
  pub fn new(kind: FaultKind, message: impl Into<String>) -> Self {
    Self {
      kind,
      message: message.into(),
    }
  }
}

impl fmt::Display for Fault {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(formatter, "{}: {}", self.kind.code(), self.message)
  }
}

impl std::error::Error for Fault {}

/// The seven operations every backend answers.
pub trait Sessions: Send + Sync {
  /// A session started for an agent at a scope, or the one already kept for that scope.
  fn start(&self, request: Start) -> impl Future<Output = Result<Session, Fault>> + Send;

  /// One turn sent into a session.
  fn send_turn(&self, turn: Turn) -> impl Future<Output = Result<Delivery, Fault>> + Send;

  /// A session's state as it stands.
  fn read_state(&self, session: &SessionId) -> impl Future<Output = Result<State, Fault>> + Send;

  /// The turn a session is in cut short; the session stands.
  fn interrupt(&self, session: &SessionId) -> impl Future<Output = Result<(), Fault>> + Send;

  /// A full session carried past its window, as its provider declares.
  fn fold(&self, session: &SessionId) -> impl Future<Output = Result<(), Fault>> + Send;

  /// A session ended and its program taken down.
  fn end(&self, session: &SessionId) -> impl Future<Output = Result<(), Fault>> + Send;

  /// Every session the backend keeps.
  fn list(&self) -> impl Future<Output = Result<Vec<Session>, Fault>> + Send;
}
