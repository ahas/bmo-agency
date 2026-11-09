//! The herdr backend of the session contract (docs/api.md, 「Sessions」): the ten operations answered over
//! the door, the places and the lines. A worker's session is its own, one per worker, carried by the
//! worker's session turn on the line a job runs on; a supervisor's session is one per room, carried by its
//! resident program on the chat line with the system prompt its settings compose; a session across rooms is
//! the one its agent's row keeps, carried by its resident program on the chat line around the room block,
//! the road and the base persona, its brain's or else the one the turn's caller hands in, or by a stand-in
//! where the agent's key walks onto one; a thrown-away session, held in memory from its start to its end,
//! carries a co-work branch's turns or a runner line's, each on a fresh harness session; and a conversation
//! the caller opened, held there too, carries ordinary turns on the provider and model it names, resuming the
//! session its road keeps. A session is known by its agent and its scope — `<agent>@worker`,
//! `<agent>@<room>`, `<agent>`, `<agent>@run:<n>`, `<agent>@conversation:<n>` — and stands across the
//! generations its harness opens.
//!
//! A turn is carried whole before *send a turn* answers with its end: whether the program took the words,
//! or refused them while it waits on a person, and how it ended. Its end is published as `turn-ended`, and
//! its start as
//! `turn-started`; a conversational turn's road as `road-walked`. Words pushed into a session are offered to
//! the run its turn carries, which its door takes at its next look at the working program.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub use agent_session::EngineError;
use agent_session::{
  Answer, Apart, Aside, Context, Conversation, Fault, FaultKind, GateAsk, Outcome, Output, RoomBlock,
  Scope, Session, SessionId, Sessions, Start, State, Turn, TurnContext, TurnEnd, WentOut, WorkParts,
};
use indexmap::IndexMap;
use policy::fill_marks;
use policy::gate_places::GATE_PLACES;
use policy::handoff::HANDOFF;
use policy::supervisor_runner::SUPERVISOR_RUNNER;
use sea_orm::TransactionTrait;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use store::{all, one, run as written};

use crate::chat_stand_in::{fallback_ended, stand_in_turn};
use crate::control::supervisor_settings::Settings;
use crate::door::aside;
use crate::door::record::{ModelSpend, UsageSpent};
use crate::door::turn::AsideOutcome;
use crate::events::Event;
use crate::job_run::claude::turn::{Shows, TASK_LOCKED, TURN_AGENT, WorkAsked, WorkSeat, carry, continues};
use crate::lines::Trees;
use crate::lines::chat::{ChatLaunch, RoomContext, compose_prepared, compose_turn, gate_run};
use crate::lines::pieces::{ComposedTurn, SupervisorTurn, compose, lost_session, registration_named};
use crate::places::gate_places;
use crate::places::{AgentRow, Ground, Places};
use crate::run_ledger;
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::session_output::{OutputWatch, Outputs};
use crate::session_questions::Questions;
use crate::supervisor_turn::guard::{Spent, spent_out};
use crate::supervisor_turn::handoff::{Due, Speaker, Why, hand_over};
use crate::supervisor_turn::overtaken;
use crate::supervisor_turn::run::{NoAnswer, Reply, Tally, let_go, run_turn};
use crate::supervisor_turn::sessions::{self, Harness, Kept, RoomSession, harness_road};
use crate::watches::machine::keep_worker_job;
use crate::worker_session;
use crate::worker_session::fold::{Folded, fold};
use crate::worker_session::records::{self, note_answer, slot_account};
use crate::{Refusal, now_ms};

// What a worker's session is told apart by in its id, after its agent.
const WORKER: &str = "worker";

// What a thrown-away session is told apart by in its id, after its agent.
const APART: &str = "run";

// What a conversation the caller opened is told apart by in its id, after its agent.
const CONVERSATION: &str = "conversation";

/// Remembers a thrown-away session a picked-up turn still runs on, with the scope its execution row names, so
/// its state answers by id until that turn's end is handed back.
pub fn remember_carried(ground: &Ground, session: SessionId, agent: String, scope: Apart) {
  ground
    .carried
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner)
    .insert(session, (agent, scope));
}

/// Lets go of a carried thrown-away session as its turn ends.
pub fn forget_carried(ground: &Ground, session: &SessionId) {
  ground.carried.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(session);
}

// The scope a carried thrown-away session stands on, where the pickup adopted its turn.
fn carried_scope_of(ground: &Ground, id: &SessionId) -> Option<(String, Apart)> {
  ground
    .carried
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner)
    .get(id)
    .cloned()
}

/// A turn put into a session began: the session, its id, and the run it goes out as.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStarted {
  pub session: String,
  pub run: String,
  pub turn_id: i64,
  pub at: i64,
}

impl Event for TurnStarted {
  const NAME: &'static str = "turn.created";
}

/// What a running turn waits for, as it begins, changes and clears (docs/api.md, 「sessions」): the session,
/// the turn, the run it goes out as, and the wait, none once it has cleared.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnChanged {
  pub session: String,
  pub turn_id: i64,
  pub run: String,
  pub waiting: Option<agent_session::Waiting>,
}

impl Event for TurnChanged {
  const NAME: &'static str = "turn.changed";
}

/// What a turn came to, as its send answers it (docs/api.md, 「Sessions」, a turn's end), and the session
/// it went on.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TurnEnded {
  pub session: String,
  #[serde(flatten)]
  pub end: TurnEnd,
}

impl Event for TurnEnded {
  const NAME: &'static str = "turn.ended";
}

/// A session's system prompt changed, as the turn that sent the changed prompt records it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemPromptChanged {
  pub session: String,
  pub run: String,
}

impl Event for SystemPromptChanged {
  const NAME: &'static str = "system-prompt-changed";
}

/// The road a conversational turn walked (docs/run-records.md, 「The roads a turn walked」): its stage, the
/// room and the run, the provider and model it went out on, how it ended, how long it held the run, its
/// tokens summed over the models that answered — none where it reported none — and how full the session's
/// window stood after it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadWalked {
  pub stage: String,
  pub chat_id: i64,
  pub run: String,
  pub provider: String,
  pub model: String,
  pub outcome: &'static str,
  pub ms: i64,
  pub input_tokens: Option<i64>,
  pub output_tokens: Option<i64>,
  pub cache_read_tokens: Option<i64>,
  pub cache_write_tokens: Option<i64>,
  pub context_ratio: Option<f64>,
}

impl Event for RoadWalked {
  const NAME: &'static str = "road-walked";
}

impl RoadWalked {
  /// The road a turn walked from `started`, its tokens added over what each model reported.
  #[allow(clippy::too_many_arguments)]
  pub fn of(
    chat_id: i64,
    run: &str,
    provider: &str,
    model: &str,
    answered: bool,
    started: Instant,
    spend: &[ModelSpend],
    context_ratio: Option<f64>,
  ) -> Self {
    let reported = !spend.is_empty();
    let sum = |pick: fn(&ModelSpend) -> i64| reported.then(|| spend.iter().map(pick).sum::<i64>());

    Self {
      stage: SUPERVISOR_RUNNER.turn_stage.clone(),
      chat_id,
      run: run.to_owned(),
      provider: provider.to_owned(),
      model: model.to_owned(),
      outcome: if answered { "answered" } else { "error" },
      ms: i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
      input_tokens: sum(|one| one.input_tokens),
      output_tokens: sum(|one| one.output_tokens),
      cache_read_tokens: sum(|one| one.cache_read_tokens),
      cache_write_tokens: sum(|one| one.cache_write_tokens),
      context_ratio,
    }
  }
}

// The provider a turn across rooms goes out on: the chat line is mapped on Claude Code alone.
const CHAT_PROVIDER: &str = "claude-code";

/// The model a repository's pin names now, or none where the repository pins none. A job there stands on
/// this model (docs/repositories.md, 「A pinned model」).
pub async fn pinned_worker_model(
  db: &sea_orm::DatabaseConnection,
  repository: &str,
) -> Result<Option<String>, store::Fault> {
  Ok(
    crate::repositories::named(db, repository)
      .await?
      .and_then(|repository| repository.pin)
      .map(|pin| pin.model),
  )
}

/// The harness session a supervisor's row keeps across rooms, where it keeps one.
pub async fn agent_session(
  db: &sea_orm::DatabaseConnection,
  agent: &str,
) -> Result<Option<String>, Refusal> {
  Ok(
    one(db, r#"SELECT "session_id" FROM "supervisors" WHERE "name" = $1"#, vec![agent.into()])
      .await?
      .and_then(|row| row.try_get::<Option<String>>("", "session_id").ok().flatten())
      .filter(|session| !session.is_empty()),
  )
}

/// The provider a supervisor's last answer went out on, where it answered: the road of the session its row
/// keeps, so a stand-in's session is resumed on the same road and never handed to another.
pub async fn agent_last_mode(
  db: &sea_orm::DatabaseConnection,
  agent: &str,
) -> Result<Option<String>, Refusal> {
  Ok(
    one(db, r#"SELECT "last_mode" FROM "supervisors" WHERE "name" = $1"#, vec![agent.into()])
      .await?
      .and_then(|row| row.try_get::<Option<String>>("", "last_mode").ok().flatten())
      .filter(|mode| !mode.is_empty()),
  )
}

/// The harness session a supervisor keeps across rooms written on its row, with the room its last turn
/// answered; none lets it go.
pub async fn keep_agent_session(
  db: &sea_orm::DatabaseConnection,
  agent: &str,
  kept: Option<(&str, i64)>,
) -> Result<(), Refusal> {
  let let_go = kept.is_none();
  let (session, chat_id) =
    kept.map_or((None, None), |(session, chat_id)| (Some(session.to_owned()), Some(chat_id)));
  let transaction = db.begin().await?;

  written(
    &transaction,
    r#"UPDATE "supervisors" SET "session_id" = $2, "chat_id" = COALESCE($3, "chat_id") WHERE "name" = $1"#,
    vec![agent.into(), session.into(), chat_id.into()],
  )
  .await
  .map(drop)
  .map_err(Refusal::from)?;

  // A session let go takes nothing it had loaded into the one that replaces it.
  if let_go {
    sessions::forget_loads(&transaction, agent).await?;
  }

  transaction.commit().await?;

  Ok(())
}

/// The session contract over herdr, on the places the server stands.
pub struct HerdrSessions {
  places: Places,
  // The turn each session carries while it goes, with the run it goes out as, which the interrupt stops:
  // one a session, a second turn refused while it goes.
  going: Mutex<HashMap<SessionId, Vec<Running>>>,
  // The sessions whose turn waits on the receiver's handoff turn. The session is lent to that turn, which
  // would otherwise be refused as a second turn of it (docs/sessions.md, 「A restart hands the session
  // over」).
  handing: Mutex<HashSet<SessionId>>,
  // The turns a handoff turn displaced, put back when it ends.
  parked: Mutex<HashMap<SessionId, Vec<Running>>>,
  // What each session's turn going now shows of itself to a reading of its state: what it went out on,
  // once its line is settled.
  showing: Mutex<HashMap<SessionId, WentOut>>,
  // The thrown-away sessions started and not ended, each with its agent and its scope, and the number the
  // next one is given.
  apart: Mutex<BTreeMap<SessionId, (String, Apart)>>,
  // The conversations the callers opened and have not ended, each with its agent and what it stands on,
  // and the number the next one is given.
  conversations: Mutex<BTreeMap<SessionId, (String, HeldConversation)>>,
  numbered: AtomicU64,
  // The id the next turn begun is given, past every id given before.
  turns: AtomicI64,
  // The questions the sessions' turns stand on, put to their callers.
  questions: Questions,
  // The sessions whose turn going now hears no words pushed into it.
  deaf: Mutex<HashSet<SessionId>>,
  // What each session's latest job turn wrote of its answer.
  outputs: Outputs,
  // The thrown-away sessions whose first prompt has been handed the gate's reference, so their later turns
  // are asked over it and not over it again.
  referenced: Mutex<HashSet<SessionId>>,
  // The context each room's and agent's session was left with by its last turn that read one.
  left: Mutex<HashMap<SessionId, Context>>,
  // The runs cut because the slot their session was bound to was given back, each with the reason its end
  // carries.
  cut: Mutex<HashMap<String, String>>,
}

/// One turn a session carries while it goes: its id, and the run it goes out as.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Running {
  turn: i64,
  run: String,
}

/// A conversation the caller opened: what its turns go out on, and the session its last answered turn on
/// a road that keeps one left for the next turn to resume.
#[derive(Clone, Debug)]
struct HeldConversation {
  conversation: Conversation,
  harness: Option<String>,
}

fn fault(kind: FaultKind, message: impl Into<String>) -> Fault {
  Fault::new(kind, message)
}

fn store_fault(refusal: Refusal) -> Fault {
  fault(FaultKind::Unreachable, refusal.detail)
}

/// A session's id: its agent and its scope. A thrown-away session has none to name here, since its id
/// carries the number the backend gives it as it starts (`apart_id`).
pub fn id_of(agent: &str, scope: &Scope) -> Option<SessionId> {
  let id = match scope {
    Scope::Agent => agent.to_owned(),
    Scope::Room(room) => format!("{agent}@{room}"),
    Scope::Worker => format!("{agent}@{WORKER}"),
    Scope::Apart(_) | Scope::Conversation(_) => return None,
  };

  Some(SessionId(id))
}

// A session the list names, where its scope gives it an id.
fn listed(agent: String, scope: Scope) -> Option<Session> {
  id_of(&agent, &scope).map(|id| Session { id, agent, scope })
}

/// The agent and scope a session's id names.
pub fn scope_of(id: &SessionId) -> (String, Scope) {
  match id.0.split_once('@') {
    None => (id.0.clone(), Scope::Agent),
    Some((agent, WORKER)) => (agent.to_owned(), Scope::Worker),
    Some((agent, room)) => (agent.to_owned(), Scope::Room(room.to_owned())),
  }
}

/// The context a worker's session record kept of its last answered turn: the tokens it carried, against
/// the window they filled at the share they filled it; none where its record keeps neither, or its share
/// reads as nothing.
pub fn carried_of(tokens: Option<i64>, ratio: Option<f64>) -> Option<Context> {
  let (tokens, ratio) = (tokens?, ratio?);
  let carried = u64::try_from(tokens).ok()?;

  (ratio > 0.0).then(|| Context {
    carried,
    window: (carried as f64 / ratio).round() as u64,
  })
}

// What a session's job turn shows of itself to a reading of the session while it goes.
struct Showing<'s> {
  sessions: &'s HerdrSessions,
  session: &'s SessionId,
  turn: i64,
}

