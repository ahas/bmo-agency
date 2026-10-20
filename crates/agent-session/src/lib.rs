//! The agent-session contract: the eleven operations every backend answers, and their request and answer
//! types (docs/api.md, 「Sessions」). A caller reaches an agent's session through this contract alone and
//! names no backend; herdr is one backend of it, implemented by the server over its door, places and
//! lines.
//!
//! What a turn carries, whom an agent answers and which model it stands on are the caller's and the
//! agent's row's, decided above this contract; a backend supplies, carries and controls, and judges
//! none of it. The shapes a turn's end, a question and its answer are written in on the wire are part of
//! the contract, so they are declared here with their names as a caller reads them.

use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
  /// A session thrown away: a co-work branch's or a research run's, every turn on it a fresh one.
  Apart(Apart),
  /// A conversation the caller opened on a provider and model of its own, kept for it until it ends.
  Conversation(Conversation),
}

/// What a thrown-away session is started on: the provider, the model and the folder its turns go out on,
/// and the item and role a co-work branch's place labels its run with, none for a research run.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Apart {
  pub provider: String,
  pub model: String,
  pub cwd: String,
  pub todo: Option<i64>,
  pub role: Option<String>,
}

/// What a conversation the caller opened stands on: the provider, the model and the folder its turns go
/// out on. A turn on a provider's road that keeps a session resumes what its last answered turn left;
/// one on a road that keeps none carries each turn afresh.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Conversation {
  pub provider: String,
  pub model: String,
  pub cwd: String,
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
/// which is the moment it is sent where none is named; an editor run's parts; the job a worker's turn
/// is of; and a work turn's parts. A backend carries each part as the line declares and judges none of
/// it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnContext {
  pub room: Option<RoomBlock>,
  pub via: Option<String>,
  pub base_persona: Option<String>,
  pub privileged: bool,
  pub answers: Vec<i64>,
  pub began: Option<i64>,
  pub run: Option<RunParts>,
  pub job: Option<JobMark>,
  pub work: Option<WorkParts>,
}

/// The job a worker's turn is of, as its caller names it, and the item that job works, where it names one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobMark {
  pub job: i64,
  pub item: Option<i64>,
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

/// A work turn's parts, as a job's runner names them (docs/api.md, 「Sessions」, a work turn's parts): the
/// harness and the model its session record is kept under, the line it is laid on, which session it goes
/// on, the folder its program works in, the run's name and what a person watching reads over it, the
/// thread its session is kept under, the room its run's lines stand under, the unit it runs under, the
/// user and the work key its logins are offered by, whether words pushed into the session reach it, and
/// the variables its program is started with beside its own.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct WorkParts {
  pub provider: String,
  pub model: String,
  pub mapping: Mapping,
  pub resume: Resume,
  pub cwd: String,
  pub name: String,
  pub title: String,
  #[serde(default)]
  pub thread: Option<String>,
  pub chat: i64,
  #[serde(default)]
  pub unit: Option<String>,
  #[serde(default)]
  pub user: Option<String>,
  pub work_key: String,
  #[serde(default)]
  pub hears_words: bool,
  #[serde(default)]
  pub variables: BTreeMap<String, String>,
}

/// The line a work turn is laid on: its model, and the effort, the tier and the answer ceiling it names
/// where it names them.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Mapping {
  pub model: String,
  #[serde(default)]
  pub effort: Option<String>,
  #[serde(default)]
  pub tier: Option<String>,
  #[serde(default)]
  pub answer_tokens: Option<u64>,
}

/// Which session a work turn goes on: the one kept for its model and login, a fresh one, or the one named.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Resume {
  Auto,
  Fresh,
  Session { id: String },
}

/// Whether the program took a turn's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
  /// The program took the words.
  Taken,
  /// The program is waiting on a person and refused the words with its pane left as it was; not a
  /// failure.
  Blocked,
}

/// How a turn ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
  Answered,
  Blocked,
  /// Ended from outside.
  Stopped,
  Failed,
  /// A worker's turn put back to its caller before anything went in: the session it settled on is not
  /// the one the caller composed its prompt for.
  Reprepared,
}

/// The error an engine ended a turn with: its kind as the program names it, `rate_limit`, `overloaded`,
/// `max_output_tokens` and the rest of its list, or none where it named none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EngineError {
  pub kind: Option<String>,
}

/// What a turn went out on once its line was settled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WentOut {
  pub provider: String,
  pub model: String,
  pub effort: Option<String>,
  pub tier: Option<String>,
}

/// A turn's answer as the program's record reported it, carried on unjudged.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
  pub mode: String,
  pub model: Option<String>,
  pub effort: Option<String>,
  pub text: String,
  /// The harness session the turn answered on.
  pub session_id: Option<String>,
  pub account: Option<String>,
  pub context_ratio: Option<f64>,
  pub context_tokens: Option<i64>,
  /// The session usage the run reported, as it stands.
  pub session_usage: Option<Value>,
  pub spend: Vec<Spend>,
  pub live_tasks: Vec<LiveTask>,
  pub workflows: Vec<Workflow>,
  /// The upstream a routed model's request went to.
  pub upstream: Option<String>,
  pub request_usages: Option<Vec<RequestUsage>>,
}

