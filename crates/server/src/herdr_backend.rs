//! The herdr backend of the session contract (docs/api.md, 「Sessions」): the ten operations answered over
//! the door, the places and the lines. A worker's session is its own, one per worker, carried by the
//! worker's session turn on the line a job runs on; a companion's session is one per room, carried by its
//! resident program on the chat line with the system prompt its settings compose; a session across rooms is
//! the one its agent's row keeps, carried by its resident program on the chat line around the room block,
//! the road and the base persona the turn's caller hands in, or by a stand-in where the agent's key walks onto
//! one; an editor work key's session carries one editor run a turn and keeps nothing; and a thrown-away
//! session, held in memory from its start to its end, carries a co-work branch's or a research run's turns,
//! each on a fresh harness session. A session is known by its agent and its scope — `<agent>@worker`,
//! `<agent>@<room>`, `<agent>`, `<key>@editor`, `<agent>@apart:<n>` — and stands across the generations its
//! harness opens.
//!
//! A turn is carried whole before *send a turn* answers with its end: whether the program took the words,
//! or refused them while it waits on a person, and how it ended. Its end is published as `turn-ended`, and
//! its start as
//! `turn-started`; a conversational turn's road as `road-walked`. Words pushed into a session are offered to
//! the run its turn carries, which its door takes at its next look at the working program.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use agent_session::EngineError;
use agent_session::{
  Answer, Apart, Aside, Context, Fault, FaultKind, Outcome, Output, RoomBlock, Scope, Session, SessionId,
  Sessions, Start, State, Turn, TurnContext, TurnEnd, WentOut, WorkParts,
};
use indexmap::IndexMap;
use policy::companion_runner::COMPANION_RUNNER;
use policy::fill_marks;
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
use crate::door::turn::{AsideOutcome, Quiet};
use crate::editor_session::{EditorTurn, composed as editor_composed, editor_turn, is_editor, workdir_of};
use crate::events::Event;
use crate::job_run::claude::turn::{Shows, WorkAsked, WorkSeat, carry, continues};
use crate::lines::chat::{ChatLaunch, RoomContext, compose_turn};
use crate::lines::companion::{CompanionTurn, LOST_SESSION, compose, pinned_provider, registration_named};
use crate::lines::editor::PROVIDER as EDITOR_PROVIDER;
use crate::places::{AgentRow, Ground, Places};
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::session_output::Outputs;
use crate::session_questions::Questions;
use crate::watches::machine::keep_worker_job;
use crate::worker_session::fold::{Folded, fold};
use crate::worker_session::records::{self, note_answer, slot_account};
use crate::worker_session::{self, Missed, Side};
use crate::{Refusal, now_ms};

// What a worker's session is told apart by in its id, after its agent.
const WORKER: &str = "worker";

// What an editor work key's session is told apart by in its id, after its key.
const EDITOR: &str = "editor";

// What a thrown-away session is told apart by in its id, after its agent.
const APART: &str = "apart";

/// A turn put into a session began: the session and the run it goes out as.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStarted {
  pub session: String,
  pub run: String,
  pub at: i64,
}

impl Event for TurnStarted {
  const NAME: &'static str = "turn-started";
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
  const NAME: &'static str = "turn-ended";
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
  // The runs each session's turns go out as while they go, which the interrupt stops: one a session, a second
  // turn refused while it goes, but an editor's session, which carries several at once.
  going: Mutex<HashMap<SessionId, Vec<String>>>,
  // What each session's turn going now shows of itself to a reading of its state: what it went out on,
  // once its line is settled.
  showing: Mutex<HashMap<SessionId, WentOut>>,
  // The thrown-away sessions started and not ended, each with its agent and its scope, and the number the
  // next one is given.
  apart: Mutex<BTreeMap<SessionId, (String, Apart)>>,
  numbered: AtomicU64,
  // The questions the sessions' turns stand on, put to their callers.
  questions: Questions,
  // The sessions whose turn going now hears no words pushed into it.
  deaf: Mutex<HashSet<SessionId>>,
  // What each session's latest job turn wrote of its answer.
  outputs: Outputs,
}

fn fault(kind: FaultKind, message: impl Into<String>) -> Fault {
  Fault::new(kind, message)
}

fn store_fault(refusal: Refusal) -> Fault {
  fault(FaultKind::Unreachable, refusal.detail)
}