impl Shows for Showing<'_> {
  fn went_out(&self, went_out: &WentOut) {
    self.sessions.showing().insert(self.session.clone(), went_out.clone());
    self.sessions.outputs.attempted(self.session, self.turn);
  }

  fn wrote(&self, texts: &[String]) {
    self.sessions.outputs.wrote(self.session, self.turn, texts);
  }
}

/// Whether words pushed into a session reach the turn it carries: a job's turn says so in its work parts,
/// and every other turn hears them.
pub fn hears_words(work: Option<&WorkParts>) -> bool {
  work.is_none_or(|work| work.hears_words)
}

/// The id a thrown-away session is started under: its agent and the number the backend gave it.
pub fn apart_id(agent: &str, number: u64) -> SessionId {
  SessionId(format!("{agent}@{APART}:{number}"))
}

/// Whether a session's id names a thrown-away session.
pub fn is_apart(id: &SessionId) -> bool {
  numbered_scope(id, APART)
}

/// The id a conversation the caller opened is started under: its agent and the number the backend gave
/// it.
pub fn conversation_id(agent: &str, number: u64) -> SessionId {
  SessionId(format!("{agent}@{CONVERSATION}:{number}"))
}

/// Whether a session's id names a conversation the caller opened.
pub fn is_conversation(id: &SessionId) -> bool {
  numbered_scope(id, CONVERSATION)
}

// Whether a session's id names a numbered session of the scope word given.
fn numbered_scope(id: &SessionId, word: &str) -> bool {
  id.0
    .split_once('@')
    .and_then(|(_, scope)| scope.strip_prefix(word))
    .is_some_and(|rest| rest.strip_prefix(':').is_some_and(|number| number.parse::<u64>().is_ok()))
}

// Whether a role's session's turns go into a place of its own: every road but Claude Code's own, whose runs
// each stand in a pane of their own (docs/api.md, 「sessions」).
fn in_places(apart: &Apart) -> bool {
  apart.provider != worker_session::PROVIDER
}

// A thrown-away session asked of what is no thrown-away session's to answer.
fn apart_refused(agent: &str) -> Fault {
  fault(FaultKind::Refused, format!("{agent}: a thrown-away session keeps no agent's row"))
}

// A room's id as the store keeps it.
fn room_number(room: &str) -> Result<i64, Fault> {
  room
    .parse()
    .map_err(|_| fault(FaultKind::Refused, format!("{room} is not a room's id")))
}

// What a supervisor's row holds that its turn reads: who it is, its order, its prompts and its threshold.
struct Pinned {
  speaker: Speaker,
  supervisor: crate::supervisors::Supervisor,
  prompts: Option<String>,
}

async fn pinned(ground: &Ground, agent: &str) -> Result<Option<Pinned>, Fault> {
  let held = crate::supervisors::named(&ground.db, agent)
    .await
    .map_err(|why| store_fault(why.into()))?;
  let Some(supervisor) = held else { return Ok(None) };

  Ok(Some(Pinned {
    speaker: Speaker {
      name: agent.to_owned(),
      label: supervisor.label.clone(),
      pinned_model: None,
    },
    prompts: supervisor.prompts.clone(),
    supervisor,
  }))
}

// The provider and model a supervisor's turn goes out on: the first provider of its own order that has quota
// above its share and can answer now (docs/supervisors.md, 「Where a supervisor's turn runs」). Where none
// can, the turn waits, never refused, until a provider of its order can again: the walk is asked again as
// each usage reading, account change or road fact comes in, and on the line's own pass.
async fn choice_of(
  ground: &Ground,
  pinned: &Pinned,
) -> Result<(&'static dyn harness_providers::Provider, String), String> {
  let supervisor = &pinned.supervisor;
  let ask = crate::road::Ask::of(crate::road::Row::Supervisor(&supervisor.name), "chat", &supervisor.order);

  loop {
    let mut moved = crate::order::quota_watch(ground);
    let resolution = crate::road::resolve(ground, &ask).await?;
    let took = resolution.road().cloned();

    crate::road::went(ground, &resolution, took.as_ref(), &[]).await;

    if let Some(road) = took {
      let provider = harness_providers::provider(&road.provider)
        .ok_or_else(|| format!("{} is no provider of this build", road.provider))?;

      return Ok((provider, road.model));
    }

    if moved.next().await.is_none() {
      return Err(resolution.sentence());
    }
  }
}

// The system prompt a supervisor's settings compose, out of the documents of the tree the process runs, for
// the provider and model it answers on.
async fn system_of(
  ground: &Ground,
  pinned: &Pinned,
  settings: &Settings,
  provider: &dyn harness_providers::Provider,
  model: &str,
) -> Result<String, String> {
  let chosen = one(
    &ground.db,
    "SELECT p.text FROM agent_base_prompts c JOIN base_prompts p ON p.name = c.prompt WHERE c.agent = $1",
    vec![pinned.speaker.name.clone().into()],
  )
  .await
  .map_err(|why| why.detail)?
  .and_then(|row| row.try_get::<String>("", "text").ok());
  let base = chosen.map(|text| harness_machine::supervisor_prompt::Base {
    text,
    model: model.to_owned(),
  });
  let roots = harness_machine::supervisor_prompt::Roots {
    assets: ground.code_root(),
    data: ground.roots.data.clone(),
  };
  let supervisor = harness_machine::supervisor_prompt::Supervisor {
    name: &pinned.speaker.name,
    prompts: pinned.prompts.as_deref(),
  };

  harness_machine::supervisor_prompt::system(
    &roots,
    supervisor,
    &settings.composition(),
    base.as_ref(),
    crate::lines::pieces::takes_harness(provider.name()),
  )
  .map_err(|why| why.to_string())
}

// The generation settings a supervisor's row holds, in the order they were saved.
fn generation_of(settings: &Settings) -> IndexMap<String, Value> {
  settings
    .generation()
    .iter()
    .filter(|(_, value)| value.is_number() || value.is_boolean())
    .map(|(name, value)| (name.clone(), value.clone()))
    .collect()
}

// What a turn on a session came to, before it is published: a failure carries why, and the engine's error
// where its engine ended the turn with one.
enum Came {
  /// The answer, with what the turn spent where its road reported it and the context it left where it read.
  Answered(String, Option<run_ledger::Spent>, Option<Context>),
  Stopped,
  Failed(String, Option<EngineError>),
  /// A job's turn, whose end is read whole where it is carried.
  Ended(Box<TurnEnd>),
}

impl Came {
  // The turn's end under the run it went out as.
  fn end(self, run: &str) -> TurnEnd {
    match self {
      Self::Answered(text, _, _) => TurnEnd::answered(run, text),
      Self::Stopped => TurnEnd::of(run, Outcome::Stopped),
      Self::Failed(why, engine) => TurnEnd::failed(run, why, engine),
      Self::Ended(end) => *end,
    }
  }

  // What the turn spent, read off the answer where it carried it and off a job's end where that did.
  fn spent(&self) -> Option<run_ledger::Spent> {
    match self {
      Self::Answered(_, spent, _) => *spent,
      Self::Ended(end) => turn_spent(end),
      _ => None,
    }
  }

  // The context the turn left, where its answer read it.
  fn context(&self) -> Option<Context> {
    match self {
      Self::Answered(_, _, context) => *context,
      Self::Ended(end) => end
        .reply
        .as_ref()
        .and_then(|reply| carried_of(reply.context_tokens, reply.context_ratio)),
      _ => None,
    }
  }
}

// The context a supervisor run's answer left.
fn left(reply: &Reply) -> Option<Context> {
  carried_of(reply.context_tokens, reply.context_ratio)
}

/// What a turn's models spent, each figure summed over them; none where they reported nothing.
pub fn spent_of(spend: &[ModelSpend]) -> Option<run_ledger::Spent> {
  (!spend.is_empty()).then(|| {
    spend.iter().fold(run_ledger::Spent::default(), |sum, one| run_ledger::Spent {
      input_tokens: sum.input_tokens + one.input_tokens,
      output_tokens: sum.output_tokens + one.output_tokens,
      cache_read_tokens: sum.cache_read_tokens + one.cache_read_tokens,
      cache_write_tokens: sum.cache_write_tokens + one.cache_write_tokens,
    })
  })
}

/// The word a turn's end goes by: an answer and a turn put back go by their outcome; any other by the
/// ending the door's last turn under its send came to, where one went through it and did not answer, else
/// by its outcome.
pub fn ending_of(end: &TurnEnd) -> &'static str {
  let outcome = match end.outcome {
    Outcome::Answered => "answered",
    Outcome::Blocked => "blocked",
    Outcome::Stopped => "stopped",
    Outcome::Failed => "failed",
    Outcome::Reprepared => "reprepared",
  };

  match end.outcome {
    Outcome::Answered | Outcome::Reprepared => outcome,
    _ => match crate::door::liveness::door_ending() {
      Some("answered") | None => outcome,
      Some(word) => word,
    },
  }
}

/// Why a run came to no answer, and the engine's error where its engine ended it with one.
pub fn failure_of(no: NoAnswer) -> (String, Option<EngineError>) {
  match no {
    NoAnswer::Errored { error, kind } => (error, Some(EngineError { kind })),
    NoAnswer::Cut(why) | NoAnswer::Failed(why) | NoAnswer::Unsubmitted(why) => (why, None),
    NoAnswer::Overtaken => ("the supervisor's reset overtook the turn".to_owned(), None),
  }
}

// Whether an agent's name is a gate work key: a gate's conversation carries the caller's whole system prompt.
fn gate_work_key(agent: &str) -> bool {
  policy::work_keys::WORK_KEYS
    .profile(agent)
    .is_some_and(|profile| profile.kind == "gate")
}

impl HerdrSessions {
  pub fn new(places: Places) -> Self {
    Self {
      places,
      going: Mutex::default(),
      handing: Mutex::default(),
      parked: Mutex::default(),
      showing: Mutex::default(),
      apart: Mutex::default(),
      conversations: Mutex::default(),
      numbered: AtomicU64::new(1),
      turns: AtomicI64::new(0),
      questions: Questions::default(),
      deaf: Mutex::default(),
      outputs: Outputs::default(),
      referenced: Mutex::default(),
      left: Mutex::default(),
      cut: Mutex::default(),
    }
  }

  // The context an agent's resident program stands at where it carries its own turns, as its record says of its
  // last answer (docs/sessions.md, 「Session compaction」); none on any other provider.
  fn resident_context(&self, agent: &str, provider: &str) -> Option<Context> {
    if !crate::chat_stand_in::inbox_declared(provider) {
      return None;
    }

    let place = crate::lines::place::Place::of(&self.ground().roots.data, agent);
    let (carried, window) = crate::door::inbox::latest_context(&place.path)?;

    Some(Context { carried, window })
  }