/// What one model of a turn spent.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spend {
  pub mode: Option<String>,
  pub model: String,
  pub tokens: i64,
  pub input_tokens: i64,
  pub output_tokens: i64,
  pub cache_read_tokens: i64,
  pub cache_write_tokens: i64,
  pub cost: Option<f64>,
}

/// A background task of the run's own, still running when the turn ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LiveTask {
  pub id: String,
  pub description: String,
}

/// A workflow the run launched, with how it ended where it did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Workflow {
  pub call: String,
  pub name: String,
  pub status: Option<String>,
}

/// What one model request of a turn spent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestUsage {
  pub input_tokens: i64,
  pub output_tokens: i64,
  pub cache_read_tokens: i64,
  pub cache_write_tokens: i64,
}

/// Why a turn did not answer, as its backend read it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Failure {
  /// The login's window is full; where none of the logins answers, the rest closed the road.
  Limit {
    resets_at: Option<i64>,
    account: Option<String>,
    silenced: bool,
  },
  /// What the login said of itself rather than of the work.
  Account {
    failure: AccountFailure,
    detail: String,
  },
  /// The run's process ended under it.
  Died {
    ending: RunEnding,
    said: Option<String>,
    message: String,
  },
  /// The program waits on a person.
  Person,
  /// The program would not get past the folder-trust question.
  Untrusted { folder: String, config: String },
  /// Ended from outside.
  Stopped,
  /// The way to the harness failed.
  Transport { message: String },
  /// The harness refused the turn.
  Refused { message: String },
  /// Nothing may carry the turn: its provider is off for the scope.
  Unavailable {
    scope: String,
    mode: String,
    reason: String,
  },
  /// Anything else, by its name and its words.
  Error { name: String, message: String },
}

impl Failure {
  /// The failure in a sentence, as a turn's end gives it as its reason.
  pub fn words(&self) -> String {
    match self {
      Self::Limit { resets_at, .. } => match resets_at {
        Some(at) => format!("the login's window is full until {at}"),
        None => "the login's window is full".to_owned(),
      },
      Self::Account { detail, .. } => detail.clone(),
      Self::Died { message, .. }
      | Self::Transport { message }
      | Self::Refused { message }
      | Self::Error { message, .. } => message.clone(),
      Self::Person => "the program waits on a person".to_owned(),
      Self::Untrusted { folder, .. } => format!("the program would not trust {folder}"),
      Self::Stopped => "the turn was stopped".to_owned(),
      Self::Unavailable { reason, .. } => reason.clone(),
    }
  }
}

/// What a failed turn's login said of itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountFailure {
  Limit,
  Login,
  Outage,
  NodeToken,
}

/// How a run that died ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunEnding {
  Exit { code: i64 },
}

/// Where one part of a turn's time or spending went.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Span {
  pub kind: String,
  pub name: String,
  pub seconds: f64,
  pub times: i64,
  pub tokens: Option<i64>,
  pub input_tokens: Option<i64>,
  pub output_tokens: Option<i64>,
  pub cache_read_tokens: Option<i64>,
  pub cache_write_tokens: Option<i64>,
}

/// A turn's end, as its send answers it and its `turn-ended` carries it (docs/api.md, 「Sessions」, a
/// turn's end): a part a scope's turn does not carry is none or empty.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TurnEnd {
  pub delivery: Delivery,
  pub run: String,
  pub outcome: Outcome,
  pub answer: Option<String>,
  pub reason: Option<String>,
  #[serde(rename = "engineError")]
  pub engine_error: Option<EngineError>,
  pub went_out: Option<WentOut>,
  pub reply: Option<Box<Reply>>,
  pub failure: Option<Failure>,
  pub spans: Vec<Span>,
  pub compacted: bool,
  /// The session a turn put back to its caller would continue, none for a fresh one.
  pub continues: Option<String>,
}

impl TurnEnd {
  /// A turn's end with only its run and outcome told; the program took the words unless it is blocked.
  pub fn of(run: impl Into<String>, outcome: Outcome) -> Self {
    Self {
      delivery: if outcome == Outcome::Blocked { Delivery::Blocked } else { Delivery::Taken },
      run: run.into(),
      outcome,
      answer: None,
      reason: None,
      engine_error: None,
      went_out: None,
      reply: None,
      failure: None,
      spans: Vec::new(),
      compacted: false,
      continues: None,
    }
  }

  /// A turn that answered with its text.
  pub fn answered(run: impl Into<String>, text: impl Into<String>) -> Self {
    Self {
      answer: Some(text.into()),
      ..Self::of(run, Outcome::Answered)
    }
  }