/// A session's id: its agent and its scope.
pub fn id_of(agent: &str, scope: &Scope) -> SessionId {
  SessionId(match scope {
    Scope::Agent => agent.to_owned(),
    Scope::Room(room) => format!("{agent}@{room}"),
    Scope::Worker => format!("{agent}@{WORKER}"),
    Scope::Editor => format!("{agent}@{EDITOR}"),
    // A thrown-away session is numbered by the backend that holds it, as it starts.
    Scope::Apart(_) => format!("{agent}@{APART}"),
  })
}

/// The agent and scope a session's id names.
pub fn scope_of(id: &SessionId) -> (String, Scope) {
  match id.0.split_once('@') {
    None => (id.0.clone(), Scope::Agent),
    Some((agent, WORKER)) => (agent.to_owned(), Scope::Worker),
    Some((agent, EDITOR)) => (agent.to_owned(), Scope::Editor),
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
}

impl Shows for Showing<'_> {
  fn went_out(&self, went_out: &WentOut) {
    self.sessions.showing().insert(self.session.clone(), went_out.clone());
    self.sessions.outputs.attempted(self.session);
  }

  fn wrote(&self, texts: &[String]) {
    self.sessions.outputs.wrote(self.session, texts);
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
  id.0
    .split_once('@')
    .and_then(|(_, scope)| scope.strip_prefix(APART))
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
  Answered(String),
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
      Self::Answered(text) => TurnEnd::answered(run, text),
      Self::Blocked => TurnEnd::of(run, Outcome::Blocked),
      Self::Stopped => TurnEnd::of(run, Outcome::Stopped),
      Self::Failed(why, engine) => TurnEnd::failed(run, why, engine),
      Self::Ended(end) => *end,
    }
  }
}

/// Why a run came to no answer, and the engine's error where its engine ended it with one.
pub fn failure_of(no: NoAnswer) -> (String, Option<EngineError>) {
  match no {
    NoAnswer::Errored { error, kind } => (error, Some(EngineError { kind })),
    NoAnswer::Cut(why) | NoAnswer::Failed(why) => (why, None),
    NoAnswer::Overtaken => ("the companion's reset overtook the turn".to_owned(), None),
  }
}

impl HerdrSessions {
  pub fn new(places: Places) -> Self {
    Self {
      places,
      going: Mutex::default(),
      showing: Mutex::default(),
      apart: Mutex::default(),
      numbered: AtomicU64::new(1),
      questions: Questions::default(),
      deaf: Mutex::default(),
      outputs: Outputs::default(),
    }
  }