  // The context a session's last turn left it with, where one read it.
  fn left_of(&self, session: &SessionId) -> Option<Context> {
    self.left.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).get(session).copied()
  }

  // What a session's last turn left, no longer what it carries once it has folded or ended.
  fn forget_left(&self, session: &SessionId) {
    self.left.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(session);
  }

  fn deaf(&self) -> std::sync::MutexGuard<'_, HashSet<SessionId>> {
    self.deaf.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  fn referenced(&self) -> std::sync::MutexGuard<'_, HashSet<SessionId>> {
    self.referenced.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  fn showing(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, WentOut>> {
    self.showing.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  fn apart(&self) -> std::sync::MutexGuard<'_, BTreeMap<SessionId, (String, Apart)>> {
    self.apart.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  // The agent and scope a session's id names: a numbered session's as it was started, which one that
  // has ended or was never started names none of.
  fn scope_held(&self, id: &SessionId) -> Result<(String, Scope), Fault> {
    if is_apart(id) {
      return match self.apart().get(id) {
        Some((agent, apart)) => Ok((agent.clone(), Scope::Apart(apart.clone()))),
        None => Err(fault(FaultKind::Unknown, format!("{id} is no thrown-away session standing now"))),
      };
    }

    if is_conversation(id) {
      return match self.conversations().get(id) {
        Some((agent, held)) => Ok((agent.clone(), Scope::Conversation(held.conversation.clone()))),
        None => Err(fault(FaultKind::Unknown, format!("{id} is no conversation standing now"))),
      };
    }

    Ok(scope_of(id))
  }

  fn conversations(&self) -> std::sync::MutexGuard<'_, BTreeMap<SessionId, (String, HeldConversation)>> {
    self.conversations.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  // The agent and scope the backend holds a session under, for a route that reads or moves a session
  // whose id a numbered scope names.
  pub async fn held_scope(&self, session: &SessionId) -> Result<(String, Scope), Fault> {
    self.scope_held(session)
  }

  /// The scope a thrown-away session the pickup carries stands on, where this process adopted its turn: its
  /// state is read by id until that turn's end is handed back (docs/sessions.md, 「A restart hands the session
  /// over」).
  pub fn carried_scope(&self, session: &SessionId) -> Option<(String, Apart)> {
    carried_scope_of(self.ground(), session)
  }

  fn ground(&self) -> &Ground {
    &self.places.ground
  }

  fn going(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, Vec<Running>>> {
    self.going.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  // The sessions lent to the receiver's handoff turn while this process waits on it.
  fn handing(&self) -> std::sync::MutexGuard<'_, HashSet<SessionId>> {
    self.handing.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  // The turns a handoff turn displaced, put back when it ends.
  fn parked(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, Vec<Running>>> {
    self.parked.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  // The id the next turn begun is given: the time it began, past every id given before it.
  fn next_turn(&self) -> i64 {
    let at = now_ms();
    let last = self
      .turns
      .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |last| Some(at.max(last + 1)))
      .unwrap_or(0);

    at.max(last + 1)
  }

  // Whether a session carries a turn now.
  fn in_turn(&self, session: &SessionId) -> bool {
    self.going().get(session).is_some_and(|runs| !runs.is_empty())
  }

  /// The run the turn a session carries now goes out as, none while it carries no turn.
  pub fn turn_run(&self, session: &SessionId) -> Option<String> {
    self
      .going()
      .get(session)
      .and_then(|runs| runs.first())
      .map(|running| running.run.clone())
  }

  /// The turn a session carries now: its id and the run it goes out as, none while it carries no turn.
  pub fn turn_going(&self, session: &SessionId) -> Option<(i64, String)> {
    self
      .going()
      .get(session)
      .and_then(|runs| runs.first())
      .map(|running| (running.turn, running.run.clone()))
  }

  /// A role session ended under whatever turn it carries, as giving back the slot it is bound to ends it
  /// (docs/role-slots.md, 「The line」): each turn running in it cut, ending `stopped` with the reason given,
  /// waited for until it has ended — on its end, never on a span — and the session then ended.
  pub async fn end_under_turns(&self, session: &SessionId, reason: &str) -> Result<(), Fault> {
    let mut ended = self.ground().events.subscribe::<TurnEnded>();

    loop {
      let runs = self.going().get(session).cloned().unwrap_or_default();

      if runs.is_empty() {
        break;
      }

      for running in &runs {
        self
          .cut
          .lock()
          .unwrap_or_else(|poisoned| poisoned.into_inner())
          .insert(running.run.clone(), reason.to_owned());

        match self.interrupt(session, running.turn).await {
          // A turn that ended between the read and the cut is no longer there to cut.
          Ok(()) => {}
          Err(fault) if fault.kind == FaultKind::UnknownTurn => {}
          Err(fault) => return Err(fault),
        }
      }

      while self.in_turn(session) {
        if ended.next().await.is_none() {
          return Err(fault(FaultKind::Busy, format!("{session} is in a turn whose end cannot be heard")));
        }
      }
    }

    self.end(session).await
  }

  /// Whether a worker's own session carries a turn of this process's now, whatever job does or does not
  /// name it.
  pub fn worker_in_turn(&self, worker: &str) -> bool {
    worker_session_id(worker).is_ok_and(|session| self.in_turn(&session))
  }

  // A run let go of its session's turns as it ends, and the words pushed into it with it.
  fn ended(&self, session: &SessionId, run: &str) {
    aside::forget(run);
    self.showing().remove(session);
    self.questions.clear(session);
    self.deaf().remove(session);

    let mut going = self.going();

    if let Some(runs) = going.get_mut(session)
      && let Some(at) = runs.iter().position(|going| going.run == run)
    {
      let ended = runs.remove(at);

      self.outputs.ended(session, ended.turn);
    }

    // The handoff turn ended: the turn that lent it the session takes it back.
    if let Some(parked) = self.parked().remove(session) {
      going.entry(session.clone()).or_default().extend(parked);
    }

    if going.get(session).is_some_and(|runs| runs.is_empty()) {
      going.remove(session);
    }
  }

  // The agent a session is asked for, where it can hold a session at that scope.
  async fn checked(&self, agent: &str, scope: &Scope) -> Result<AgentRow, Fault> {
    let ground = self.ground();
    let Some(row) = AgentRow::named(&ground.db, agent).await.map_err(store_fault)? else {
      return Err(fault(FaultKind::Unknown, format!("no agent is named {agent}")));
    };
    let conversational = crate::conversational::settings(&ground.db, agent).await.map_err(store_fault)?;

    match scope {
      Scope::Worker if row.kind == "worker" => Ok(row),
      Scope::Worker => Err(fault(FaultKind::Refused, format!("{agent} is not a worker"))),
      Scope::Room(room) if conversational.is_some() => room_number(room).map(|_| row),
      Scope::Room(_) => Err(fault(FaultKind::Refused, format!("{agent} is not a conversational agent"))),
      Scope::Agent
        if conversational
          .as_ref()
          .is_some_and(|settings| settings.scope == crate::conversational::Scope::Agent) =>
      {
        Ok(row)
      }
      Scope::Agent => Err(fault(FaultKind::Refused, format!("{agent} keeps no session across rooms"))),
      Scope::Conversation(_) if conversational.as_ref().is_some() => Ok(row),
      Scope::Conversation(_) if gate_work_key(agent) => Ok(row),
      Scope::Conversation(_) => {
        Err(fault(FaultKind::Refused, format!("{agent} is not a conversational agent")))
      }
      Scope::Apart(_) => Err(apart_refused(agent)),
    }
  }

  // The agent a session's scope stands on, where it stands on one; a thrown-away session stands on the
  // name its turns run under, which is no agent's row.
  async fn admitted(&self, agent: &str, scope: &Scope) -> Result<Option<AgentRow>, Fault> {
    match scope {
      // A role's run goes out on whichever provider its slot or its caller names, each a provider this build
      // carries and the owner has on (docs/provider-order.md, 「Choosing the provider」).
      Scope::Apart(apart) => {
        self.provider_ready(&apart.provider).await?;

        Ok(None)
      }
      Scope::Conversation(conversation) => {
        self.provider_ready(&conversation.provider).await?;

        let Some(row) = AgentRow::named(&self.ground().db, agent).await.map_err(store_fault)? else {
          return Err(fault(FaultKind::Unknown, format!("no agent is named {agent}")));
        };

        if crate::conversational::settings(&self.ground().db, agent)
          .await
          .map_err(store_fault)?
          .is_none()
        {
          return Err(fault(FaultKind::Refused, format!("{agent} is not a conversational agent")));
        }

        Ok(Some(row))
      }
      _ => self.checked(agent, scope).await.map(Some),
    }
  }

  // Whether a provider is one this build carries and the owner has switched on.
  async fn provider_ready(&self, provider: &str) -> Result<(), Fault> {
    if harness_providers::provider(provider).is_none() {
      return Err(fault(FaultKind::Refused, format!("no provider named {provider} in this build")));
    }

    if !crate::control::harness::provider_enabled(&self.ground().db, provider)
      .await
      .map_err(|why| fault(FaultKind::Refused, why.error))?
    {
      return Err(fault(FaultKind::Refused, format!("{provider} is switched off")));
    }

    Ok(())
  }

  // The supervisor's session in the room its turn goes on in: the one it holds, or a new generation where it
  // holds none, where the harness session it holds was opened on another provider or model or stands on a
  // place its registration no longer has, or where a generation that kept turns holds no harness session.
  async fn room_session(
    &self,
    agent: &str,
    chat_id: i64,
    provider: &'static dyn harness_providers::Provider,
    model: &str,
  ) -> Result<RoomSession, Refusal> {
    let ground = self.ground();
    let db = &ground.db;
    let Some(held) = sessions::session(db, agent, chat_id).await? else {
      return sessions::open_generation(db, agent, chat_id).await;
    };
    let moved = match &held.harness {
      None => false,
      Some(harness) => {
        harness.road != harness_road(provider.name(), model)
          || match (&harness.placement, provider.not_registered()) {
            (Some(node), Some(_)) => registration_named(ground, model)
              .await
              .map(|held| !held.holds_node(node))
              .unwrap_or(false),
            _ => false,
          }
      }
    };
    let unresumable =
      held.harness.is_none() && sessions::keeps_turns(db, agent, chat_id, held.generation).await?;

    if moved || unresumable {
      sessions::open_generation(db, agent, chat_id).await
    } else {
      Ok(held)
    }
  }

  // The handoff asked for where a chat-line turn is about to open a fresh session in place of one the agent
  // had, and what came of it said in the turn's run log (docs/sessions.md, 「A restart hands the session
  // over」); a turn that resumes its session asks nothing.
  async fn handed_off(
    &self,
    log: &mut RunLog,
    agent: &str,
    room: Option<i64>,
    resuming: bool,
    kept: bool,
    reason: &str,
  ) {
    let ground = self.ground();
    let standing = crate::lines::place::Place::of(&ground.roots.data, agent).program_alive().is_some();

    if !crate::handoff::replaces(kept, resuming, standing) {
      return;
    }

    // A failed run asks for a fresh session no more than once a minute: a turn that keeps failing must not
    // leave the room in a handoff loop of a row every ten seconds (docs/supervisors.md, 「A turn that goes
    // nowhere」).
    let key = format!("handoff_asked:{agent}:{}", room.unwrap_or(0));
    let now = now_ms();
    let asked_at = crate::queue::settings::get(&ground.db, &key)
      .await
      .ok()
      .flatten()
      .and_then(|at| at.parse::<i64>().ok());

    if asked_at.is_some_and(|at| now - at < 60_000) {
      return;
    }

    let _ = crate::queue::settings::set(&ground.db, &key, &now.to_string()).await;

    // The handoff a supervisor's one session across rooms is asked for is one more turn of that same session,
    // so the session is lent to it while this turn waits: the receiver finds the session by the state agency
    // answers for it, and that turn would otherwise be refused as a second turn of the session. A room's own
    // session is not lent, since the handoff turn there would open a generation of its own.
    let lent = room.is_none().then(|| SessionId(agent.to_owned()));

    if let Some(session) = &lent {
      self.handing().insert(session.clone());
    }

    let handed = crate::handoff::asked(ground.vars(), agent, reason, room).await;

    if let Some(session) = &lent {
      self.handing().remove(session);
    }

    log.write(&ground.db, Line::note(handed.said())).await;
  }

  // One turn of a supervisor in a room, carried by its resident program on its room's session.
  async fn room_turn(
    &self,
    agent: &str,
    chat_id: i64,
    run: &str,
    prompt: &str,
    context: &TurnContext,
    shown: OutputWatch<'_>,
  ) -> Result<Came, Fault> {
    let ground = self.ground();
    let db = ground.db.clone();
    let failed = |why: String| Ok(Came::Failed(why, None));
    let detail = |refusal: Refusal| refusal.detail;
    let Some(pinned) = pinned(ground, agent).await? else {
      return Err(fault(FaultKind::Unknown, format!("no agent is named {agent}")));
    };
    let (provider, model) = match choice_of(ground, &pinned).await {
      Ok(choice) => choice,
      Err(why) => return failed(why),
    };
    let began = context.began.unwrap_or_else(now_ms);
    let hold = &self.places.kept.reset;
    let keeping = hold.keeping().await;

    if overtaken::overtaken(&db, agent, began).await {
      return Ok(Came::Stopped);
    }

    let settings = Settings::of(&db, agent)
      .await
      .map_err(|refused| fault(FaultKind::Refused, refused.error))?;
    let system = match system_of(ground, &pinned, &settings, provider, &model).await {
      Ok(system) => system,
      Err(why) => return failed(why),
    };
    let mut log = RunLog::new(Target {
      agent: agent.to_owned(),
      run: run.to_owned(),
      chat_id: Some(chat_id),
      job_id: None,
    });

    if sessions::reset_waiting(&db, agent, chat_id).await.map_err(store_fault)? {
      if sessions::session(&db, agent, chat_id).await.map_err(store_fault)?.is_some() {
        sessions::open_generation(&db, agent, chat_id).await.map_err(store_fault)?;
      }

      sessions::settle_reset(&db, agent, chat_id).await.map_err(store_fault)?;
      log.write(&db, Line::note(SUPERVISOR_RUNNER.log.cleared.clone())).await;
    }

    let mut session = self
      .room_session(agent, chat_id, provider, &model)
      .await
      .map_err(|why| fault(FaultKind::Refused, detail(why)))?;
    let limits = &SUPERVISOR_RUNNER.limits;
    let guarded = provider.declared().resident.as_ref().is_some_and(|resident| resident.turn_limits);

    // A room session carried over its net by a turn that never ended opens a fresh generation rather
    // than judging every new turn over the net at once.
    if guarded && session.spent.calls >= limits.calls {
      session = sessions::open_generation(&db, agent, chat_id).await.map_err(store_fault)?;
      log.write(&db, Line::note(SUPERVISOR_RUNNER.log.cleared.clone())).await;
    }

    let hash = format!("{:x}", Sha256::digest(system.as_bytes()));
    let moved = sessions::prompt_moved(session.harness.is_some(), session.system_hash.as_deref(), &hash);

    drop(keeping);

    if moved {
      // The documents the session's prompt is composed from moved: the old session writes its handoff note
      // in one more turn, and the turn below answers on the generation that opens with it, composed afresh
      // from the new documents (docs/supervisors.md, 「A session in each room」).
      let due = Due {
        provider,
        model: model.clone(),
        system: system.clone(),
        generation: generation_of(&settings),
        session: session.harness.as_ref().map(|harness| harness.id.clone()).unwrap_or_default(),
        placement: session.harness.as_ref().and_then(|harness| harness.placement.clone()),
        ratio: 1.0,
        why: Why::Documents,
      };

      hand_over(ground, hold, &pinned.speaker, chat_id, due).await;
      session = sessions::session(&db, agent, chat_id)
        .await
        .map_err(store_fault)?
        .ok_or_else(|| fault(FaultKind::Unknown, "the room session to hand over is gone"))?;

      // A handoff that wrote nothing leaves the old generation in place; the turn still opens on a fresh
      // one, so it reads the documents that moved.
      if session.harness.is_some() {
        session = sessions::open_generation(&db, agent, chat_id).await.map_err(store_fault)?;
      }
    }

    let changed =
      moved || (session.system_hash.is_some() && session.system_hash.as_deref() != Some(hash.as_str()));

    if session.system_hash.as_deref() != Some(hash.as_str()) {
      session = sessions::mark_system(&db, agent, chat_id, &hash).await.map_err(store_fault)?;
    }

    if changed {
      log.write(&db, Line::note(SUPERVISOR_RUNNER.log.system_changed.clone())).await;

      if let Some(session) = id_of(agent, &Scope::Room(chat_id.to_string())) {
        ground.events.publish(SystemPromptChanged {
          session: session.0,
          run: run.to_owned(),
        });
      }
    }

    // A session opened by a handoff begins with the note its last generation wrote, ahead of the turn's
    // words.
    let prompt = match (&session.harness, &session.handoff) {
      (None, Some(note)) => format!("{note}\n\n{prompt}"),
      _ => prompt.to_owned(),
    };
    let generation = generation_of(&settings);

    let started = Instant::now();

    // A fresh generation in place of one the supervisor's program still stands on asks for its handoff first;
    // a prompt that moved already wrote one above.
    if !moved {
      self
        .handed_off(
          &mut log,
          agent,
          Some(chat_id),
          session.harness.is_some(),
          false,
          &HANDOFF.reasons.replaced,
        )
        .await;
    }

    let mut asked = self
      .room_run(
        &mut log,
        &pinned.speaker,
        provider,
        &model,
        &session,
        &system,
        &prompt,
        &generation,
        guarded,
        began,
        context,
        shown,
      )
      .await;

    // A session its harness no longer holds or cannot continue is let go, and the turn is asked again once
    // on a fresh one.
    if let Err(NoAnswer::Failed(said)) = &asked
      && session.harness.is_some()
      && lost_session(said)
    {
      session = sessions::open_generation(&db, agent, chat_id).await.map_err(store_fault)?;
      self
        .handed_off(&mut log, agent, Some(chat_id), false, false, &HANDOFF.reasons.lost)
        .await;
      asked = self
        .room_run(
          &mut log,
          &pinned.speaker,
          provider,
          &model,
          &session,
          &system,
          &aside::carried(run, &prompt),
          &generation,
          guarded,
          began,
          context,
          shown,
        )
        .await;
    }

    let _keeping = hold.keeping().await;
    let (reply, placement) = match asked {
      Ok(asked) if !overtaken::overtaken(&db, agent, began).await => asked,
      Ok(_) | Err(NoAnswer::Overtaken) => return Ok(Came::Stopped),
      Err(no) => {
        let (reason, engine) = failure_of(no);
        let _ = sessions::mark_spent(&db, agent, chat_id, Spent::default()).await;

        ground.events.publish(RoadWalked::of(
          chat_id,
          run,
          provider.name(),
          &model,
          false,
          started,
          &[],
          None,
        ));
        log
          .write(
            &db,
            Line {
              kind: "end",
              text: reason.clone(),
              state: Some("failed"),
              ..Line::default()
            },
          )
          .await;
        return Ok(Came::Failed(reason, engine));
      }
    };

    ground.events.publish(RoadWalked::of(
      chat_id,
      run,
      &reply.mode,
      &reply.model,
      true,
      started,
      &reply.spend,
      reply.context_ratio,
    ));
    self.record_usage(agent, &reply, run).await;
    sessions::record_turn(
      &db,
      &session,
      &[
        Kept {
          role: "user",
          text: &prompt,
        },
        Kept {
          role: "assistant",
          text: &reply.text,
        },
      ],
      Some(&Harness {
        id: reply.session_id.clone(),
        road: harness_road(provider.name(), &model),
        placement: placement.clone(),
      }),
      None,
    )
    .await
    .map_err(store_fault)?;
    log
      .write(
        &db,
        Line {
          kind: "end",
          text: SUPERVISOR_RUNNER.log.answered.clone(),
          state: Some("succeeded"),
          ..Line::default()
        },
      )
      .await;

    // A turn whose context reached its fold ratio hands its session over before the next one, where its
    // provider declares the handoff (docs/supervisors.md, 「Its limit」).
    let hands_off = provider
      .declared()
      .resident
      .as_ref()
      .is_some_and(|resident| resident.handover == policy::providers::Handover::Handoff);
    let mark = pinned.supervisor.fold_ratio;

    if let Some(ratio) = reply.context_ratio.filter(|ratio| hands_off && *ratio >= mark) {
      let due = Due {
        provider,
        model,
        system,
        generation,
        session: reply.session_id.clone(),
        placement,
        ratio,
        why: Why::Limit,
      };
      let (ground, hold, speaker) = (self.places.ground.clone(), hold.clone(), pinned.speaker);

      tokio::spawn(async move { hand_over(&ground, &hold, &speaker, chat_id, due).await });
    }

    Ok(Came::Answered(reply.text.clone(), spent_of(&reply.spend), left(&reply)))
  }

  #[allow(clippy::too_many_arguments)]
  async fn room_run(
    &self,
    log: &mut RunLog,
    speaker: &Speaker,
    provider: &'static dyn harness_providers::Provider,
    model: &str,
    session: &RoomSession,
    system: &str,
    prompt: &str,
    generation: &IndexMap<String, Value>,
    guarded: bool,
    began: i64,
    context: &TurnContext,
    shown: OutputWatch<'_>,
  ) -> Result<(Reply, Option<String>), NoAnswer> {
    let ground = self.ground();

    if let Some(reason) = spent_out(&SUPERVISOR_RUNNER.limits, session.spent).filter(|_| guarded) {
      return Err(NoAnswer::Cut(reason));
    }

    let resume = session
      .harness
      .as_ref()
      .map(|harness| (harness.id.as_str(), harness.placement.as_deref()));
    let run = log.run().to_owned();
    // A registration whose engine the arbiter stands is asked a card first, held for the turn and given
    // back at its end (docs/own-llm.md, 「An engine the arbiter stands」).
    let card = crate::cards::hold_for_model(
      ground,
      provider.name(),
      model,
      &speaker.name,
      resume.map(|(id, _)| id),
      log,
      Some(shown),
    )
    .await
    .map_err(|refused| NoAnswer::Failed(refused.detail))?;
    let watching = card.as_ref().map(|card| crate::cards::watch_turn(ground, card, &run));
    let turned = async {
      let composed = compose(
        ground,
        &ground.machine(),
        &SupervisorTurn {
          agent: &speaker.name,
          chat_id: session.chat_id,
          provider: provider.name(),
          model,
          system,
          prompt,
          privileged: context.privileged,
          resume,
          card: card.as_ref().map(|card| card.node.as_str()),
          run: &run,
          answers: &context.answers,
          generation,
          began,
        },
      )
      .await
      .map_err(|refused| NoAnswer::Failed(refused.detail))?;

      log.launched(&ground.db, Some(&composed.system_path), prompt, &[]).await;

      let tally = Tally {
        agent: &speaker.name,
        chat_id: session.chat_id,
        carried: session.spent,
      };
      let reply = run_turn(
        ground,
        &speaker.name,
        crate::lines::chat::LINE,
        provider.name(),
        &composed,
        resume.map(|(id, _)| id),
        log,
        guarded,
        Some(&tally),
        Some(shown),
      )
      .await?;

      Ok((reply, composed.placement))
    }
    .await;

    if let Some(watching) = watching {
      watching.finish(ground).await;
    }

    if let Some(card) = &card {
      crate::cards::give_back(ground, card).await;
    }

    turned
  }

  // One turn of an agent across rooms, carried by its resident program on the chat line around the room
  // block, the road and the base persona, on the one session its row keeps; a session
  // its harness no longer holds is let go, and the turn asked again once on a fresh one.
  async fn agent_turn(
    &self,
    agent: &str,
    run: &str,
    prompt: &str,
    context: &TurnContext,
    shown: OutputWatch<'_>,
    handoff: bool,
  ) -> Result<Came, Fault> {
    let ground = self.ground();
    let db = ground.db.clone();
    let Some(room) = &context.room else {
      return Err(fault(
        FaultKind::Refused,
        format!("{agent}: a turn across rooms names the room it answers"),
      ));
    };
    let began = context.began.unwrap_or_else(now_ms);
    let mut log = RunLog::new(Target {
      agent: agent.to_owned(),
      run: run.to_owned(),
      chat_id: Some(room.chat_id),
      job_id: None,
    });
    let mut held = agent_session(&db, agent).await.map_err(store_fault)?;
    let last_mode = agent_last_mode(&db, agent).await.map_err(store_fault)?;
    // A session the last answer left on a stand-in is resumed on that road; one left on Claude Code is
    // Claude's, and the stand-in opens a session of its own.
    let stand_in_resume = if last_mode.as_deref() == Some(CHAT_PROVIDER) { None } else { held.clone() };

    // A reset the owner asked of the supervisor's one session closes it before this turn opens a new one,
    // and the session is asked for its handoff while agency still names it, as before any fresh session
    // replaces one the agent had (docs/session-resets.md, 「Who holds a session」).
    if !handoff && sessions::reset_waiting_agent(&db, agent).await.map_err(store_fault)? {
      self.handed_off(&mut log, agent, None, false, true, &HANDOFF.reasons.replaced).await;
      keep_agent_session(&db, agent, None).await.map_err(store_fault)?;
      sessions::settle_reset_agent(&db, agent).await.map_err(store_fault)?;
      held = None;
      log
        .write(&db, Line::note(policy::supervisor_runner::SUPERVISOR_RUNNER.log.cleared.clone()))
        .await;
    }

    let started = Instant::now();
    // The agent's key walked first: a stand-in that answers carries the turn, and Claude Code's session
    // stays as it stands for its next turn. The run is the chat line's; the fallback walk and roads are
    // the one mechanism every line crosses over on (docs/cache-fallback.md).
    let mut line_run = match crate::lines::chat::chat_run(
      ground,
      &ground.machine(),
      &launch_of(agent, room, context, stand_in_resume),
      prompt,
      false,
    )
    .await
    {
      Ok(line_run) => line_run,
      Err(why) => return Ok(Came::Failed(why.detail, None)),
    };
    let hash = line_run.system_hash.clone();
    let recorded = sessions::agent_system_hash(&db, agent).await.map_err(store_fault)?;
    // The receiver's handoff turn is one more turn of the session whose prompt moved: it reads the prompt the
    // session holds, so it is no second move.
    let moved = !handoff && sessions::prompt_moved(held.is_some(), recorded.as_deref(), &hash);

    if moved {
      // The documents the prompt is composed from moved under a session the agent already had: the standing
      // program read the old prompt, so it is asked for its handoff while agency still names its session, and
      // then let go, and this answer goes out on a fresh session composed from the new documents
      // (docs/supervisors.md, 「A session in each room」).
      self.handed_off(&mut log, agent, None, false, true, &HANDOFF.reasons.moved).await;

      keep_agent_session(&db, agent, None).await.map_err(store_fault)?;
      held = None;
      line_run.resume = None;
      log.write(&db, Line::note(SUPERVISOR_RUNNER.log.system_changed.clone())).await;

      if let Some(session) = id_of(agent, &Scope::Agent) {
        ground.events.publish(SystemPromptChanged {
          session: session.0,
          run: run.to_owned(),
        });
      }
    }

    sessions::mark_agent_system(&db, agent, &hash).await.map_err(store_fault)?;

    let stood_in =
      stand_in_turn(ground, &mut log, &line_run, agent, &context.answers, began, Some(shown)).await;
    let on_claude = matches!(stood_in, Ok(None));
    let asked = match stood_in {
      Ok(Some(reply)) => Ok(reply),
      Err(no) => Err(no),
      Ok(None) => {
        // A fresh session in place of one the agent's program still stands on asks for its handoff first.
        if !moved {
          self
            .handed_off(&mut log, agent, None, held.is_some(), false, &HANDOFF.reasons.replaced)
            .await;
        }

        let mut asked =
          self.agent_run(&mut log, agent, room, context, prompt, held.clone(), began, shown).await;

        if let Err(NoAnswer::Failed(said)) = &asked
          && held.is_some()
          && lost_session(said)
        {
          // The session the turn could not resume is asked for its handoff while agency still names it,
          // then let go.
          self.handed_off(&mut log, agent, None, false, false, &HANDOFF.reasons.lost).await;
          keep_agent_session(&db, agent, None).await.map_err(store_fault)?;
          asked = self
            .agent_run(&mut log, agent, room, context, &aside::carried(run, prompt), None, began, shown)
            .await;
        }

        asked
      }
    };

    let reply = match asked {
      Ok(reply) => reply,
      Err(NoAnswer::Overtaken) => return Ok(Came::Stopped),
      Err(no) => {
        let (reason, engine) = failure_of(no);
        let model = crate::order::supervisor_entry(&db, agent, CHAT_PROVIDER)
          .await
          .map(|entry| entry.model)
          .unwrap_or_default();

        ground.events.publish(RoadWalked::of(
          room.chat_id,
          run,
          CHAT_PROVIDER,
          &model,
          false,
          started,
          &[],
          None,
        ));
        log
          .write(
            &db,
            Line {
              kind: "end",
              text: reason.clone(),
              state: Some("failed"),
              ..Line::default()
            },
          )
          .await;

        return Ok(Came::Failed(reason, engine));
      }
    };

    ground.events.publish(RoadWalked::of(
      room.chat_id,
      run,
      &reply.mode,
      &reply.model,
      true,
      started,
      &reply.spend,
      reply.context_ratio,
    ));
    self.record_usage(agent, &reply, run).await;

    if !reply.session_id.is_empty() {
      keep_agent_session(&db, agent, Some((&reply.session_id, room.chat_id)))
        .await
        .map_err(store_fault)?;
    }

    if on_claude {
      fallback_ended(ground).await;
    }

    log
      .write(
        &db,
        Line {
          kind: "end",
          text: SUPERVISOR_RUNNER.log.answered.clone(),
          state: Some("succeeded"),
          ..Line::default()
        },
      )
      .await;

    Ok(Came::Answered(reply.text.clone(), spent_of(&reply.spend), left(&reply)))
  }

  // One turn of a conversation the caller opened, on the provider and model its scope names: the chat
  // line on Claude Code, with the agent's own documents and thinking, or the stand-in road the provider
  // declares. The session the last answered turn left is resumed where the road keeps one, and one the
  // program no longer holds is let go and the turn asked again on a fresh one.
  async fn conversation_turn(
    &self,
    turn: &Turn,
    agent: &str,
    conversation: &Conversation,
    run: &str,
    shown: OutputWatch<'_>,
  ) -> Result<Came, Fault> {
    let ground = self.ground();
    let began = turn.context.began.unwrap_or_else(now_ms);
    let room = turn.context.room.clone().unwrap_or(RoomBlock {
      chat_id: 0,
      text: String::new(),
      mark: String::new(),
    });
    let held = self.held_harness(&turn.session);
    let launch = launch_on(
      agent,
      &room,
      &turn.context,
      held.clone(),
      Some(&conversation.model),
      Some(&conversation.cwd),
    );
    let mut log = RunLog::new(Target {
      agent: agent.to_owned(),
      run: run.to_owned(),
      chat_id: Some(room.chat_id),
      job_id: None,
    });
    let started = Instant::now();
    let mut asked = self
      .conversation_asked(&mut log, agent, &launch, conversation, turn, began, shown)
      .await;

    if let Err(NoAnswer::Failed(said)) = &asked
      && held.is_some()
      && lost_session(said)
    {
      let fresh =
        launch_on(agent, &room, &turn.context, None, Some(&conversation.model), Some(&conversation.cwd));
      asked = self.conversation_asked(&mut log, agent, &fresh, conversation, turn, began, shown).await;
    }

    let reply = match asked {
      Ok(reply) => reply,
      Err(NoAnswer::Overtaken) => return Ok(Came::Stopped),
      Err(no) => {
        let (reason, engine) = failure_of(no);

        ground.events.publish(RoadWalked::of(
          room.chat_id,
          run,
          &conversation.provider,
          &conversation.model,
          false,
          started,
          &[],
          None,
        ));
        log
          .write(
            &ground.db,
            Line {
              kind: "end",
              text: reason.clone(),
              state: Some("failed"),
              ..Line::default()
            },
          )
          .await;

        return Ok(Came::Failed(reason, engine));
      }
    };

    ground.events.publish(RoadWalked::of(
      room.chat_id,
      run,
      &reply.mode,
      &reply.model,
      true,
      started,
      &reply.spend,
      reply.context_ratio,
    ));
    self.record_usage(agent, &reply, run).await;
    self.keep_conversation_harness(&turn.session, &conversation.provider, &reply);
    log
      .write(
        &ground.db,
        Line {
          kind: "end",
          text: SUPERVISOR_RUNNER.log.answered.clone(),
          state: Some("succeeded"),
          ..Line::default()
        },
      )
      .await;

    Ok(Came::Answered(reply.text.clone(), spent_of(&reply.spend), left(&reply)))
  }

  // One gate's turn of a conversation: the gate's instructions as the program's whole system prompt, on the
  // model and effort its mapping names, with no tools, no MCP server and the thinking the gate asks for.
  #[allow(clippy::too_many_arguments)]
  async fn gate_turn(
    &self,
    turn: &Turn,
    agent: &str,
    provider: &str,
    cwd: &str,
    gate: &GateAsk,
    run: &str,
    shown: OutputWatch<'_>,
  ) -> Result<Came, Fault> {
    let ground = self.ground();
    let machine = ground.machine();
    let began = turn.context.began.unwrap_or_else(now_ms);
    let chat_id = turn.context.room.as_ref().map_or(0, |room| room.chat_id);
    let mut log = RunLog::new(Target {
      agent: agent.to_owned(),
      run: run.to_owned(),
      chat_id: turn.context.room.as_ref().map(|room| room.chat_id),
      job_id: None,
    });
    let mut prepared = gate_run(ground, agent, gate, &turn.prompt, cwd)
      .map_err(|refused| fault(FaultKind::Refused, refused.detail))?;
    // On a road whose program carries its own turns the turn stands in a place kept for its program
    // (docs/agent-control.md, 「A gate's places」), its run's records staying the agent's.
    let seat = if provider != CHAT_PROVIDER && crate::chat_stand_in::inbox_declared(provider) {
      let program = gate_places::Asked {
        agent,
        provider,
        model: &prepared.model,
        effort: &prepared.effort,
        thinking: prepared.thinking,
        system: gate.system.as_deref().unwrap_or_default(),
        cwd,
      }
      .key();
      let seat = self.places.kept.gate_places.take(ground, agent, &program).await;

      prepared.agent = seat.name.clone();
      Some(seat)
    } else {
      None
    };
    let started = Instant::now();
    let asked = if provider == CHAT_PROVIDER {
      let composed =
        compose_prepared(ground, &machine, &prepared, &turn.prompt, run, &turn.context.answers, began)
          .await;

      match composed {
        Ok(composed) => {
          log.launched(&ground.db, Some(&composed.system_path), &turn.prompt, &[]).await;

          run_turn(
            ground,
            agent,
            crate::lines::chat::LINE,
            CHAT_PROVIDER,
            &composed,
            None,
            &mut log,
            false,
            None,
            Some(shown),
          )
          .await
        }
        Err(refused) => Err(NoAnswer::Failed(refused.detail)),
      }
    } else {
      crate::chat_stand_in::gate_turn_on(
        ground,
        &mut log,
        &prepared,
        provider,
        &turn.context.answers,
        began,
        gate,
        Some(shown),
      )
      .await
    };

    let reply = match asked {
      Ok(reply) => reply,
      Err(NoAnswer::Overtaken) => return Ok(Came::Stopped),
      Err(no) => {
        let (reason, engine) = failure_of(no);

        ground.events.publish(RoadWalked::of(
          chat_id,
          run,
          provider,
          &prepared.model,
          false,
          started,
          &[],
          None,
        ));
        log
          .write(
            &ground.db,
            Line {
              kind: "end",
              text: reason.clone(),
              state: Some("failed"),
              ..Line::default()
            },
          )
          .await;

        return Ok(Came::Failed(reason, engine));
      }
    };

    ground.events.publish(RoadWalked::of(
      chat_id,
      run,
      &reply.mode,
      &reply.model,
      true,
      started,
      &reply.spend,
      reply.context_ratio,
    ));
    self.record_usage(agent, &reply, run).await;
    log
      .write(
        &ground.db,
        Line {
          kind: "end",
          text: SUPERVISOR_RUNNER.log.answered.clone(),
          state: Some("succeeded"),
          ..Line::default()
        },
      )
      .await;

    // A session past its bound starts afresh once the answer is on its way, its place held until it has.
    let held: i64 = reply
      .spend
      .iter()
      .map(|spent| spent.input_tokens + spent.cache_read_tokens + spent.output_tokens)
      .sum();

    if let Some(seat) = seat
      && held >= GATE_PLACES.session_tokens
    {
      tokio::spawn(seat.start_afresh());
    }

    Ok(Came::Answered(reply.text.clone(), spent_of(&reply.spend), left(&reply)))
  }

  // One turn of a conversation on the road its launch names: Claude Code's chat line on the deployment's
  // own program, or the stand-in road the provider declares.
  #[allow(clippy::too_many_arguments)]
  async fn conversation_asked(
    &self,
    log: &mut RunLog,
    agent: &str,
    launch: &ChatLaunch<'_>,
    conversation: &Conversation,
    turn: &Turn,
    began: i64,
    shown: OutputWatch<'_>,
  ) -> Result<Reply, NoAnswer> {
    if conversation.provider == CHAT_PROVIDER {
      return self
        .agent_run_launched(log, agent, launch, &turn.prompt, &turn.context.answers, began, shown)
        .await;
    }

    crate::chat_stand_in::turn_on(
      self.ground(),
      log,
      launch,
      &conversation.provider,
      &conversation.model,
      &turn.prompt,
      &turn.context.answers,
      began,
      Some(shown),
    )
    .await
  }

  // The session a conversation's next turn on the provider named would resume, none where its road keeps
  // none.
  fn held_harness(&self, session: &SessionId) -> Option<String> {
    self.conversations().get(session).and_then(|(_, held)| held.harness.clone())
  }

  // The session a conversation's answered turn left, kept for its next turn where its road keeps one.
  fn keep_conversation_harness(&self, session: &SessionId, provider: &str, reply: &Reply) {
    let keeps = harness_providers::provider(provider)
      .and_then(|declared| declared.declared().resident.as_ref())
      .is_some_and(|resident| resident.turns != policy::providers::Turns::Inbox);

    if !keeps {
      return;
    }

    if let Some((_, held)) = self.conversations().get_mut(session) {
      held.harness = Some(reply.session_id.clone());
    }
  }

  #[allow(clippy::too_many_arguments)]
  async fn agent_run(
    &self,
    log: &mut RunLog,
    agent: &str,
    room: &RoomBlock,
    context: &TurnContext,
    prompt: &str,
    resume: Option<String>,
    began: i64,
    shown: OutputWatch<'_>,
  ) -> Result<Reply, NoAnswer> {
    let launch = launch_of(agent, room, context, resume);
    self
      .agent_run_launched(log, agent, &launch, prompt, &context.answers, began, shown)
      .await
  }

  // One chat-line turn on the launch as it stands, which names the road its run goes out on.
  #[allow(clippy::too_many_arguments)]
  async fn agent_run_launched(
    &self,
    log: &mut RunLog,
    agent: &str,
    launch: &ChatLaunch<'_>,
    prompt: &str,
    answers: &[i64],
    began: i64,
    shown: OutputWatch<'_>,
  ) -> Result<Reply, NoAnswer> {
    let ground = self.ground();
    let composed = compose_turn(ground, &ground.machine(), launch, prompt, log, answers, began)
      .await
      .map_err(|refused| NoAnswer::Failed(refused.detail))?;

    log.launched(&ground.db, Some(&composed.system_path), prompt, &[]).await;

    run_turn(
      ground,
      agent,
      crate::lines::chat::LINE,
      CHAT_PROVIDER,
      &composed,
      launch.resume.as_deref(),
      log,
      false,
      None,
      Some(shown),
    )
    .await
  }

  // What a supervisor's run spent is its own: its last answer written and its usage published.
  async fn record_usage(&self, agent: &str, reply: &Reply, run: &str) {
    let ground = self.ground();

    if let Err(why) =
      note_answer(&ground.db, agent, &reply.mode, Some(&reply.model), reply.effort.as_deref()).await
    {
      eprintln!("supervisor {agent}: its last answer was not written: {}", why.detail);
    }

    let account = slot_account(&reply.mode, reply.account.as_deref());

    if let Some(spent) =
      UsageSpent::of(agent, Some(run), now_ms() / 1000, &reply.spend, |_| account.clone())
    {
      ground.events.publish(spent);
    }
  }

  // A turn's end published as `turn-ended`, before its send is answered with it.
  // One turn sent into a session and carried to its end.
  async fn sent(&self, mut turn: Turn) -> Result<TurnEnd, Fault> {
    let (agent, scope) = self.scope_held(&turn.session)?;

    // There is no worker scope: a run in a repository is a role's session on its slot (docs/api.md,
    // 「sessions」).
    if matches!(scope, Scope::Worker) {
      return Err(no_worker_scope(&turn.session));
    }

    let row = self.admitted(&agent, &scope).await?;
    let at = now_ms();
    let room = match &scope {
      Scope::Room(room) => Some(room.clone()),
      Scope::Agent => turn.context.room.as_ref().map(|room| room.chat_id.to_string()),
      Scope::Worker | Scope::Apart(_) | Scope::Conversation(_) => None,
    };

    // A job's turn goes out under the run name its runner gives it, on Claude Code alone; a thrown-away
    // session carries nothing else.
    let work = match scope {
      Scope::Worker | Scope::Apart(_) => turn.context.work.as_ref(),
      _ => None,
    };

    // A gate's turn carries the gate's ask instead of work, and goes out on the gate road.
    if matches!(scope, Scope::Apart(_)) && work.is_none() && turn.context.gate.is_none() {
      return Err(fault(
        FaultKind::Refused,
        format!("{}: a thrown-away session's turn names its work parts", turn.session),
      ));
    }

    if matches!(scope, Scope::Conversation(_)) && turn.context.work.is_some() {
      return Err(fault(
        FaultKind::Refused,
        format!("{}: a conversation's turn carries no work parts", turn.session),
      ));
    }

    // A thrown-away session's first prompt carries the reference the gate picks over its instruction; a
    // co-work branch's prompt is composed by its parent and carries none, and a gate's turn is a
    // judgement of its own. A session whose pick fails keeps its own words and nothing else, and a later
    // turn of the same session is asked over the reference its first prompt already carries.
    if let Scope::Apart(apart) = &scope
      && !(apart.todo.is_some() && apart.role.is_some())
      && turn.context.gate.is_none()
      && turn.context.job.is_none()
      && self.referenced().insert(turn.session.clone())
    {
      let lines: Vec<String> = turn.prompt.lines().map(str::to_owned).collect();

      match self.ground().peers.gate.reference(lines).await {
        Ok(entries) => turn.prompt = crate::peers::gate::attach(&turn.prompt, &entries),
        Err(why) => eprintln!("{}: the gate's reference was not picked: {why}", turn.session),
      }
    }

    let run = match (&room, work) {
      (_, Some(work)) => work.name.clone(),
      (Some(room), None) => {
        fill_marks(&SUPERVISOR_RUNNER.runs.turn, &[("chat", room.as_str()), ("at", &at.to_string())])
      }
      (None, None) => format!("turn:{agent}:{at}"),
    };

    let turn_id;
    // A session its turn lent the receiver's handoff turn takes that turn now; the turn that lent it stands
    // waiting and takes the session back when the handoff turn ends.
    let lent = self.handing().remove(&turn.session);

    {
      let mut going = self.going();
      let runs = going.entry(turn.session.clone()).or_default();

      if !runs.is_empty() && !lent {
        return Err(fault(FaultKind::Busy, format!("{} is in a turn", turn.session)));
      }

      if lent {
        let parked = std::mem::take(runs);

        self.parked().insert(turn.session.clone(), parked);
      }

      turn_id = self.next_turn();
      runs.push(Running {
        turn: turn_id,
        run: run.clone(),
      });
    }

    self.outputs.begun(&turn.session, turn_id, &run);

    // A chat turn's run stands in the ledger from its start: a turn into a room that is no job's and no
    // gate's.
    let ledger = match (&room, work, &turn.context.gate) {
      (Some(room), None, None) => {
        open_chat_run(&self.ground().db, &agent, room, &run, &turn.context, at).await
      }
      _ => None,
    };

    if !hears_words(work) {
      self.deaf().insert(turn.session.clone());
    }

    // The turn is kept while it goes, so a server started after this one goes down hands its end back to
    // the caller (docs/sessions.md, 「A restart hands the session over」).
    crate::turn_pickup::keep(
      &self.ground().db,
      &crate::turn_pickup::InFlight {
        session: turn.session.0.clone(),
        turn: turn_id,
        run: run.clone(),
        agent: agent.clone(),
        chat_id: room.as_deref().and_then(|room| room.parse().ok()),
        ledger,
        job: turn.context.job.as_ref().map(|job| job.job),
        started_at: at,
      },
    )
    .await;

    // Carried by this process until its end, which closes it; the queue's pass leaves it to that end.
    let _carrying = crate::turn_pickup::Carrying::of(&run);

    // A room turn of a conversational agent is what an item it files is filed under.
    if let (Some(room), None, None) = (&room, work, &turn.context.gate) {
      crate::turn_marks::carrying(
        &agent,
        &run,
        crate::turn_marks::Mark {
          chat_id: room.parse().unwrap_or_default(),
          privileged: turn.context.privileged,
        },
      );
    }

    self.ground().events.publish(TurnStarted {
      session: turn.session.0.clone(),
      run: run.clone(),
      turn_id,
      at,
    });

    let shown = OutputWatch {
      outputs: &self.outputs,
      session: &turn.session,
      turn: turn_id,
    };
    let came = match (&scope, &row) {
      (Scope::Apart(apart), _) => match (&turn.context.gate, work) {
        (Some(gate), _) => {
          self.gate_turn(&turn, &agent, &apart.provider, &apart.cwd, gate, &run, shown).await
        }
        (None, Some(work)) => {
          let db = &self.ground().db;

          if turn.context.job.is_some() {
            keep_worker_job(db, &agent, turn.context.job).await;
          }

          let came = if apart.provider == worker_session::PROVIDER {
            self.carried(&turn, &agent, WorkSeat::Apart(apart), work, turn_id).await
          } else {
            let began = turn.context.began.unwrap_or(at);

            self
              .apart_in_place(
                &turn.session,
                &agent,
                apart,
                work,
                &turn.prompt,
                began,
                turn.context.job,
                shown,
              )
              .await
          };

          if turn.context.job.is_some() {
            keep_worker_job(db, &agent, None).await;
          }

          came
        }
        (None, None) => Err(apart_refused(&agent)),
      },
      (Scope::Conversation(conversation), _) => match &turn.context.gate {
        Some(gate) => {
          self
            .gate_turn(&turn, &agent, &conversation.provider, &conversation.cwd, gate, &run, shown)
            .await
        }
        None => self.conversation_turn(&turn, &agent, conversation, &run, shown).await,
      },
      (Scope::Worker, _) => Err(no_worker_scope(&turn.session)),
      (Scope::Room(room), _) => match room_number(room) {
        Ok(chat_id) => self.room_turn(&agent, chat_id, &run, &turn.prompt, &turn.context, shown).await,
        Err(refused) => Err(refused),
      },
      (Scope::Agent, _) => self.agent_turn(&agent, &run, &turn.prompt, &turn.context, shown, lent).await,
    };

    self.ended(&turn.session, &run);
    crate::turn_marks::ended(&agent, &run);
    crate::turn_pickup::forget(&self.ground().db, &run, "its end reached its caller").await;

    // A turn cut because its session's slot was given back says so.
    let cut = self.cut.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&run);

    // A turn refused once its start was published still ends, failed with the refusal.
    match came {
      Ok(came) => {
        let spent = came.spent();
        let context = came.context();

        if let (Some(context), Scope::Room(_) | Scope::Agent) = (context, &scope) {
          self
            .left
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(turn.session.clone(), context);
        }

        let mut end = came.end(&run);

        end.turn = turn_id;

        if end.outcome != Outcome::Answered && end.reason.is_none() {
          end.reason = cut;
        }

        close_chat_run(&self.ground().db, ledger, &end, spent, context).await;

        Ok(self.published(&turn.session, end))
      }
      Err(refused) => {
        let mut end = TurnEnd::failed(&run, refused.message.clone(), None);

        end.turn = turn_id;
        close_chat_run(&self.ground().db, ledger, &end, None, None).await;
        self.published(&turn.session, end);
        Err(refused)
      }
    }
  }

  // A turn's end published, its `ending` the word the door's last turn under the send ended on where the
  // turn did not answer, else its outcome's own.
  fn published(&self, session: &SessionId, mut end: TurnEnd) -> TurnEnd {
    // A turn stopped because its card was taken says so, whatever its program made of the stop.
    if let Some(by) = crate::cards::taken_from(&end.run) {
      end.ending = Some(crate::cards::CARD_TAKEN.to_owned());
      end.reason = Some(format!("the card was taken by {by}"));
    }

    if end.ending.is_none() {
      end.ending = Some(ending_of(&end).to_owned());
    }

    self.ground().events.publish(TurnEnded {
      session: session.0.clone(),
      end: end.clone(),
    });

    end
  }

  // A state given the turn its session carries now and what that turn went out on.
  fn going_shown(&self, session: &SessionId, state: &mut State) {
    state.run = match self.going().get(session).map(Vec::as_slice) {
      Some([run]) => Some(run.run.clone()),
      _ => None,
    };
    state.went_out = self.showing().get(session).cloned();
    state.question = self.questions.shown(session);
  }

  // A job's turn carried on the seat given, what it went out on and what it writes shown to a reading of its
  // session while it goes.
  async fn carried(
    &self,
    turn: &Turn,
    agent: &str,
    seat: WorkSeat<'_>,
    work: &WorkParts,
    turn_id: i64,
  ) -> Result<Came, Fault> {
    let asked = WorkAsked {
      ground: self.ground(),
      agent,
      seat,
      work,
      job: turn.context.job,
      prompt: &turn.prompt,
      session: &turn.session,
      questions: &self.questions,
    };
    let shown = Showing {
      sessions: self,
      session: &turn.session,
      turn: turn_id,
    };

    carry(&asked, &shown).await.map(|end| Came::Ended(Box::new(end)))
  }

  // The session a run in a place of its own kept, folded where its turn left it at or past its role's
  // compaction threshold (docs/sessions.md, 「Session compaction」): one more turn on the run's own road into the
  // same session, the fold word, before the run gives its slot back. A fold that answered leaves the session
  // kept and valid with its context unread; one that did not leaves it as it stood.
  #[allow(clippy::too_many_arguments)]
  async fn fold_in_place(
    &self,
    run: &crate::lines::chat::ChatRun,
    role: &str,
    taken: &crate::road::Road,
    session: &str,
    share: Option<f64>,
    log: &mut RunLog,
    supervisor: Option<&str>,
  ) {
    let ground = self.ground();
    let held = match crate::apart_sessions::take_for_fold(&ground.db, role, session, share).await {
      Ok(Some(held)) => held,
      Ok(None) => return,
      Err(why) => {
        eprintln!("{session}: whether it is due a fold could not be read: {why:?}");
        return;
      }
    };
    let fold = crate::lines::chat::ChatRun {
      prompt: crate::worker_session::fold::FOLD_WORD.to_owned(),
      fold: true,
      resume: Some(held.clone()),
      ..run.clone()
    };
    // The fold reached the session only where it answered on that same session.
    let folded =
      crate::chat_stand_in::job_turn_on(ground, log, &fold, role, taken, now_ms(), None, supervisor)
        .await
        .is_ok_and(|reply| reply.session_id == held);

    ground.events.publish(crate::worker_session::told::SessionFolded {
      session: held.clone(),
    });

    let given = if folded {
      crate::apart_sessions::folded(&ground.db, &held, now_ms()).await
    } else {
      eprintln!("compact {role} {held}: the fold did not answer, the session stands as it was");
      crate::apart_sessions::give_back(&ground.db, &held).await
    };

    if let Err(why) = given {
      eprintln!("{session}: the session it folded was not given back: {why:?}");
    }
  }

  // One turn of a role's session on a provider other than Claude Code's own, in the place of its own
  // (docs/api.md, 「sessions」): the runner line it names with the instructions its caller handed in, or else
  // the worker line opening on the platform's harness and the role's prompt, with the repository's own where
  // its folder is a repository's checkout; on the session the role keeps at that folder on that provider and
  // model where it keeps sessions and one is valid, a new one otherwise; the role's bundles and the record its
  // caller named on its token.
  #[allow(clippy::too_many_arguments)]
  async fn apart_in_place(
    &self,
    session: &SessionId,
    agent: &str,
    apart: &Apart,
    work: &WorkParts,
    prompt: &str,
    began: i64,
    job: Option<agent_session::JobMark>,
    shown: OutputWatch<'_>,
  ) -> Result<Came, Fault> {
    let ground = self.ground();
    let trees = Trees::of(ground);
    let place = crate::apart_places::place_name(session);
    let role = apart.role.as_deref().unwrap_or_else(|| crate::agent_roles::job_role());
    let refused = |why: String| fault(FaultKind::Refused, why);
    let composed = crate::lines::role::compose(ground, &trees, apart, role, &work.mapping)
      .await
      .map_err(|why| refused(why.detail))?;
    let mut env = ground.env().clone();

    crate::apart_places::mark(ground, &place, session).map_err(refused)?;
    env.extend(work.variables.iter().map(|(name, value)| (name.clone(), value.clone())));
    env.extend(crate::lines::token::record_variables(apart));
    // A supervisor's double runs as a turn of that supervisor, so every tool and script acts as it.
    env.insert(TURN_AGENT.to_owned(), apart.supervisor.clone().unwrap_or_else(|| agent.to_owned()));

    if job.is_some() {
      env.insert(TASK_LOCKED.to_owned(), "1".to_owned());
    }

    // The session the role keeps at this folder on this provider and model, where it keeps sessions.
    let key = if crate::apart_sessions::setting(&ground.db, role).await.keeps() {
      Some(crate::apart_sessions::SessionKey {
        role: role.to_owned(),
        path: composed.cwd.clone(),
        provider: apart.provider.clone(),
        model: apart.model.clone(),
        effort: composed.effort.clone(),
        account: None,
        command: crate::apart_sessions::key_of(
          composed.line,
          &composed.cwd,
          &composed.system_path,
          &composed.mcp_config,
          &composed.tools_allowed,
        ),
      })
    } else {
      None
    };
    let resume = match &key {
      Some(key) => crate::apart_sessions::take(&ground.db, key, now_ms()).await.unwrap_or_else(|why| {
        eprintln!("{session}: its kept session could not be read, so it opens a new one: {why:?}");
        None
      }),
      None => None,
    };
    let chat_run = composed.run(crate::lines::role::Turned {
      place: place.clone(),
      model: apart.model.clone(),
      prompt: prompt.to_owned(),
      resume: resume.clone(),
      env,
      cards: crate::cards::Ask {
        own: apart.demand,
        within: apart.within,
        terms: crate::cards::terms(apart.card_rank, apart.card_pool.clone()),
      },
    });
    let mut log = RunLog::new(Target {
      agent: agent.to_owned(),
      run: work.name.clone(),
      chat_id: (work.chat != 0).then_some(work.chat),
      job_id: job.map(|job| job.job),
    });
    let taken = crate::road::Road {
      provider: apart.provider.clone(),
      model: apart.model.clone(),
      account: None,
    };
    let opened_at = now_ms();
    let came = crate::chat_stand_in::job_turn_on(
      ground,
      &mut log,
      &chat_run,
      role,
      &taken,
      began,
      Some(shown),
      apart.supervisor.as_deref(),
    )
    .await;

    if let Some(key) = &key {
      let kept = match &came {
        Ok(reply) if !reply.session_id.is_empty() => {
          let held = crate::apart_sessions::Answered {
            at: now_ms(),
            opened_at,
            context_tokens: reply.context_tokens,
            context_share: reply.context_ratio,
          };

          crate::apart_sessions::keep(&ground.db, key, &reply.session_id, held).await
        }
        Ok(_) => Ok(()),
        Err(_) => match &resume {
          Some(held) => crate::apart_sessions::give_back(&ground.db, held).await,
          None => Ok(()),
        },
      };

      if let Err(why) = kept {
        eprintln!("{session}: the session it ran on was not kept: {why:?}");
      }

      if let Ok(reply) = &came
        && !reply.session_id.is_empty()
      {
        self
          .fold_in_place(
            &chat_run,
            role,
            &taken,
            &reply.session_id,
            reply.context_ratio,
            &mut log,
            apart.supervisor.as_deref(),
          )
          .await;
      }
    }

    match came {
      Ok(reply) => Ok(Came::Answered(reply.text.clone(), spent_of(&reply.spend), left(&reply))),
      Err(NoAnswer::Overtaken) => Ok(Came::Stopped),
      Err(no) => {
        let (reason, engine) = failure_of(no);

        Ok(Came::Failed(reason, engine))
      }
    }
  }

  // An agent's session across rooms folded in the program standing on it (docs/sessions.md, 「Session
  // compaction」): the fold word put in on the command that program stands on, so that nothing of the turn
  // restarts it or moves the session, under the lock every turn of that program holds. An inbox program takes
  // no command and folds itself.
  async fn fold_resident(&self, agent: &str, session: &SessionId) -> Result<(), Fault> {
    let ground = self.ground();
    let supervisor = crate::supervisors::named(&ground.db, agent)
      .await
      .map_err(|why| store_fault(why.into()))?;
    let (provider, model) = supervisor
      .as_ref()
      .and_then(road_of)
      .unwrap_or_else(|| (CHAT_PROVIDER.to_owned(), None));

    if let Some(refused) = folds_itself(session, crate::chat_stand_in::inbox_declared(&provider)) {
      return Err(refused);
    }

    let Some(held) = agent_session(&ground.db, agent).await.map_err(store_fault)? else {
      return Err(fault(FaultKind::Unknown, format!("{session} has not been opened")));
    };
    let Some(standing) = crate::lines::place::Place::of(&ground.roots.data, agent).program() else {
      return Err(fault(FaultKind::Refused, format!("{session} has no program standing to fold it")));
    };

    if crate::door::endpoint::session_named(&standing.command.args) != Some(held.as_str()) {
      return Err(fault(
        FaultKind::Refused,
        format!("{session}: its program stands on another session than {held}"),
      ));
    }

    let began = now_ms();
    let mut command = standing.command;

    // The token its place handed it is the place's own to keep: the program is known by the command without it.
    command.env.remove(crate::lines::token::TOKEN_VARIABLE);

    let composed = ComposedTurn {
      system_path: PathBuf::new(),
      command,
      session: held.clone(),
      prompt: crate::worker_session::fold::FOLD_WORD.to_owned(),
      model: model.unwrap_or_default(),
      effort: None,
      placement: None,
      window: None,
      account: None,
      began,
      run_env: None,
      given: None,
    };
    let mut log = RunLog::new(Target {
      agent: agent.to_owned(),
      run: format!("bmo-fold-{agent}-{began}"),
      chat_id: None,
      job_id: None,
    });
    let came = run_turn(
      ground,
      agent,
      crate::lines::chat::LINE,
      &provider,
      &composed,
      None,
      &mut log,
      false,
      None,
      None,
    )
    .await;
    let told = |ground: &Ground| {
      ground.events.publish(crate::worker_session::told::SessionFolded {
        session: held.clone(),
      });
    };

    match came {
      Ok(_) => {
        println!("compact {agent} {held}: compacted");
        told(ground);
        self.forget_left(session);

        Ok(())
      }
      Err(no) => {
        // Only a fold whose end could not be read may have reached the session.
        if matches!(no, NoAnswer::Failed(_) | NoAnswer::Cut(_)) {
          told(ground);
        }

        let (reason, _) = failure_of(no);

        eprintln!("compact {agent} {held}: failed — {reason}");

        Err(fault(FaultKind::Refused, format!("{session} could not be folded: {reason}")))
      }
    }
  }

  // A worker's latest session record, the one a fold or a reading of its state goes by.
  async fn latest_worker(&self, worker: &str) -> Result<Option<records::WorkerSession>, Fault> {
    Ok(records::sessions(&self.ground().db, worker).await.map_err(store_fault)?.pop())
  }
}

impl Sessions for HerdrSessions {
  async fn start(&self, request: Start) -> Result<Session, Fault> {
    if matches!(&request.scope, Scope::Apart(_) | Scope::Conversation(_))
      && (request.agent.trim().is_empty() || request.agent.contains('@'))
    {
      return Err(fault(FaultKind::Refused, format!("{} is no name a turn runs under", request.agent)));
    }

    self.admitted(&request.agent, &request.scope).await?;

    let (id, scope) = match &request.scope {
      Scope::Apart(apart) => {
        let mut apart = apart.clone();

        // A session on a runner line naming no folder goes out in that line's own.
        if apart.cwd.trim().is_empty()
          && let Some(line) = apart.line.as_deref()
        {
          apart.cwd = Trees::of(self.ground()).agents.join(line).to_string_lossy().into_owned();
        }

        // A number whose place still stands, kept for a turn the process before left in flight, is passed
        // over, so no new session goes into it.
        let data = &self.ground().roots.data;
        let id = loop {
          let id = apart_id(&request.agent, self.numbered.fetch_add(1, Ordering::Relaxed));

          if !crate::apart_places::is_apart_place(data, &crate::apart_places::place_name(&id)) {
            break id;
          }
        };

        self.apart().insert(id.clone(), (request.agent.clone(), apart.clone()));
        (id, Scope::Apart(apart))
      }
      Scope::Conversation(conversation) => {
        let mut conversation = conversation.clone();

        if conversation.cwd.trim().is_empty() {
          conversation.cwd = self.ground().source.to_string_lossy().into_owned();
        }

        let id = conversation_id(&request.agent, self.numbered.fetch_add(1, Ordering::Relaxed));

        self.conversations().insert(
          id.clone(),
          (
            request.agent.clone(),
            HeldConversation {
              conversation: conversation.clone(),
              harness: None,
            },
          ),
        );
        (id, Scope::Conversation(conversation))
      }
      scope => (
        id_of(&request.agent, scope).ok_or_else(|| {
          fault(FaultKind::Refused, format!("{}: its scope names no session", request.agent))
        })?,
        scope.clone(),
      ),
    };

    Ok(Session {
      id,
      agent: request.agent,
      scope,
    })
  }

  async fn switch(
    &self,
    session: &SessionId,
    provider: Option<&str>,
    model: Option<&str>,
  ) -> Result<Session, Fault> {
    let (agent, scope) = self.scope_held(session)?;

    if self.in_turn(session) {
      return Err(fault(FaultKind::Busy, format!("{session} is in a turn")));
    }

    match scope {
      Scope::Conversation(conversation) => {
        let provider = provider.map(str::trim).filter(|provider| !provider.is_empty());
        let model = model.map(str::trim).filter(|model| !model.is_empty());
        let next = Conversation {
          provider: provider.unwrap_or(&conversation.provider).to_owned(),
          model: model.unwrap_or(&conversation.model).to_owned(),
          cwd: conversation.cwd.clone(),
        };

        self.provider_ready(&next.provider).await?;

        let moved = next.provider != conversation.provider || next.model != conversation.model;

        if let Some((_, held)) = self.conversations().get_mut(session) {
          held.conversation = next.clone();

          if moved {
            held.harness = None;
          }
        }

        Ok(Session {
          id: session.clone(),
          agent,
          scope: Scope::Conversation(next),
        })
      }
      Scope::Apart(apart) => {
        let provider = provider.map(str::trim).filter(|provider| !provider.is_empty());
        let model = model.map(str::trim).filter(|model| !model.is_empty());
        let next = Apart {
          provider: provider.unwrap_or(&apart.provider).to_owned(),
          model: model.unwrap_or(&apart.model).to_owned(),
          ..apart.clone()
        };

        self.provider_ready(&next.provider).await?;

        if let Some((_, held)) = self.apart().get_mut(session) {
          *held = next.clone();
        }

        Ok(Session {
          id: session.clone(),
          agent,
          scope: Scope::Apart(next),
        })
      }
      Scope::Worker => Err(fault(
        FaultKind::Refused,
        format!("{session} is handed over on its change route, not switched"),
      )),
      Scope::Agent | Scope::Room(_) => Err(fault(
        FaultKind::Refused,
        format!("{session} goes out on the provider and model its agent's row names"),
      )),
    }
  }

  async fn push(&self, session: &SessionId, turn: i64, words: &str) -> Result<Aside, Fault> {
    let (agent, scope) = self.scope_held(session)?;

    self.admitted(&agent, &scope).await?;

    // The turn the words name must be the one the session carries now.
    let run = match self.going().get(session).map(Vec::as_slice) {
      Some([run]) if run.turn == turn => run.run.clone(),
      Some(_) => {
        return Err(fault(FaultKind::UnknownTurn, format!("no turn {turn} is going in {session}")));
      }
      None => {
        return Err(fault(FaultKind::UnknownTurn, format!("no turn {turn} is going in {session}")));
      }
    };

    // A job's turn that hears no words takes none.
    if self.deaf().contains(session) {
      return Ok(Aside::Refused);
    }
    let Some(mut offer) = aside::offer(&run, words) else {
      return Ok(Aside::Refused);
    };
    let said = |outcome: AsideOutcome| match outcome {
      AsideOutcome::Delivered => Aside::Delivered,
      AsideOutcome::Waiting => Aside::Waiting,
      AsideOutcome::Refused => Aside::Refused,
    };
    // The offer stands until a look takes it or the turn ends, which answers every offer it did not take.
    Ok((&mut offer.answer).await.map_or(Aside::Refused, said))
  }

  // The door's ending of every turn the send puts through a program's pane is kept for its end
  // (docs/api.md, 「sessions」, a turn's `ending`).
  async fn send_turn(&self, turn: Turn) -> Result<TurnEnd, Fault> {
    crate::door::liveness::keeping_endings(self.sent(turn)).await
  }

  async fn read_state(&self, session: &SessionId, model: Option<&str>) -> Result<State, Fault> {
    // A thrown-away session does not outlive the process that started it, but one carrying a picked-up turn
    // stands until that turn ends: its state answers by id meanwhile (docs/sessions.md, 「A restart hands the
    // session over」).
    let (agent, scope) = match self.scope_held(session) {
      Ok(held) => held,
      Err(fault) => match carried_scope_of(self.ground(), session) {
        Some((agent, scope)) => (agent, Scope::Apart(scope)),
        None => return Err(fault),
      },
    };

    self.admitted(&agent, &scope).await?;

    let ground = self.ground();
    let place = crate::lines::place::place_of(&ground.roots.data, &agent).to_string_lossy().into_owned();

    match &scope {
      // A room's session is readable from its start, before its first turn opens a generation: it stands
      // then on the provider and model its agent's row names, at generation 0.
      Scope::Room(room) => {
        let held = sessions::session(&ground.db, &agent, room_number(room)?).await.map_err(store_fault)?;
        let road = held.as_ref().and_then(|held| held.harness.as_ref()).map(|harness| &harness.road);
        let (provider, model) = match road.and_then(|road| road.split_once("::")) {
          Some((provider, model)) => (provider.to_owned(), model.to_owned()),
          None => match pinned(ground, &agent).await? {
            Some(pinned) => match choice_of(ground, &pinned).await {
              Ok((provider, model)) => (provider.name().to_owned(), model),
              Err(_) => Default::default(),
            },
            None => Default::default(),
          },
        };

        let context = self.resident_context(&agent, &provider).or_else(|| self.left_of(session));

        Ok(State::standing(
          session.clone(),
          provider,
          model,
          place,
          u64::try_from(held.as_ref().map_or(0, |held| held.generation)).unwrap_or_default(),
          context,
        ))
      }
      Scope::Agent => {
        // A supervisor whose turns go through its resident's inbox holds no session id of its own: its session
        // stands from its start on the provider its last answer went out on, else its order's first, at its
        // resident's context.
        let held = crate::supervisors::named(&ground.db, &agent)
          .await
          .map_err(|why| store_fault(why.into()))?;
        let road = held.as_ref().and_then(road_of);

        if let Some((provider, model)) =
          road.filter(|(provider, _)| crate::chat_stand_in::inbox_declared(provider))
        {
          let model = model.unwrap_or_default();
          let context = self.resident_context(&agent, &provider).or_else(|| self.left_of(session));

          return Ok(State::standing(session.clone(), provider, model, place, 1, context));
        }

        if agent_session(&ground.db, &agent).await.map_err(store_fault)?.is_none() {
          return Err(fault(FaultKind::Unknown, format!("{session} has not been opened")));
        }

        let model = held.as_ref().and_then(|held| held.last_model.clone()).unwrap_or_default();

        Ok(State::standing(
          session.clone(),
          CHAT_PROVIDER.to_owned(),
          model,
          place,
          1,
          self.left_of(session),
        ))
      }
      Scope::Worker => {
        let Some(held) = self.latest_worker(&agent).await? else {
          return Err(fault(FaultKind::Unknown, format!("{session} has not been opened")));
        };
        let generation = records::generation(&ground.db, &agent, &held.model).await.map_err(store_fault)?;
        // The road the worker stands on: the provider it is pinned to, else the one its held session last
        // answered on, else Claude Code's own.
        let provider = match crate::queue::routes::worker_pin(&ground.db, &agent).await {
          Ok(Some((provider, _))) => provider,
          Ok(None) => {
            crate::worker_session::held::held(ground, &crate::worker_session::held::scope_of(&agent))
              .await
              .ok()
              .flatten()
              .map(|held| held.mode)
              .filter(|mode| !mode.is_empty())
              .unwrap_or_else(|| worker_session::PROVIDER.to_owned())
          }
          Err(why) => return Err(fault(FaultKind::Refused, why.0)),
        };
        let mut state = State::standing(
          session.clone(),
          provider,
          held.answered_model.clone().unwrap_or_else(|| held.model.clone()),
          place,
          generation,
          carried_of(held.context_tokens, held.context_ratio),
        );

        self.going_shown(session, &mut state);
        state.continues = match model {
          Some(model) => continues(ground, &agent, model).await.map_err(store_fault)?,
          None => None,
        };

        Ok(state)
      }
      Scope::Apart(apart) => {
        let mut state = State::standing(
          session.clone(),
          apart.provider.clone(),
          apart.model.clone(),
          apart.cwd.clone(),
          1,
          None,
        );

        self.going_shown(session, &mut state);
        Ok(state)
      }
      Scope::Conversation(conversation) => {
        let mut state = State::standing(
          session.clone(),
          conversation.provider.clone(),
          conversation.model.clone(),
          conversation.cwd.clone(),
          1,
          None,
        );

        self.going_shown(session, &mut state);
        state.continues = self.held_harness(session);
        Ok(state)
      }
    }
  }

  // A worker's and a thrown-away session's turns put their questions to their caller; every other scope's
  // leaves them standing for a person, and takes no answer.
  async fn answer(&self, session: &SessionId, question: &str, answer: Answer) -> Result<(), Fault> {
    let (agent, scope) = self.scope_held(session)?;

    self.admitted(&agent, &scope).await?;

    match scope {
      Scope::Worker | Scope::Apart(_) => self.questions.answer(session, question, answer),
      _ => Err(fault(
        FaultKind::Unknown,
        format!("{session} puts no question to its caller, so it stands on no question {question}"),
      )),
    }
  }

  // Every scope's turns keep what they write while they go, and a turn that has ended keeps its words
  // until the session's next turn begins.
  async fn output(&self, session: &SessionId, turn: Option<i64>) -> Result<Output, Fault> {
    let (agent, scope) = self.scope_held(session)?;

    self.admitted(&agent, &scope).await?;

    self.outputs.read(session, turn).ok_or_else(|| match turn {
      Some(turn) => fault(FaultKind::UnknownTurn, format!("{session} has carried no turn {turn}")),
      None => fault(FaultKind::Unknown, format!("{session} has carried no turn to read")),
    })
  }

  async fn interrupt(&self, session: &SessionId, turn: i64) -> Result<(), Fault> {
    let runs = self.going().get(session).cloned().unwrap_or_default();
    let Some(running) = runs.into_iter().find(|running| running.turn == turn) else {
      return Err(fault(FaultKind::UnknownTurn, format!("no turn {turn} is going in {session}")));
    };
    let stops = crate::queue::stops::Stops::of(self.ground(), None);

    stops.cancel(&running.run).await.map_err(store_fault)?;
    Ok(())
  }

  async fn fold(&self, session: &SessionId) -> Result<(), Fault> {
    let (agent, scope) = self.scope_held(session)?;
    let row = self.admitted(&agent, &scope).await?;
    let ground = self.ground();

    if self.in_turn(session) {
      return Err(fault(FaultKind::Busy, format!("{session} is in a turn")));
    }

    match &scope {
      Scope::Room(room) => {
        let chat_id = room_number(room)?;
        let Some(pinned) = pinned(ground, &agent).await? else {
          return Err(fault(FaultKind::Unknown, format!("no agent is named {agent}")));
        };
        let (provider, model) =
          choice_of(ground, &pinned).await.map_err(|why| fault(FaultKind::Refused, why))?;
        let held = sessions::session(&ground.db, &agent, chat_id).await.map_err(store_fault)?;
        let Some(harness) = held.and_then(|held| held.harness) else {
          return Err(fault(FaultKind::Unknown, format!("{session} holds no harness session to fold")));
        };
        let settings = Settings::of(&ground.db, &agent)
          .await
          .map_err(|refused| fault(FaultKind::Refused, refused.error))?;
        let system = system_of(ground, &pinned, &settings, provider, &model)
          .await
          .map_err(|why| fault(FaultKind::Refused, why))?;
        let due = Due {
          provider,
          model,
          system,
          generation: generation_of(&settings),
          session: harness.id,
          placement: harness.placement,
          ratio: 1.0,
          why: Why::Limit,
        };

        hand_over(ground, &self.places.kept.reset, &pinned.speaker, chat_id, due).await;
        self.forget_left(session);
        Ok(())
      }
      Scope::Agent => self.fold_resident(&agent, session).await,
      Scope::Worker => {
        let (Some(row), Some(held)) = (row, self.latest_worker(&agent).await?) else {
          return Err(fault(FaultKind::Unknown, format!("{session} has not been opened")));
        };

        match fold(ground, &row, &held.model, &held.account, &held.session_id).await {
          Folded::Compacted => {
            self.forget_left(session);
            Ok(())
          }
          Folded::TooShort => Ok(()),
          Folded::Busy => Err(fault(FaultKind::Busy, format!("a job holds {agent}'s checkout"))),
          Folded::Failed => Err(fault(FaultKind::Refused, format!("{session} could not be folded"))),
        }
      }
      Scope::Apart(_) => Err(fault(
        FaultKind::Refused,
        format!("{session} is an apart session, whose turns each go out as a run of their own"),
      )),
      Scope::Conversation(_) => Err(fault(
        FaultKind::Refused,
        format!("{session} is a conversation, whose road compacts it inside its own turns"),
      )),
    }
  }

  async fn end(&self, session: &SessionId) -> Result<(), Fault> {
    let (agent, scope) = self.scope_held(session)?;

    self.admitted(&agent, &scope).await?;

    if self.in_turn(session) {
      return Err(fault(FaultKind::Busy, format!("{session} is in a turn")));
    }

    self.forget_left(session);

    let ground = self.ground();

    match &scope {
      Scope::Room(room) => {
        let chat_id = room_number(room)?;
        let held = sessions::session(&ground.db, &agent, chat_id).await.map_err(store_fault)?;

        if let Some(harness) = held.and_then(|held| held.harness) {
          let_go(ground, &agent, &[harness.id]).await;
          sessions::open_generation(&ground.db, &agent, chat_id).await.map_err(store_fault)?;
        }
      }
      Scope::Agent => {
        if let Some(held) = agent_session(&ground.db, &agent).await.map_err(store_fault)? {
          let_go(ground, &agent, &[held]).await;
          keep_agent_session(&ground.db, &agent, None).await.map_err(store_fault)?;
        }
      }
      Scope::Worker => {
        let place = crate::lines::place::Place::of(&ground.roots.data, &agent);

        place.release().await;

        for held in records::sessions(&ground.db, &agent).await.map_err(store_fault)? {
          records::forget(&ground.db, &agent, &held.model, Some(&held.account))
            .await
            .map_err(store_fault)?;
        }
      }
      Scope::Apart(apart) => {
        self.apart().remove(session);
        self.referenced().remove(session);

        // A session in a place of its own ends it.
        if in_places(apart) {
          crate::apart_places::end(ground, &crate::apart_places::place_name(session)).await;
        }

        // A lease bound to the session ends with it (docs/role-slots.md, 「The line」).
        if let Err(why) = crate::order_run::dispatch::release_sessions(
          &ground.db,
          &ground.events,
          std::slice::from_ref(&session.0),
          "its role session ended",
        )
        .await
        {
          eprintln!("{session}: the slot lease bound to it was not given back: {why}");
        }
      }
      Scope::Conversation(_) => {
        let harness = self.conversations().remove(session).and_then(|(_, held)| held.harness);

        if let Some(harness) = harness {
          let_go(ground, &agent, &[harness]).await;
        }
      }
    }

    // A hold the session took for a turn that never started its job ends with the session (docs/api.md,
    // 「assignments and jobs」): a hold is never for a clock, so only an observed end lets it go. A room
    // session's harness may have named the agent itself rather than the session's id.
    let mut holders = vec![session.0.as_str()];

    if matches!(scope, Scope::Room(_)) && agent.as_str() != session.0.as_str() {
      holders.push(agent.as_str());
    }

    for holder in holders {
      if let Err(why) = crate::control::work::release_held(&ground.db, holder).await {
        eprintln!("{session}: the holds it took were not released: {}", why.error);
      }
    }

    self.outputs.forget(session);

    Ok(())
  }

  async fn list(&self) -> Result<Vec<Session>, Fault> {
    let db = &self.ground().db;
    let mut sessions = vec![];
    let rooms = all(db, "SELECT agent, chat_id FROM room_sessions ORDER BY agent, chat_id", vec![])
      .await
      .map_err(|why| store_fault(why.into()))?;

    for row in rooms {
      let agent: String = row.try_get("", "agent").map_err(|why| store_fault(why.into()))?;
      let chat_id: i64 = row.try_get("", "chat_id").map_err(|why| store_fault(why.into()))?;

      sessions.extend(listed(agent, Scope::Room(chat_id.to_string())));
    }

    let across = all(
      db,
      r#"SELECT "name" FROM "supervisors" WHERE "session_scope" = 'agent' AND "session_id" IS NOT NULL ORDER BY "name""#,
      vec![],
    )
    .await
    .map_err(|why| store_fault(why.into()))?;

    for row in across {
      let agent: String = row.try_get("", "name").map_err(|why| store_fault(why.into()))?;

      sessions.extend(listed(agent, Scope::Agent));
    }

    for (id, (agent, apart)) in self.apart().iter() {
      sessions.push(Session {
        id: id.clone(),
        agent: agent.clone(),
        scope: Scope::Apart(apart.clone()),
      });
    }

    for (id, (agent, held)) in self.conversations().iter() {
      sessions.push(Session {
        id: id.clone(),
        agent: agent.clone(),
        scope: Scope::Conversation(held.conversation.clone()),
      });
    }

    Ok(sessions)
  }
}

/// The refusal of a fold of a session across rooms whose program takes no command and folds itself, none where
/// the word reaches the program.
pub fn folds_itself(session: &SessionId, inbox: bool) -> Option<Fault> {
  inbox.then(|| {
    fault(
      FaultKind::Refused,
      format!("{session} is carried past its window by its program inside its own turns"),
    )
  })
}

// The road a supervisor's session across rooms stands on: the provider its last answer went out on, else its
// order's first, with the model it names.
fn road_of(supervisor: &crate::supervisors::Supervisor) -> Option<(String, Option<String>)> {
  supervisor
    .last_mode
    .clone()
    .filter(|mode| !mode.is_empty())
    .map(|mode| (mode, supervisor.last_model.clone()))
    .or_else(|| {
      supervisor
        .order
        .first()
        .map(|entry| (entry.provider.clone(), Some(entry.model.clone())))
    })
}

// A turn across rooms as the chat line launches it: the room's block, the road and the base persona its
// caller handed in for where the agent's brain gives none, on the session named.
fn launch_of<'a>(
  agent: &'a str,
  room: &RoomBlock,
  context: &'a TurnContext,
  resume: Option<String>,
) -> ChatLaunch<'a> {
  launch_on(agent, room, context, resume, None, None)
}

// A turn as a launch that names the model and folder where the caller chose them rather than the agent's
// own.
fn launch_on<'a>(
  agent: &'a str,
  room: &RoomBlock,
  context: &'a TurnContext,
  resume: Option<String>,
  model: Option<&'a str>,
  cwd: Option<&'a str>,
) -> ChatLaunch<'a> {
  ChatLaunch {
    agent,
    chat_id: room.chat_id,
    room: RoomContext {
      chat_id: room.chat_id,
      text: room.text.clone(),
      mark: room.mark.clone(),
    },
    via: context.via.as_deref(),
    resume,
    privileged: context.privileged,
    ping: false,
    base_persona: context.base_persona.as_deref(),
    model,
    cwd,
  }
}

