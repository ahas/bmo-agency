//! The herdr backend of the session contract (docs/api.md, 「Sessions」): the ten operations answered over
//! the door, the places and the lines. A worker's session is its own, one per worker, carried by the
//! worker's session turn on the line a job runs on; a companion's session is one per room, carried by its
//! resident program on the chat line with the system prompt its settings compose; a session across rooms is
//! the one its agent's row keeps, carried by its resident program on the chat line around the room block,
//! the road and the base persona, its brain's or else the one the turn's caller hands in, or by a stand-in
//! where the agent's key walks onto one; a thrown-away session, held in memory from its start to its end,
//! carries a co-work branch's or a research run's turns, each on a fresh harness session; and a conversation
//! the caller opened, held there too, carries ordinary turns on the provider and model it names, resuming the
//! session its road keeps. A session is known by its agent and its scope — `<agent>@worker`,
//! `<agent>@<room>`, `<agent>`, `<agent>@apart:<n>`, `<agent>@conversation:<n>` — and stands across the
//! generations its harness opens.
//!
//! A turn is carried whole before *send a turn* answers with its end: whether the program took the words,
//! or refused them while it waits on a person, and how it ended. Its end is published as `turn-ended`, and
//! its start as
//! `turn-started`; a conversational turn's road as `road-walked`. Words pushed into a session are offered to
//! the run its turn carries, which its door takes at its next look at the working program.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use agent_session::EngineError;
use agent_session::{
  Answer, Apart, Aside, Context, Conversation, Fault, FaultKind, GateAsk, Outcome, Output, RoomBlock,
  Scope, Session, SessionId, Sessions, Start, State, Turn, TurnContext, TurnEnd, WentOut, WorkParts,
};
use indexmap::IndexMap;
use policy::companion_runner::COMPANION_RUNNER;
use policy::fill_marks;
use policy::gate_places::GATE_PLACES;
use policy::resident_turn::RESIDENT_TURN;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use store::{all, one, run as written};

use crate::chat_stand_in::{fallback_ended, stand_in_turn};
use crate::companion::guard::spent_out;
use crate::companion::handoff::{Companion, Due, hand_over};
use crate::companion::overtaken;
use crate::companion::run::{NoAnswer, Reply, Tally, let_go, run_turn};
use crate::companion::sessions::{self, Harness, Kept, RoomSession, harness_road};
use crate::control::claude_accounts::{Judging, account_open};
use crate::control::companions::Settings;
use crate::door::aside;
use crate::door::record::{ModelSpend, UsageSpent};
use crate::door::turn::{AsideOutcome, Quiet, Watch};
use crate::events::Event;
use crate::job_run::claude::turn::{Shows, WorkAsked, WorkSeat, carry, continues};
use crate::lines::chat::{ChatLaunch, RoomContext, compose_prepared, compose_turn, gate_run};
use crate::lines::companion::{CompanionTurn, LOST_SESSION, compose, pinned_provider, registration_named};
use crate::places::gate_places;
use crate::places::locks::{pane_lock_path, try_hold};
use crate::places::{AgentRow, Ground, Places};
use crate::run_ledger;
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::session_output::{OutputWatch, Outputs};
use crate::session_questions::Questions;
use crate::watches::machine::keep_worker_job;
use crate::worker_session::fold::{Folded, fold};
use crate::worker_session::records::{self, note_answer, slot_account};
use crate::worker_session::{self, Missed, Side, Turned};
use crate::worker_words::{WordRoad, WordTurn};
use crate::{Refusal, now_ms};

// What a worker's session is told apart by in its id, after its agent.
const WORKER: &str = "worker";

// What a thrown-away session is told apart by in its id, after its agent.
const APART: &str = "apart";

// What a conversation the caller opened is told apart by in its id, after its agent.
const CONVERSATION: &str = "conversation";

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
      stage: COMPANION_RUNNER.turn_stage.clone(),
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