  fn deaf(&self) -> std::sync::MutexGuard<'_, HashSet<SessionId>> {
    self.deaf.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  fn showing(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, WentOut>> {
    self.showing.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  fn apart(&self) -> std::sync::MutexGuard<'_, BTreeMap<SessionId, (String, Apart)>> {
    self.apart.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  // The agent and scope a session's id names: a thrown-away session's as it was started, which one that
  // has ended or was never started names none of.
  fn scope_held(&self, id: &SessionId) -> Result<(String, Scope), Fault> {
    if !is_apart(id) {
      return Ok(scope_of(id));
    }

    match self.apart().get(id) {
      Some((agent, apart)) => Ok((agent.clone(), Scope::Apart(apart.clone()))),
      None => Err(fault(FaultKind::Unknown, format!("{id} is no thrown-away session standing now"))),
    }
  }

  fn ground(&self) -> &Ground {
    &self.places.ground
  }

  fn going(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, Vec<String>>> {
    self.going.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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
    self.outputs.ended(session);

    let mut going = self.going();

    if let Some(runs) = going.get_mut(session) {
      runs.retain(|going| going != run);

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
        if conversational.is_some_and(|settings| settings.scope == crate::conversational::Scope::Agent) =>
      {
        Ok(row)
      }
      Scope::Agent => Err(fault(FaultKind::Refused, format!("{agent} keeps no session across rooms"))),
      Scope::Editor => {
        Err(fault(FaultKind::Refused, format!("{agent} is an editor's work key, not an agent")))
      }
      Scope::Apart(_) => Err(apart_refused(agent)),
    }
  }

  // The agent a session's scope stands on, where it stands on one; an editor's session stands on its work
  // key alone, which must be an editor's, and a thrown-away session on the name its turns run under, which
  // is no agent's row.
  async fn admitted(&self, agent: &str, scope: &Scope) -> Result<Option<AgentRow>, Fault> {
    match scope {
      Scope::Editor if is_editor(agent) => Ok(None),
      Scope::Editor => Err(fault(FaultKind::Refused, format!("{agent} is no editor's work key"))),
      Scope::Apart(apart) if apart.provider != worker_session::PROVIDER => Err(fault(
        FaultKind::Refused,
        format!(
          "{agent}: a thrown-away session goes out on {}, not {}",
          worker_session::PROVIDER,
          apart.provider
        ),
      )),
      Scope::Apart(_) => Ok(None),
      _ => self.checked(agent, scope).await.map(Some),
    }
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

  async fn worker_turn(&self, row: &AgentRow, run: &str, prompt: &str) -> Result<Came, Fault> {
    let ground = self.ground();
    let (model, session) = self.worker_session(&row.name).await?;
    let turned = worker_session::turn(
      &Side { ground },
      worker_session::Asked {
        worker: row,
        model,
        session,
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

    Ok(match (turned.reached, turned.missed) {
      (_, Some(Missed::Person)) => Came::Blocked,
      (_, Some(Missed::Stopped)) => Came::Stopped,
      (false, missed) => Came::Failed(format!("the words did not reach the session ({missed:?})"), None),
      (true, _) => Came::Answered(turned.reply.map(|reply| reply.text).unwrap_or_default()),
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
        ground.events.publish(SystemPromptChanged {
          session: id_of(agent, &Scope::Room(chat_id.to_string())).0,
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

    // A turn whose context reached the mark hands its session over before the next one, where its provider
    // declares the handoff.
    let hands_off = provider
      .declared()
      .resident
      .as_ref()
      .is_some_and(|resident| resident.handover == policy::providers::Handover::Handoff);

    if let Some(ratio) = reply
      .context_ratio
      .filter(|ratio| hands_off && *ratio >= policy::resident_turn::RESIDENT_TURN.fold_ratio)
    {
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

    Ok(Came::Answered(reply.text))
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
      provider.name(),
      &composed,
      resume.map(|(id, _)| id),
      log,
      guarded,
      Some(&tally),
    )
    .await?;

    Ok((reply, composed.placement))
  }

  // One turn of an agent across rooms, carried by its resident program on the chat line around the room
  // block, the road and the base persona its caller handed in, on the one session its row keeps; a session
  // its harness no longer holds is let go, and the turn asked again once on a fresh one.
  async fn agent_turn(
    &self,
    agent: &str,
    run: &str,
    prompt: &str,
    context: &TurnContext,
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
    // stays as it stands for its next turn.
    let stood_in = stand_in_turn(
      ground,
      &mut log,
      &launch_of(agent, room, context, None),
      prompt,
      &context.answers,
      began,
    )
    .await;
    let on_claude = matches!(stood_in, Ok(None));
    let asked = match stood_in {
      Ok(Some(reply)) => Ok(reply),
      Err(no) => Err(no),
      Ok(None) => {
        let mut asked = self.agent_run(&mut log, agent, room, context, prompt, held.clone(), began).await;

        if let Err(NoAnswer::Failed(said)) = &asked
          && held.is_some()
          && said.contains(LOST_SESSION)
        {
          keep_agent_session(&db, agent, None).await.map_err(store_fault)?;
          asked = self
            .agent_run(&mut log, agent, room, context, &aside::carried(run, prompt), None, began)
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

    Ok(Came::Answered(reply.text))
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
  ) -> Result<Reply, NoAnswer> {
    let ground = self.ground();
    let launch = launch_of(agent, room, context, resume.clone());
    let run = log.run().to_owned();
    let composed = compose_turn(ground, &ground.machine(), &launch, prompt, &run, &context.answers, began)
      .await
      .map_err(|refused| NoAnswer::Failed(refused.detail))?;

    log.launched(&ground.db, Some(&composed.system_path), prompt, &[]).await;

    run_turn(ground, agent, CHAT_PROVIDER, &composed, resume.as_deref(), log, false, None).await
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
      Some([run]) => Some(run.clone()),
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
    };

    self.outputs.begun(&turn.session, &work.name);
    carry(&asked, &shown).await.map(|end| Came::Ended(Box::new(end)))
  }

  // A worker's latest session record, the one a fold or a reading of its state goes by.
  async fn latest_worker(&self, worker: &str) -> Result<Option<records::WorkerSession>, Fault> {
    Ok(records::sessions(&self.ground().db, worker).await.map_err(store_fault)?.pop())
  }
}

impl Sessions for HerdrSessions {
  async fn start(&self, request: Start) -> Result<Session, Fault> {
    self.admitted(&request.agent, &request.scope).await?;

    let id = match &request.scope {
      Scope::Apart(apart) => {
        if request.agent.trim().is_empty() || request.agent.contains('@') {
          return Err(fault(FaultKind::Refused, format!("{} is no name a turn runs under", request.agent)));
        }

        let id = apart_id(&request.agent, self.numbered.fetch_add(1, Ordering::Relaxed));

        self.apart().insert(id.clone(), (request.agent.clone(), apart.clone()));
        id
      }
      scope => id_of(&request.agent, scope),
    };

    Ok(Session {
      id,
      agent: request.agent,
      scope: request.scope,
    })
  }

  async fn push(&self, session: &SessionId, words: &str) -> Result<Aside, Fault> {
    let (agent, scope) = self.scope_held(session)?;

    self.admitted(&agent, &scope).await?;

    // A job's turn that hears no words takes none.
    if self.deaf().contains(session) {
      return Ok(Aside::Refused);
    }

    // The run the session's turn carries now; a session carrying several, an editor's, takes words into none.
    let run = match self.going().get(session).map(Vec::as_slice) {
      Some([run]) => run.clone(),
      _ => return Ok(Aside::Refused),
    };
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
    let at = now_ms();
    let room = match &scope {
      Scope::Room(room) => Some(room.clone()),
      Scope::Agent => turn.context.room.as_ref().map(|room| room.chat_id.to_string()),
      Scope::Worker | Scope::Editor | Scope::Apart(_) => None,
    };
    let parts = match scope {
      Scope::Editor => turn.context.run.as_ref(),
      _ => None,
    };

    if let Err(why) = workdir_of(parts) {
      return Err(fault(FaultKind::Refused, why));
    }

    // A job's turn goes out under the run name its runner gives it, on Claude Code alone; a thrown-away
    // session carries nothing else.
    let work = match scope {
      Scope::Worker | Scope::Apart(_) => turn.context.work.as_ref(),
      _ => None,
    };

    if matches!(scope, Scope::Apart(_)) && work.is_none() {
      return Err(fault(
        FaultKind::Refused,
        format!("{}: a thrown-away session's turn names its work parts", turn.session),
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

    let run = match (room, parts, work) {
      (_, Some(parts), _) => parts.name.clone(),
      (_, None, Some(work)) => work.name.clone(),
      (Some(room), None, None) => {
        fill_marks(&COMPANION_RUNNER.runs.turn, &[("chat", room.as_str()), ("at", &at.to_string())])
      }
      (None, None, None) => format!("turn:{agent}:{at}"),
    };

    {
      let mut going = self.going();
      let runs = going.entry(turn.session.clone()).or_default();
      let several = matches!(scope, Scope::Editor);

      if runs.contains(&run) || (!several && !runs.is_empty()) {
        return Err(fault(FaultKind::Busy, format!("{} is in a turn", turn.session)));
      }

      runs.push(run.clone());
    }

    if !hears_words(work) {
      self.deaf().insert(turn.session.clone());
    }

    self.ground().events.publish(TurnStarted {
      session: turn.session.0.clone(),
      run: run.clone(),
      at,
    });

    let came = match (&scope, &row) {
      (Scope::Worker, Some(row)) => {
        let db = &self.ground().db;

        keep_worker_job(db, &agent, turn.context.job).await;

        let came = match work {
          Some(work) => self.carried(&turn, &agent, WorkSeat::Worker(row), work).await,
          None => self.worker_turn(row, &run, &turn.prompt).await,
        };

        keep_worker_job(db, &agent, None).await;
        came
      }
      (Scope::Apart(apart), _) => match work {
        Some(work) => self.carried(&turn, &agent, WorkSeat::Apart(apart), work).await,
        None => Err(apart_refused(&agent)),
      },
      (Scope::Worker, None) => Err(fault(FaultKind::Unknown, format!("no worker is named {agent}"))),
      (Scope::Editor, _) => {
        let asked = EditorTurn {
          key: &agent,
          run: &run,
          prompt: &turn.prompt,
          parts,
        };

        Ok(match editor_turn(self.ground(), &asked).await {
          Ok(answer) => Came::Answered(answer),
          Err(why) => Came::Failed(why, None),
        })
      }
      (Scope::Room(room), _) => match room_number(room) {
        Ok(chat_id) => self.room_turn(&agent, chat_id, &run, &turn.prompt, &turn.context).await,
        Err(refused) => Err(refused),
      },
      (Scope::Agent, _) => self.agent_turn(&agent, &run, &turn.prompt, &turn.context).await,
    };

    self.ended(&turn.session, &run);

    // A turn refused once its start was published still ends, failed with the refusal.
    match came {
      Ok(came) => Ok(self.published(&turn.session, came.end(&run))),
      Err(refused) => {
        self.published(&turn.session, TurnEnd::failed(&run, refused.message.clone(), None));
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
      Scope::Editor => {
        let editor = editor_composed(ground, &agent).await.map_err(|why| fault(FaultKind::Refused, why))?;

        Ok(State::standing(
          session.clone(),
          EDITOR_PROVIDER.to_owned(),
          editor.model,
          editor.cwd.to_string_lossy().into_owned(),
          1,
          None,
        ))
      }
      Scope::Room(room) => {
        let held = sessions::session(&ground.db, &agent, room_number(room)?).await.map_err(store_fault)?;
        let Some(held) = held else {
          return Err(fault(FaultKind::Unknown, format!("{session} has not been opened")));
        };
        let (provider, model) = held
          .harness
          .as_ref()
          .and_then(|harness| harness.road.split_once("::"))
          .map(|(provider, model)| (provider.to_owned(), model.to_owned()))
          .unwrap_or_default();

        Ok(State::standing(
          session.clone(),
          provider,
          model,
          place,
          u64::try_from(held.generation).unwrap_or_default(),
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
        let mut state = State::standing(
          session.clone(),
          worker_session::PROVIDER.to_owned(),
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

  // A worker's and a thrown-away session's job turns keep what they write; every other scope's turn is
  // read by its own events and keeps nothing here.
  async fn output(&self, session: &SessionId) -> Result<Output, Fault> {
    let (agent, scope) = self.scope_held(session)?;

    self.admitted(&agent, &scope).await?;

    match scope {
      Scope::Worker | Scope::Apart(_) => self
        .outputs
        .read(session)
        .ok_or_else(|| fault(FaultKind::Unknown, format!("{session} has carried no job turn to read"))),
      _ => Err(fault(FaultKind::Refused, format!("{session} keeps no record of a turn to read"))),
    }
  }

  async fn interrupt(&self, session: &SessionId) -> Result<(), Fault> {
    let runs = self.going().get(session).cloned().unwrap_or_default();
    let stops = crate::queue::stops::Stops::of(self.ground(), None);

    for run in &runs {
      stops.cancel(run).await.map_err(store_fault)?;
    }

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
      Scope::Editor => Err(fault(FaultKind::Refused, format!("{session} keeps no session to fold"))),
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
      Scope::Editor => {}
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
    }

    self.outputs.forget(session);

    Ok(())
  }

  async fn list(&self) -> Result<Vec<Session>, Fault> {
    let db = &self.ground().db;
    let mut listed = vec![];
    let rooms = all(db, "SELECT agent, chat_id FROM room_sessions ORDER BY agent, chat_id", vec![])
      .await
      .map_err(|why| store_fault(why.into()))?;

    for row in rooms {
      let agent: String = row.try_get("", "agent").map_err(|why| store_fault(why.into()))?;
      let chat_id: i64 = row.try_get("", "chat_id").map_err(|why| store_fault(why.into()))?;
      let scope = Scope::Room(chat_id.to_string());

      listed.push(Session {
        id: id_of(&agent, &scope),
        agent,
        scope,
      });
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

      listed.push(Session {
        id: id_of(&agent, &Scope::Agent),
        agent,
        scope: Scope::Agent,
      });
    }

    let workers = all(db, "SELECT DISTINCT worker FROM worker_sessions ORDER BY worker", vec![])
      .await
      .map_err(|why| store_fault(why.into()))?;

    for row in workers {
      let agent: String = row.try_get("", "worker").map_err(|why| store_fault(why.into()))?;

      listed.push(Session {
        id: id_of(&agent, &Scope::Worker),
        agent,
        scope: Scope::Worker,
      });
    }

    for (id, (agent, apart)) in self.apart().iter() {
      listed.push(Session {
        id: id.clone(),
        agent: agent.clone(),
        scope: Scope::Apart(apart.clone()),
      });
    }

    Ok(listed)
  }
}

// A turn across rooms as the chat line launches it: the room's block, the road and the base persona its
// caller handed in, on the session named.
fn launch_of<'a>(
  agent: &'a str,
  room: &RoomBlock,
  context: &'a TurnContext,
  resume: Option<String>,
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
  }
}

/// The backend shared by the routes that answer the contract.
pub type Shared = Arc<HerdrSessions>;