/// The backend shared by the routes that answer the contract.
pub type Shared = Arc<HerdrSessions>;

// What a turn sent to a `<repository>@worker` address is answered: no session stands there.
fn no_worker_scope(session: &SessionId) -> Fault {
  fault(
    FaultKind::Unknown,
    format!("{session} is no session: {}", crate::control::session_ops::WORKER_RETIRED),
  )
}

// The worker's own session, as the contract names it.
fn worker_session_id(worker: &str) -> Result<SessionId, Fault> {
  id_of(worker, &Scope::Worker)
    .ok_or_else(|| fault(FaultKind::Unknown, format!("{worker} names no worker's session")))
}

// A chat turn's run opened in the ledger (docs/run-records.md, 「The run ledger」): the room it runs for,
// the round its context names and the latest message it answers. A run the ledger would not take is said
// and never fails the turn.
async fn open_chat_run(
  db: &sea_orm::DatabaseConnection,
  agent: &str,
  room: &str,
  run: &str,
  context: &TurnContext,
  at: i64,
) -> Option<i64> {
  let opening = run_ledger::Opening {
    agent: agent.to_owned(),
    chat_id: room.parse().ok(),
    unit: run.to_owned(),
    depth: 0,
    cap_ms: None,
    started_at: at,
    started_by: None,
    search_id: None,
    round: context.round.clone(),
    message_id: context.answers.iter().max().copied(),
  };

  match run_ledger::open(db, &opening).await {
    Ok(id) => Some(id),
    Err(why) => {
      eprintln!("{run}: the chat run could not be written to the ledger: {}", why.detail);
      None
    }
  }
}