/// The harness session an agent's row keeps across rooms, where it keeps one.
pub async fn agent_session(
  db: &sea_orm::DatabaseConnection,
  agent: &str,
) -> Result<Option<String>, Refusal> {
  Ok(
    one(db, r#"SELECT "session_id" FROM "agents" WHERE "name" = $1"#, vec![agent.into()])
      .await?
      .and_then(|row| row.try_get::<Option<String>>("", "session_id").ok().flatten())
      .filter(|session| !session.is_empty()),
  )
}

/// The harness session an agent keeps across rooms written on its row, with the room its last turn
/// answered; none lets it go.
pub async fn keep_agent_session(
  db: &sea_orm::DatabaseConnection,
  agent: &str,
  kept: Option<(&str, i64)>,
) -> Result<(), Refusal> {
  let (session, chat_id) =
    kept.map_or((None, None), |(session, chat_id)| (Some(session.to_owned()), Some(chat_id)));

  written(
    db,
    r#"UPDATE "agents" SET "session_id" = $2, "chat_id" = COALESCE($3, "chat_id") WHERE "name" = $1"#,
    vec![agent.into(), session.into(), chat_id.into()],
  )
  .await
  .map(|_| ())
  .map_err(Refusal::from)
}

/// The session contract over herdr, on the places the server stands.
pub struct HerdrSessions {
  places: Places,
  // The turn each session carries while it goes, with the run it goes out as, which the interrupt stops:
  // one a session, a second turn refused while it goes.
  going: Mutex<HashMap<SessionId, Vec<Running>>>,
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
  // The places each bounded role holds for its running turns, by the role's name.
  role_places: Mutex<BTreeMap<String, Arc<tokio::sync::Semaphore>>>,
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

// What a companion's agent row holds of its pin and its prompts.
struct Pinned {
  companion: Companion,
  provider: Option<String>,
  prompts: Option<String>,
}

async fn pinned(ground: &Ground, agent: &str) -> Result<Option<Pinned>, Fault> {
  let row = one(
    &ground.db,
    r#"SELECT "name", "label", "pinned_provider", "pinned_model", "prompts" FROM "agents" WHERE "name" = $1"#,
    vec![agent.into()],
  )
  .await
  .map_err(|why| store_fault(why.into()))?;
  let Some(row) = row else { return Ok(None) };
  let text = |name: &str| row.try_get::<Option<String>>("", name).ok().flatten();

  Ok(Some(Pinned {
    companion: Companion {
      name: agent.to_owned(),
      label: text("label"),
      pinned_model: text("pinned_model"),
    },
    provider: text("pinned_provider"),
    prompts: text("prompts"),
  }))
}

// The provider a companion's row pins it to (`lines::companion::pinned_provider`).
async fn provider_of(
  ground: &Ground,
  pinned: &Pinned,
) -> Result<&'static dyn harness_providers::Provider, String> {
  pinned_provider(ground, pinned.provider.as_deref(), pinned.companion.pinned_model.as_deref()).await
}

// The system prompt a companion's settings compose, out of the documents of the tree the process runs.
async fn system_of(ground: &Ground, pinned: &Pinned, settings: &Settings) -> Result<String, String> {
  let chosen = one(
    &ground.db,
    "SELECT p.text FROM agent_base_prompts c JOIN base_prompts p ON p.name = c.prompt WHERE c.agent = $1",
    vec![pinned.companion.name.clone().into()],
  )
  .await
  .map_err(|why| why.detail)?
  .and_then(|row| row.try_get::<String>("", "text").ok());
  let base = chosen.map(|text| harness_machine::companion_prompt::Base {
    text,
    model: pinned.companion.pinned_model.clone().unwrap_or_default(),
  });
  let roots = harness_machine::companion_prompt::Roots {
    assets: ground.code_root(),
    data: ground.roots.data.clone(),
  };
  let companion = harness_machine::companion_prompt::Companion {
    name: &pinned.companion.name,
    prompts: pinned.prompts.as_deref(),
  };

  harness_machine::companion_prompt::system(&roots, companion, &settings.composition(), base.as_ref())
    .map_err(|why| why.to_string())
}

// The generation settings a companion's row holds, in the order they were saved.
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
  /// The answer, with what the turn spent where its road reported it.
  Answered(String, Option<run_ledger::Spent>),
  Blocked,
  Stopped,
  Failed(String, Option<EngineError>),
  /// A job's turn, whose end is read whole where it is carried.
  Ended(Box<TurnEnd>),
}

impl Came {
  // The turn's end under the run it went out as.
  fn end(self, run: &str) -> TurnEnd {
    match self {
      Self::Answered(text, _) => TurnEnd::answered(run, text),
      Self::Blocked => TurnEnd::of(run, Outcome::Blocked),
      Self::Stopped => TurnEnd::of(run, Outcome::Stopped),
      Self::Failed(why, engine) => TurnEnd::failed(run, why, engine),
      Self::Ended(end) => *end,
    }
  }

  // What the turn spent, read off the answer where it carried it and off a job's end where that did.
  fn spent(&self) -> Option<run_ledger::Spent> {
    match self {
      Self::Answered(_, spent) => *spent,
      Self::Ended(end) => turn_spent(end),
      _ => None,
    }
  }
}

// What a turn's models spent, each figure summed over them; none where they reported nothing.
fn spent_of(spend: &[ModelSpend]) -> Option<run_ledger::Spent> {
  (!spend.is_empty()).then(|| {
    spend.iter().fold(run_ledger::Spent::default(), |sum, one| run_ledger::Spent {
      input_tokens: sum.input_tokens + one.input_tokens,
      output_tokens: sum.output_tokens + one.output_tokens,
      cache_read_tokens: sum.cache_read_tokens + one.cache_read_tokens,
      cache_write_tokens: sum.cache_write_tokens + one.cache_write_tokens,
    })
  })
}