  /// A turn that failed, with why and the engine's error where its engine ended it with one.
  pub fn failed(
    run: impl Into<String>,
    reason: impl Into<String>,
    engine_error: Option<EngineError>,
  ) -> Self {
    Self {
      reason: Some(reason.into()),
      engine_error,
      ..Self::of(run, Outcome::Failed)
    }
  }
}

/// What became of words pushed into the turn a session is working.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aside {
  /// The working turn took them.
  Delivered,
  /// The program is waiting on a person, and the words stayed out.
  Waiting,
  /// They did not go in: no turn was working, its program takes no words mid-turn, or it ended first.
  Refused,
}

/// A question a turn's program stopped on, put to the turn's caller: its id and what it asks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Question {
  pub id: String,
  #[serde(flatten)]
  pub asks: Asks,
}

/// What a question asks: leave to run a command or make a request, with whether the program takes words
/// with a refusal; or something the reading does not know, with the last rows the screen shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Asks {
  Permission { text: String, amendable: bool },
  Unknown { screen: String },
}

/// The caller's answer to a question: let it go on, leave it standing for a person, or refuse it with the
/// words given.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Answer {
  Allow,
  Leave,
  Refuse {
    #[serde(default)]
    words: Option<String>,
  },
}

/// What a session's turn now going, or its last one, has written so far.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Output {
  pub run: String,
  pub running: bool,
  pub text: String,
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
  /// The run of the turn going now, none between turns.
  pub run: Option<String>,
  /// What the turn going now went out on, once its line is settled.
  pub went_out: Option<WentOut>,
  /// The question the turn going now stands on, put to its caller.
  pub question: Option<Question>,
  /// The harness session a conversation's next turn, or a worker's on the model the read names, would
  /// continue, none for a fresh one or where the read names none.
  pub continues: Option<String>,
}

impl State {
  /// A session's state between turns, with nothing continued.
  pub fn standing(
    session: SessionId,
    provider: String,
    model: String,
    place: String,
    generation: u64,
    context: Option<Context>,
  ) -> Self {
    Self {
      session,
      provider,
      model,
      place,
      generation,
      context,
      run: None,
      went_out: None,
      question: None,
      continues: None,
    }
  }
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

/// The eleven operations every backend answers.
pub trait Sessions: Send + Sync {
  /// A session started for an agent at a scope, or the one already kept for that scope.
  fn start(&self, request: Start) -> impl Future<Output = Result<Session, Fault>> + Send;

  /// A session moved to another provider and model, each left as it stands where the caller names only
  /// the other. A scope whose provider and model are its agent's refuses; a session in a turn refuses
  /// until it ends.
  fn switch(
    &self,
    session: &SessionId,
    provider: Option<&str>,
    model: Option<&str>,
  ) -> impl Future<Output = Result<Session, Fault>> + Send;

  /// One turn sent into a session and carried until it ends, answered with its end.
  fn send_turn(&self, turn: Turn) -> impl Future<Output = Result<TurnEnd, Fault>> + Send;

  /// Words pushed into the turn a session is working. A backend that cannot put words into a working turn
  /// refuses them, which changes nothing.
  fn push(&self, _session: &SessionId, _words: &str) -> impl Future<Output = Result<Aside, Fault>> + Send {
    async { Ok(Aside::Refused) }
  }

  /// A session's state as it stands; a worker's names the session its next turn on the model named would
  /// continue, and a conversation's the one its next turn would.
  fn read_state(
    &self,
    session: &SessionId,
    model: Option<&str>,
  ) -> impl Future<Output = Result<State, Fault>> + Send;

  /// The caller's answer to the question a session's turn stands on. A backend whose turns put no
  /// question to their caller refuses it.
  fn answer(
    &self,
    _session: &SessionId,
    _question: &str,
    _answer: Answer,
  ) -> impl Future<Output = Result<(), Fault>> + Send {
    async { Err(Fault::new(FaultKind::Refused, "this backend puts no question to a turn's caller")) }
  }

  /// What a session's turn now going, or its last one, has written so far. A backend that keeps no
  /// record of a turn refuses it.
  fn output(&self, _session: &SessionId) -> impl Future<Output = Result<Output, Fault>> + Send {
    async { Err(Fault::new(FaultKind::Refused, "this backend keeps no record of a turn to read")) }
  }

  /// The turn a session is in cut short; the session stands.
  fn interrupt(&self, session: &SessionId) -> impl Future<Output = Result<(), Fault>> + Send;

  /// A full session carried past its window, as its provider declares.
  fn fold(&self, session: &SessionId) -> impl Future<Output = Result<(), Fault>> + Send;

  /// A session ended and its program taken down.
  fn end(&self, session: &SessionId) -> impl Future<Output = Result<(), Fault>> + Send;

  /// Every session the backend keeps.
  fn list(&self) -> impl Future<Output = Result<Vec<Session>, Fault>> + Send;
}