// What a job's turn spent, each figure summed over the models that answered it; none where it reported none.
pub fn turn_spent(end: &TurnEnd) -> Option<run_ledger::Spent> {
  let spend = &end.reply.as_ref()?.spend;

  (!spend.is_empty()).then(|| {
    spend.iter().fold(run_ledger::Spent::default(), |sum, one| run_ledger::Spent {
      input_tokens: sum.input_tokens + one.input_tokens,
      output_tokens: sum.output_tokens + one.output_tokens,
      cache_read_tokens: sum.cache_read_tokens + one.cache_read_tokens,
      cache_write_tokens: sum.cache_write_tokens + one.cache_write_tokens,
    })
  })
}

/// A chat turn's run closed as its turn ended: how it ended, the tokens its context carried — its answer's,
/// else the context the turn left — and what it spent.
pub async fn close_chat_run(
  db: &sea_orm::DatabaseConnection,
  ledger: Option<i64>,
  end: &TurnEnd,
  spent: Option<run_ledger::Spent>,
  left: Option<Context>,
) {
  let Some(id) = ledger else {
    return;
  };
  let closing = run_ledger::Closing {
    finished_at: now_ms(),
    outcome: serde_json::to_value(end.outcome)
      .ok()
      .and_then(|outcome| outcome.as_str().map(str::to_owned))
      .unwrap_or_else(|| "failed".to_owned()),
    context_tokens: end
      .reply
      .as_ref()
      .and_then(|reply| reply.context_tokens)
      .or_else(|| left.and_then(|left| i64::try_from(left.carried).ok())),
    spent,
  };

  if let Err(run_ledger::NotClosed::Store(why)) = run_ledger::close(db, id, &closing).await {
    eprintln!("{}: the chat run could not be closed in the ledger: {}", end.run, why.detail);
  }
}