/// Why a run came to no answer, and the engine's error where its engine ended it with one.
pub fn failure_of(no: NoAnswer) -> (String, Option<EngineError>) {
  match no {
    NoAnswer::Errored { error, kind } => (error, Some(EngineError { kind })),
    NoAnswer::Cut(why) | NoAnswer::Failed(why) => (why, None),
    NoAnswer::Overtaken => ("the companion's reset overtook the turn".to_owned(), None),
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
      showing: Mutex::default(),
      apart: Mutex::default(),
      conversations: Mutex::default(),
      numbered: AtomicU64::new(1),
      turns: AtomicI64::new(0),
      questions: Questions::default(),
      deaf: Mutex::default(),
      outputs: Outputs::default(),
      role_places: Mutex::default(),
    }
  }

  fn deaf(&self) -> std::sync::MutexGuard<'_, HashSet<SessionId>> {
    self.deaf.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  // The role's places: a role the policy bounds holds one for the turn's length, so its agents run at
  // most that many at once. A turn beyond the bound waits; none is refused.
  async fn role_turn(&self, agent: &str) -> Result<Option<tokio::sync::OwnedSemaphorePermit>, Fault> {
    let role = crate::conversational::role_of(&self.ground().db, agent).await.map_err(store_fault)?;
    let Some(limit) = role
      .as_deref()
      .and_then(|role| policy::roles::ROLES.role(role))
      .and_then(|role| role.max_concurrent)
    else {
      return Ok(None);
    };
    let place = {
      let mut held = self.role_places.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

      held
        .entry(role.unwrap_or_default())
        .or_insert_with(|| Arc::new(tokio::sync::Semaphore::new(limit)))
        .clone()
    };

    place
      .acquire_owned()
      .await
      .map(Some)
      .map_err(|_| fault(FaultKind::Unknown, format!("{agent}: its role's places closed")))
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

  fn ground(&self) -> &Ground {
    &self.places.ground
  }

  fn going(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, Vec<Running>>> {
    self.going.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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

  // A run let go of its session's turns as it ends, and the words pushed into it with it.
  fn ended(&self, session: &SessionId, run: &str) {
    aside::forget(run);
    self.showing().remove(session);
    self.questions.clear(session);
    self.deaf().remove(session);

    let mut going = self.going();

    if let Some(runs) = going.get_mut(session) {
      if let Some(at) = runs.iter().position(|going| going.run == run) {
        let ended = runs.remove(at);

        self.outputs.ended(session, ended.turn);
      }

      if runs.is_empty() {
        going.remove(session);
      }
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
      Scope::Apart(apart) if apart.provider != worker_session::PROVIDER => Err(fault(
        FaultKind::Refused,
        format!(
          "{agent}: a thrown-away session goes out on {}, not {}",
          worker_session::PROVIDER,
          apart.provider
        ),
      )),
      Scope::Apart(_) => Ok(None),
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

  // The worker's session a turn goes on: the model of the one it answered on last, under a login that
  // answers now, and that session where it may still be resumed; the model its row last ran on otherwise.
  async fn worker_session(&self, worker: &str) -> Result<(String, Option<String>), Fault> {
    let ground = self.ground();
    let held = records::sessions(&ground.db, worker).await.map_err(store_fault)?;
    let now = now_ms();

    for session in held.iter().rev() {
      let open = account_open(Judging::of(ground), &session.account, now)
        .await
        .map_err(|refused| fault(FaultKind::Unreachable, refused.error))?;

      if open {
        let ttl = crate::lines::standing::session_ttl_ms(ground, &session.model).await;
        let resumed = records::resumable(&ground.db, worker, &session.model, &session.account, ttl)
          .await
          .map_err(store_fault)?;

        return Ok((session.model.clone(), resumed));
      }
    }

    let last = one(
      &ground.db,
      r#"SELECT "last_model" FROM "agents" WHERE "name" = $1"#,
      vec![worker.into()],
    )
    .await
    .map_err(|why| store_fault(why.into()))?
    .and_then(|row| row.try_get::<Option<String>>("", "last_model").ok().flatten());

    match last {
      Some(model) => Ok((model, None)),
      None => Err(fault(FaultKind::Refused, format!("{worker} has run on no model to open a session on"))),
    }
  }

  async fn worker_turn(
    &self,
    row: &AgentRow,
    run: &str,
    prompt: &str,
    session: &SessionId,
    turn: i64,
  ) -> Result<Came, Fault> {
    let ground = self.ground();
    let (model, kept) = self.worker_session(&row.name).await?;
    let turned = worker_session::turn(
      &Side { ground },
      worker_session::Asked {
        worker: row,
        model,
        session: kept,
        prompt: prompt.to_owned(),
        name: run.to_owned(),
        title: format!("{}: a turn", row.name),
        label: "turn".to_owned(),
        bound: None,
      },
      &mut Quiet,
    )
    .await
    .map_err(|refusal| fault(FaultKind::Refused, refusal.detail))?;

    if let Some(reply) = &turned.reply {
      self.outputs.wrote(session, turn, std::slice::from_ref(&reply.text));
    }

    Ok(match (turned.reached, turned.missed) {
      (_, Some(Missed::Person)) => Came::Blocked,
      (_, Some(Missed::Stopped)) => Came::Stopped,
      (false, missed) => Came::Failed(format!("the words did not reach the session ({missed:?})"), None),
      (true, _) => Came::Answered(turned.reply.map(|reply| reply.text).unwrap_or_default(), None),
    })
  }

  // The companion's session in the room its turn goes on in: the one it holds, or a new generation where it
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
              .map(|held| !held.addresses.iter().any(|address| &address.node == node))
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

  // One turn of a companion in a room, carried by its resident program on its room's session.
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
    let provider = match provider_of(ground, &pinned).await {
      Ok(provider) => provider,
      Err(why) => return failed(why),
    };
    let began = context.began.unwrap_or_else(now_ms);
    let hold = &self.places.kept.reset;
    let keeping = hold.keeping().await;

    if overtaken::overtaken(&db, agent, began).await {
      return Ok(Came::Stopped);
    }

    let model = pinned.companion.pinned_model.clone().unwrap_or_default();
    let settings = Settings::of(&db, agent)
      .await
      .map_err(|refused| fault(FaultKind::Refused, refused.error))?;
    let system = match system_of(ground, &pinned, &settings).await {
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
      log.write(&db, Line::note(COMPANION_RUNNER.log.cleared.clone())).await;
    }

    let mut session = self
      .room_session(agent, chat_id, provider, &model)
      .await
      .map_err(|why| fault(FaultKind::Refused, detail(why)))?;
    let hash = format!("{:x}", Sha256::digest(system.as_bytes()));

    if session.system_hash.as_deref() != Some(hash.as_str()) {
      let changed = session.system_hash.is_some();

      session = sessions::mark_system(&db, agent, chat_id, &hash).await.map_err(store_fault)?;

      if changed {
        log.write(&db, Line::note(COMPANION_RUNNER.log.system_changed.clone())).await;

        if let Some(session) = id_of(agent, &Scope::Room(chat_id.to_string())) {
          ground.events.publish(SystemPromptChanged {
            session: session.0,
            run: run.to_owned(),
          });
        }
      }
    }

    // A session opened by a handoff begins with the note its last generation wrote, ahead of the turn's
    // words.
    let prompt = match (&session.harness, &session.handoff) {
      (None, Some(note)) => format!("{note}\n\n{prompt}"),
      _ => prompt.to_owned(),
    };
    let generation = generation_of(&settings);
    let guarded = provider.declared().resident.as_ref().is_some_and(|resident| resident.turn_limits);

    drop(keeping);

    let started = Instant::now();
    let mut asked = self
      .room_run(
        &mut log,
        &pinned.companion,
        provider,
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

    // A session its harness no longer holds is let go, and the turn is asked again once on a fresh one.
    if let Err(NoAnswer::Failed(said)) = &asked
      && session.harness.is_some()
      && said.contains(LOST_SESSION)
    {
      session = sessions::open_generation(&db, agent, chat_id).await.map_err(store_fault)?;
      asked = self
        .room_run(
          &mut log,
          &pinned.companion,
          provider,
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
          text: COMPANION_RUNNER.log.answered.clone(),
          state: Some("succeeded"),
          ..Line::default()
        },
      )
      .await;

    // A turn whose context reached its fold ratio hands its session over before the next one, where its
    // provider declares the handoff (docs/companions.md, 「Its limit」).
    let hands_off = provider
      .declared()
      .resident
      .as_ref()
      .is_some_and(|resident| resident.handover == policy::providers::Handover::Handoff);
    let mark =
      match crate::control::fold_ratios::of_agent(&db, provider.name(), &pinned.companion.name).await {
        Ok(mark) => mark,
        Err(refused) => {
          eprintln!(
            "agency: {}'s fold ratio did not read, its provider's stands: {}",
            pinned.companion.name, refused.error
          );

          crate::control::fold_ratios::fallback(provider.name()).0
        }
      };

    if let Some(ratio) = reply.context_ratio.filter(|ratio| hands_off && *ratio >= mark) {
      let due = Due {
        provider,
        system,
        generation,
        session: reply.session_id.clone(),
        placement,
        ratio,
      };
      let (ground, hold, companion) = (self.places.ground.clone(), hold.clone(), pinned.companion);

      tokio::spawn(async move { hand_over(&ground, &hold, &companion, chat_id, due).await });
    }

    Ok(Came::Answered(reply.text, spent_of(&reply.spend)))
  }

  #[allow(clippy::too_many_arguments)]
  async fn room_run(
    &self,
    log: &mut RunLog,
    companion: &Companion,
    provider: &'static dyn harness_providers::Provider,
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

    if let Some(reason) = spent_out(&COMPANION_RUNNER.limits, session.spent).filter(|_| guarded) {
      return Err(NoAnswer::Cut(reason));
    }

    let resume = session
      .harness
      .as_ref()
      .map(|harness| (harness.id.as_str(), harness.placement.as_deref()));
    let model = companion.pinned_model.clone().unwrap_or_default();
    let run = log.run().to_owned();
    let composed = compose(
      ground,
      &ground.machine(),
      &CompanionTurn {
        agent: &companion.name,
        chat_id: session.chat_id,
        provider: provider.name(),
        model: &model,
        system,
        prompt,
        privileged: context.privileged,
        resume,
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
      agent: &companion.name,
      chat_id: session.chat_id,
      carried: session.spent,
    };
    let reply = run_turn(
      ground,
      &companion.name,
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
    let held = agent_session(&db, agent).await.map_err(store_fault)?;
    let started = Instant::now();
    // The agent's key walked first: a stand-in that answers carries the turn, and Claude Code's session
    // stays as it stands for its next turn. The run is the chat line's; the fallback walk and roads are
    // the one mechanism every line crosses over on (docs/cache-fallback.md).
    let line_run = match crate::lines::chat::chat_run(
      ground,
      &ground.machine(),
      &launch_of(agent, room, context, None),
      prompt,
      false,
    )
    .await
    {
      Ok(line_run) => line_run,
      Err(why) => return Ok(Came::Failed(why.detail, None)),
    };
    let stood_in =
      stand_in_turn(ground, &mut log, &line_run, agent, &context.answers, began, Some(shown)).await;
    let on_claude = matches!(stood_in, Ok(None));
    let asked = match stood_in {
      Ok(Some(reply)) => Ok(reply),
      Err(no) => Err(no),
      Ok(None) => {
        let mut asked =
          self.agent_run(&mut log, agent, room, context, prompt, held.clone(), began, shown).await;

        if let Err(NoAnswer::Failed(said)) = &asked
          && held.is_some()
          && said.contains(LOST_SESSION)
        {
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
        let model = crate::control::tiers::resolve(&db, agent, CHAT_PROVIDER)
          .await
          .map(|mapping| mapping.model)
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

    if on_claude {
      keep_agent_session(&db, agent, Some((&reply.session_id, room.chat_id)))
        .await
        .map_err(store_fault)?;
      fallback_ended(ground).await;
    }

    log
      .write(
        &db,
        Line {
          kind: "end",
          text: COMPANION_RUNNER.log.answered.clone(),
          state: Some("succeeded"),
          ..Line::default()
        },
      )
      .await;

    Ok(Came::Answered(reply.text, spent_of(&reply.spend)))
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
      && said.contains(LOST_SESSION)
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
          text: COMPANION_RUNNER.log.answered.clone(),
          state: Some("succeeded"),
          ..Line::default()
        },
      )
      .await;

    Ok(Came::Answered(reply.text, spent_of(&reply.spend)))
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
          text: COMPANION_RUNNER.log.answered.clone(),
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

    Ok(Came::Answered(reply.text, spent_of(&reply.spend)))
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

  // What a companion's run spent is its own: its last answer written and its usage published.
  async fn record_usage(&self, agent: &str, reply: &Reply, run: &str) {
    let ground = self.ground();

    if let Err(why) =
      note_answer(&ground.db, agent, &reply.mode, Some(&reply.model), reply.effort.as_deref()).await
    {
      eprintln!("companion {agent}: its last answer was not written: {}", why.detail);
    }

    let account = slot_account(&reply.mode, reply.account.as_deref());

    if let Some(spent) =
      UsageSpent::of(agent, Some(run), now_ms() / 1000, &reply.spend, |_| account.clone())
    {
      ground.events.publish(spent);
    }
  }

  // A turn's end published as `turn-ended`, before its send is answered with it.
  fn published(&self, session: &SessionId, end: TurnEnd) -> TurnEnd {
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

  // A worker's job turn carried on a fallback road: the same stand-in walk, roads and setting a chat turn
  // crosses over on, composing the worker's own line — its rules, tools, MCP and checkout. The road was
  // chosen in the job's work parts (`control::work`, `worker_road`); dsh keeps no session, so nothing of the
  // worker's Claude session is read, judged or kept here.
  async fn worker_stand_in(
    &self,
    agent: &str,
    work: &WorkParts,
    prompt: &str,
    began: i64,
    job: Option<agent_session::JobMark>,
    shown: OutputWatch<'_>,
  ) -> Result<Came, Fault> {
    let ground = self.ground();
    let trees = crate::lines::Trees::of(ground);
    let row = AgentRow::named(&ground.db, agent)
      .await
      .map_err(|why| fault(FaultKind::Unknown, why.detail))?
      .ok_or_else(|| fault(FaultKind::Unknown, format!("no worker is named {agent}")))?;
    let cwd = row.cwd.clone().unwrap_or_else(|| ground.source.to_string_lossy().into_owned());
    let line = crate::lines::worker::worker_run(ground, &trees, Some(agent), None, &work.model, &cwd)
      .await
      .map_err(|why| fault(FaultKind::Refused, why.detail))?;
    let mut env: std::collections::BTreeMap<String, String> = ground.env().clone();

    for (name, value) in &work.variables {
      env.insert(name.clone(), value.clone());
    }
    env.insert("BMO_TURN_AGENT".to_owned(), agent.to_owned());
    if job.is_some() {
      env.insert("BMO_TASK_LOCKED".to_owned(), "1".to_owned());
    }

    // The worker's whole system prompt where a base prompt composed one, else its rules file, which dsh
    // carries after the headless opener as the worker's persona.
    let system_path = line.system_prompt.clone().unwrap_or_else(|| line.rules.clone());
    let run = crate::lines::chat::ChatRun {
      line: crate::lines::worker::LINE,
      agent: agent.to_owned(),
      model: work.model.clone(),
      effort: line.effort.clone(),
      tier: work.mapping.tier.clone(),
      reasoning: line.reasoning.clone().unwrap_or_default(),
      thinking: line.thinking,
      system_path,
      room: RoomContext {
        chat_id: 0,
        text: String::new(),
        mark: String::new(),
      },
      // The worker takes every dsh tool row but the web (its own server replaces it); the list only has to
      // be non-empty for the road to hand it its working rows rather than none.
      tools_shown: "Bash".to_owned(),
      tools_allowed: "Bash".to_owned(),
      mcp_config: line.mcp_config.clone(),
      mcp_tools: None,
      prompt: prompt.to_owned(),
      persona_unused: None,
      fold: false,
      ping: false,
      resume: None,
      cwd,
      env,
    };
    let mut log = RunLog::new(Target {
      agent: agent.to_owned(),
      run: work.name.clone(),
      chat_id: (work.chat != 0).then_some(work.chat),
      job_id: job.map(|job| job.job),
    });

    match stand_in_turn(ground, &mut log, &run, crate::queue::routes::JOB_WORK_KEY, &[], began, Some(shown))
      .await
    {
      Ok(Some(reply)) => Ok(Came::Answered(reply.text, spent_of(&reply.spend))),
      Ok(None) => Ok(Came::Failed(
        format!("{agent}: no fallback road carried the job and Claude could not be asked"),
        None,
      )),
      Err(NoAnswer::Overtaken) => Ok(Came::Stopped),
      Err(no) => {
        let (reason, engine) = failure_of(no);

        Ok(Came::Failed(reason, engine))
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
        let id = apart_id(&request.agent, self.numbered.fetch_add(1, Ordering::Relaxed));

        self.apart().insert(id.clone(), (request.agent.clone(), apart.clone()));
        (id, request.scope.clone())
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

        if next.provider != worker_session::PROVIDER {
          return Err(fault(
            FaultKind::Refused,
            format!("{session} goes out on {}, not {}", worker_session::PROVIDER, next.provider),
          ));
        }

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
    let wait = Duration::from_millis(RESIDENT_TURN.push_take_ms);

    Ok(match tokio::time::timeout(wait, &mut offer.answer).await {
      Ok(answered) => answered.map_or(Aside::Refused, said),
      Err(_) if offer.withdraw() => Aside::Refused,
      Err(_) => (&mut offer.answer).await.map_or(Aside::Refused, said),
    })
  }

  async fn send_turn(&self, turn: Turn) -> Result<TurnEnd, Fault> {
    let (agent, scope) = self.scope_held(&turn.session)?;
    let row = self.admitted(&agent, &scope).await?;
    let _role_place = self.role_turn(&agent).await?;
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

    if let (Scope::Worker, Some(work)) = (&scope, work)
      && work.provider != worker_session::PROVIDER
    {
      return Err(fault(
        FaultKind::Refused,
        format!(
          "{agent}: a worker's session stands on {}, not {}",
          worker_session::PROVIDER,
          work.provider
        ),
      ));
    }

    let run = match (&room, work) {
      (_, Some(work)) => work.name.clone(),
      (Some(room), None) => {
        fill_marks(&COMPANION_RUNNER.runs.turn, &[("chat", room.as_str()), ("at", &at.to_string())])
      }
      (None, None) => format!("turn:{agent}:{at}"),
    };

    let turn_id;

    {
      let mut going = self.going();
      let runs = going.entry(turn.session.clone()).or_default();

      if !runs.is_empty() {
        return Err(fault(FaultKind::Busy, format!("{} is in a turn", turn.session)));
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
      (Scope::Worker, Some(row)) => {
        let db = &self.ground().db;

        keep_worker_job(db, &agent, turn.context.job).await;

        let began = turn.context.began.unwrap_or(at);
        let came = match work {
          // The worker's road is Claude where its key stands on it; a fallback road crosses over on the
          // same stand-in walk a chat turn uses (docs/cache-fallback.md).
          Some(work) if work.provider == worker_session::PROVIDER => {
            self.carried(&turn, &agent, WorkSeat::Worker(row), work, turn_id).await
          }
          Some(work) => {
            self.worker_stand_in(&agent, work, &turn.prompt, began, turn.context.job, shown).await
          }
          None => self.worker_turn(row, &run, &turn.prompt, &turn.session, turn_id).await,
        };

        keep_worker_job(db, &agent, None).await;
        came
      }
      (Scope::Apart(apart), _) => match (&turn.context.gate, work) {
        (Some(gate), _) => {
          self.gate_turn(&turn, &agent, &apart.provider, &apart.cwd, gate, &run, shown).await
        }
        (None, Some(work)) => self.carried(&turn, &agent, WorkSeat::Apart(apart), work, turn_id).await,
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
      (Scope::Worker, None) => Err(fault(FaultKind::Unknown, format!("no worker is named {agent}"))),
      (Scope::Room(room), _) => match room_number(room) {
        Ok(chat_id) => self.room_turn(&agent, chat_id, &run, &turn.prompt, &turn.context, shown).await,
        Err(refused) => Err(refused),
      },
      (Scope::Agent, _) => self.agent_turn(&agent, &run, &turn.prompt, &turn.context, shown).await,
    };

    self.ended(&turn.session, &run);

    // A turn refused once its start was published still ends, failed with the refusal.
    match came {
      Ok(came) => {
        let spent = came.spent();
        let mut end = came.end(&run);

        end.turn = turn_id;
        close_chat_run(&self.ground().db, ledger, &end, spent).await;

        Ok(self.published(&turn.session, end))
      }
      Err(refused) => {
        let mut end = TurnEnd::failed(&run, refused.message.clone(), None);

        end.turn = turn_id;
        close_chat_run(&self.ground().db, ledger, &end, None).await;
        self.published(&turn.session, end);
        Err(refused)
      }
    }
  }

  async fn read_state(&self, session: &SessionId, model: Option<&str>) -> Result<State, Fault> {
    let (agent, scope) = self.scope_held(session)?;

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
            Some(pinned) => {
              (pinned.provider.unwrap_or_default(), pinned.companion.pinned_model.unwrap_or_default())
            }
            None => Default::default(),
          },
        };

        Ok(State::standing(
          session.clone(),
          provider,
          model,
          place,
          u64::try_from(held.as_ref().map_or(0, |held| held.generation)).unwrap_or_default(),
          None,
        ))
      }
      Scope::Agent => {
        if agent_session(&ground.db, &agent).await.map_err(store_fault)?.is_none() {
          return Err(fault(FaultKind::Unknown, format!("{session} has not been opened")));
        }

        let model = one(
          &ground.db,
          r#"SELECT "last_model" FROM "agents" WHERE "name" = $1"#,
          vec![agent.clone().into()],
        )
        .await
        .map_err(|why| store_fault(why.into()))?
        .and_then(|row| row.try_get::<Option<String>>("", "last_model").ok().flatten())
        .unwrap_or_default();

        Ok(State::standing(session.clone(), CHAT_PROVIDER.to_owned(), model, place, 1, None))
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
        let provider = provider_of(ground, &pinned).await.map_err(|why| fault(FaultKind::Refused, why))?;
        let held = sessions::session(&ground.db, &agent, chat_id).await.map_err(store_fault)?;
        let Some(harness) = held.and_then(|held| held.harness) else {
          return Err(fault(FaultKind::Unknown, format!("{session} holds no harness session to fold")));
        };
        let settings = Settings::of(&ground.db, &agent)
          .await
          .map_err(|refused| fault(FaultKind::Refused, refused.error))?;
        let system = system_of(ground, &pinned, &settings)
          .await
          .map_err(|why| fault(FaultKind::Refused, why))?;
        let due = Due {
          provider,
          system,
          generation: generation_of(&settings),
          session: harness.id,
          placement: harness.placement,
          ratio: 1.0,
        };

        hand_over(ground, &self.places.kept.reset, &pinned.companion, chat_id, due).await;
        Ok(())
      }
      Scope::Agent => Err(fault(
        FaultKind::Refused,
        format!("{session} is carried past its window by its program inside its own turns"),
      )),
      Scope::Worker => {
        let (Some(row), Some(held)) = (row, self.latest_worker(&agent).await?) else {
          return Err(fault(FaultKind::Unknown, format!("{session} has not been opened")));
        };

        match fold(ground, &row, &held.model, &held.account, &held.session_id).await {
          Folded::Compacted | Folded::TooShort => Ok(()),
          Folded::Busy => Err(fault(FaultKind::Busy, format!("a job holds {agent}'s checkout"))),
          Folded::Failed => Err(fault(FaultKind::Refused, format!("{session} could not be folded"))),
        }
      }
      Scope::Apart(_) => Err(fault(
        FaultKind::Refused,
        format!("{session} is thrown away, every turn on a fresh session"),
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
      Scope::Apart(_) => {
        self.apart().remove(session);
      }
      Scope::Conversation(_) => {
        let harness = self.conversations().remove(session).and_then(|(_, held)| held.harness);

        if let Some(harness) = harness {
          let_go(ground, &agent, &[harness]).await;
        }
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
      r#"SELECT "name" FROM "agents" WHERE "session_scope" = 'agent' AND "session_id" IS NOT NULL ORDER BY "name""#,
      vec![],
    )
    .await
    .map_err(|why| store_fault(why.into()))?;

    for row in across {
      let agent: String = row.try_get("", "name").map_err(|why| store_fault(why.into()))?;

      sessions.extend(listed(agent, Scope::Agent));
    }

    let workers = all(db, "SELECT DISTINCT worker FROM worker_sessions ORDER BY worker", vec![])
      .await
      .map_err(|why| store_fault(why.into()))?;

    for row in workers {
      let agent: String = row.try_get("", "worker").map_err(|why| store_fault(why.into()))?;

      sessions.extend(listed(agent, Scope::Worker));
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

// The worker's own session, as the contract names it.
fn worker_session_id(worker: &str) -> Result<SessionId, Fault> {
  id_of(worker, &Scope::Worker)
    .ok_or_else(|| fault(FaultKind::Unknown, format!("{worker} names no worker's session")))
}

/// The words said to a worker go into its session through the backend: pushed into the turn it is working,
/// or carried in a turn of their own that holds the session as a contract's turn does, so neither goes in
/// over the other.
impl WordRoad for HerdrSessions {
  async fn push(&self, worker: &str, words: &str) -> Result<Aside, Fault> {
    let session = worker_session_id(worker)?;
    let turn = self.going().get(&session).and_then(|runs| (runs.len() == 1).then(|| runs[0].turn));

    match turn {
      Some(turn) => Sessions::push(self, &session, turn, words).await,
      None => Ok(Aside::Refused),
    }
  }

  async fn turn<W: Watch + Send + Sync>(
    &self,
    worker: &AgentRow,
    asked: WordTurn,
    watch: &mut W,
  ) -> Result<Option<Turned>, Refusal> {
    let session = worker_session_id(&worker.name).map_err(|why| Refusal::failed(why.message))?;
    // The worker's pane lock, taken without waiting: a job of the worker's waits out the word's turn on it.
    let Some(_pane) = try_hold(&pane_lock_path(&self.ground().roots.data, &worker.name)) else {
      return Ok(None);
    };

    let turn;
    let run = asked.name.clone();

    {
      let mut going = self.going();

      if going.get(&session).is_some_and(|runs| !runs.is_empty()) {
        return Ok(None);
      }

      turn = self.next_turn();
      going.entry(session.clone()).or_default().push(Running {
        turn,
        run: run.clone(),
      });
    }
    self.outputs.begun(&session, turn, &run);

    let turned = match self.worker_session(&worker.name).await {
      Ok((model, resumed)) => {
        worker_session::turn(
          &Side {
            ground: self.ground(),
          },
          worker_session::Asked {
            worker,
            model,
            session: resumed,
            prompt: asked.prompt,
            name: asked.name.clone(),
            title: asked.title,
            label: "word".to_owned(),
            bound: Some(asked.bound),
          },
          watch,
        )
        .await
      }
      Err(why) => Err(Refusal::failed(why.message)),
    };

    if let Ok(turned) = &turned
      && let Some(reply) = &turned.reply
    {
      self.outputs.wrote(&session, turn, std::slice::from_ref(&reply.text));
    }

    self.ended(&session, &run);

    turned.map(Some)
  }
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
    started_at: at.div_euclid(1000),
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

// A chat turn's run closed as its turn ended: how it ended, the tokens its context carried and what it spent.
async fn close_chat_run(
  db: &sea_orm::DatabaseConnection,
  ledger: Option<i64>,
  end: &TurnEnd,
  spent: Option<run_ledger::Spent>,
) {
  let Some(id) = ledger else {
    return;
  };
  let closing = run_ledger::Closing {
    finished_at: now_ms().div_euclid(1000),
    outcome: serde_json::to_value(end.outcome)
      .ok()
      .and_then(|outcome| outcome.as_str().map(str::to_owned))
      .unwrap_or_else(|| "failed".to_owned()),
    context_tokens: end.reply.as_ref().and_then(|reply| reply.context_tokens),
    spent,
  };

  if let Err(run_ledger::NotClosed::Store(why)) = run_ledger::close(db, id, &closing).await {
    eprintln!("{}: the chat run could not be closed in the ledger: {}", end.run, why.detail);
  }
}
