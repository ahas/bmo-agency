//! The herdr backend of the session contract (docs/api.md, 「Sessions」): the seven operations answered over
//! the door, the places and the lines. A worker's session is its own, one per worker, carried by the
//! worker's session turn on the line a job runs on; a companion's session is one per room, carried by its
//! resident program on the chat line with the system prompt its settings compose. A session is known by its
//! agent and its scope — `<agent>@worker`, `<agent>@<room>` — and stands across the generations its harness
//! opens.
//!
//! A turn is carried whole before *send a turn* answers: whether the program took the words, or refused
//! them while it waits on a person. What it came to is published as `turn-ended`, and its start as
//! `turn-started`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agent_session::{
  Context, Delivery, Fault, FaultKind, Scope, Session, SessionId, Sessions, Start, State, Turn,
};
use indexmap::IndexMap;
use policy::companion_runner::COMPANION_RUNNER;
use policy::fill_marks;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use store::{all, one};

use crate::companion::guard::spent_out;
use crate::companion::handoff::{Companion, Due, hand_over};
use crate::companion::overtaken;
use crate::companion::run::{NoAnswer, Reply, Tally, let_go, run_turn};
use crate::companion::sessions::{self, Harness, Kept, RoomSession, harness_road};
use crate::control::claude_accounts::{Judging, account_open};
use crate::control::companions::Settings;
use crate::door::record::UsageSpent;
use crate::door::turn::Quiet;
use crate::events::Event;
use crate::lines::companion::{CompanionTurn, LOST_SESSION, compose, registration_named};
use crate::places::{AgentRow, Ground, Places};
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::worker_session::fold::{Folded, fold};
use crate::worker_session::records::{self, note_answer, slot_account};
use crate::worker_session::{self, Missed, Side};
use crate::{Refusal, now_ms};

// What a worker's session is told apart by in its id, after its agent.
const WORKER: &str = "worker";

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

/// What a turn came to: `answered` with the answer's text, `blocked` where its program waits on a person,
/// `stopped` where it was ended from outside, or `failed` with why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnEnded {
  pub session: String,
  pub run: String,
  pub outcome: &'static str,
  pub answer: Option<String>,
  pub reason: Option<String>,
}

impl Event for TurnEnded {
  const NAME: &'static str = "turn-ended";
}

/// The session contract over herdr, on the places the server stands.
pub struct HerdrSessions {
  places: Places,
  // The run each session's turn goes out as while one goes, which the interrupt stops and a second turn
  // is refused for.
  going: Mutex<HashMap<SessionId, String>>,
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
  })
}

/// The agent and scope a session's id names.
pub fn scope_of(id: &SessionId) -> (String, Scope) {
  match id.0.split_once('@') {
    None => (id.0.clone(), Scope::Agent),
    Some((agent, WORKER)) => (agent.to_owned(), Scope::Worker),
    Some((agent, room)) => (agent.to_owned(), Scope::Room(room.to_owned())),
  }
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

// The provider a companion is pinned to, where it carries a companion's answers and its model has an
// address where the registry serves it.
async fn provider_of(
  ground: &Ground,
  pinned: &Pinned,
) -> Result<&'static dyn harness_providers::Provider, String> {
  let refused = &COMPANION_RUNNER.refused;
  let (Some(provider), Some(model)) = (&pinned.provider, &pinned.companion.pinned_model) else {
    return Err(refused.no_pin.clone());
  };
  let Some(found) = harness_providers::provider(provider) else {
    return Err(fill_marks(&refused.no_provider, &[("provider", provider)]));
  };

  if !found.declared().lines.iter().any(|line| line == crate::lines::companion::LINE) {
    return Err(fill_marks(&refused.no_line, &[("label", &found.declared().label)]));
  }

  if found.not_registered().is_some() && registration_named(ground, model).await.is_err() {
    return Err(fill_marks(&refused.no_model, &[("model", model)]));
  }

  Ok(found)
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

// What a turn on a session came to, before it is published.
enum Came {
  Answered(String),
  Blocked,
  Stopped,
  Failed(String),
}

impl HerdrSessions {
  pub fn new(places: Places) -> Self {
    Self {
      places,
      going: Mutex::default(),
    }
  }

  fn ground(&self) -> &Ground {
    &self.places.ground
  }

  fn going(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, String>> {
    self.going.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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
      Scope::Agent => Err(fault(
        FaultKind::Refused,
        format!(
          "{agent}: a session across rooms goes on the chat line, whose room context its caller composes"
        ),
      )),
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
      (false, missed) => Came::Failed(format!("the words did not reach the session ({missed:?})")),
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
  async fn room_turn(&self, agent: &str, chat_id: i64, run: &str, prompt: &str) -> Result<Came, Fault> {
    let ground = self.ground();
    let db = ground.db.clone();
    let failed = |why: String| Ok(Came::Failed(why));
    let detail = |refusal: Refusal| refusal.detail;
    let Some(pinned) = pinned(ground, agent).await? else {
      return Err(fault(FaultKind::Unknown, format!("no agent is named {agent}")));
    };
    let provider = match provider_of(ground, &pinned).await {
      Ok(provider) => provider,
      Err(why) => return failed(why),
    };
    let began = now_ms();
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
          &prompt,
          &generation,
          guarded,
          began,
        )
        .await;
    }

    let _keeping = hold.keeping().await;
    let (reply, placement) = match asked {
      Ok(asked) if !overtaken::overtaken(&db, agent, began).await => asked,
      Ok(_) | Err(NoAnswer::Overtaken) => return Ok(Came::Stopped),
      Err(NoAnswer::Cut(reason) | NoAnswer::Failed(reason) | NoAnswer::Errored { error: reason, .. }) => {
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
        return failed(reason);
      }
    };

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
        privileged: false,
        resume,
        run: &run,
        answers: &[],
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

  fn published(&self, session: &SessionId, run: &str, came: &Came) {
    let (outcome, answer, reason) = match came {
      Came::Answered(text) => ("answered", Some(text.clone()), None),
      Came::Blocked => ("blocked", None, None),
      Came::Stopped => ("stopped", None, None),
      Came::Failed(why) => ("failed", None, Some(why.clone())),
    };

    self.ground().events.publish(TurnEnded {
      session: session.0.clone(),
      run: run.to_owned(),
      outcome,
      answer,
      reason,
    });
  }

  // A worker's latest session record, the one a fold or a reading of its state goes by.
  async fn latest_worker(&self, worker: &str) -> Result<Option<records::WorkerSession>, Fault> {
    Ok(records::sessions(&self.ground().db, worker).await.map_err(store_fault)?.pop())
  }
}

impl Sessions for HerdrSessions {
  async fn start(&self, request: Start) -> Result<Session, Fault> {
    self.checked(&request.agent, &request.scope).await?;

    Ok(Session {
      id: id_of(&request.agent, &request.scope),
      agent: request.agent,
      scope: request.scope,
    })
  }

  async fn send_turn(&self, turn: Turn) -> Result<Delivery, Fault> {
    let (agent, scope) = scope_of(&turn.session);
    let row = self.checked(&agent, &scope).await?;
    let at = now_ms();
    let run = match &scope {
      Scope::Room(room) => {
        fill_marks(&COMPANION_RUNNER.runs.turn, &[("chat", room.as_str()), ("at", &at.to_string())])
      }
      _ => format!("turn:{agent}:{at}"),
    };

    {
      let mut going = self.going();

      if going.contains_key(&turn.session) {
        return Err(fault(FaultKind::Busy, format!("{} is in a turn", turn.session)));
      }

      going.insert(turn.session.clone(), run.clone());
    }

    self.ground().events.publish(TurnStarted {
      session: turn.session.0.clone(),
      run: run.clone(),
      at,
    });

    let came = match &scope {
      Scope::Worker => self.worker_turn(&row, &run, &turn.prompt).await,
      Scope::Room(room) => match room_number(room) {
        Ok(chat_id) => self.room_turn(&agent, chat_id, &run, &turn.prompt).await,
        Err(refused) => Err(refused),
      },
      Scope::Agent => Err(fault(FaultKind::Refused, "a session across rooms is not carried here")),
    };

    self.going().remove(&turn.session);

    let came = came?;

    self.published(&turn.session, &run, &came);

    Ok(if matches!(came, Came::Blocked) { Delivery::Blocked } else { Delivery::Taken })
  }

  async fn read_state(&self, session: &SessionId) -> Result<State, Fault> {
    let (agent, scope) = scope_of(session);

    self.checked(&agent, &scope).await?;

    let ground = self.ground();
    let place = crate::lines::place::place_of(&ground.roots.data, &agent).to_string_lossy().into_owned();

    match &scope {
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

        Ok(State {
          session: session.clone(),
          provider,
          model,
          place,
          generation: u64::try_from(held.generation).unwrap_or_default(),
          context: None,
        })
      }
      _ => {
        let Some(held) = self.latest_worker(&agent).await? else {
          return Err(fault(FaultKind::Unknown, format!("{session} has not been opened")));
        };

        Ok(State {
          session: session.clone(),
          provider: harness_providers::Provider::name(&harness_providers::claude_code::ClaudeCode)
            .to_owned(),
          model: held.answered_model.unwrap_or(held.model),
          place,
          generation: 1,
          context: None::<Context>,
        })
      }
    }
  }

  async fn interrupt(&self, session: &SessionId) -> Result<(), Fault> {
    let Some(run) = self.going().get(session).cloned() else {
      return Ok(());
    };
    let stops = crate::queue::stops::Stops::of(self.ground(), None);

    stops.cancel(&run).await.map_err(store_fault)?;

    Ok(())
  }

  async fn fold(&self, session: &SessionId) -> Result<(), Fault> {
    let (agent, scope) = scope_of(session);
    let row = self.checked(&agent, &scope).await?;
    let ground = self.ground();

    if self.going().contains_key(session) {
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
      _ => {
        let Some(held) = self.latest_worker(&agent).await? else {
          return Err(fault(FaultKind::Unknown, format!("{session} has not been opened")));
        };

        match fold(ground, &row, &held.model, &held.account, &held.session_id).await {
          Folded::Compacted | Folded::TooShort => Ok(()),
          Folded::Busy => Err(fault(FaultKind::Busy, format!("a job holds {agent}'s checkout"))),
          Folded::Failed => Err(fault(FaultKind::Refused, format!("{session} could not be folded"))),
        }
      }
    }
  }

  async fn end(&self, session: &SessionId) -> Result<(), Fault> {
    let (agent, scope) = scope_of(session);

    self.checked(&agent, &scope).await?;

    if self.going().contains_key(session) {
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
      _ => {
        let place = crate::lines::place::Place::of(&ground.roots.data, &agent);

        place.release().await;

        for held in records::sessions(&ground.db, &agent).await.map_err(store_fault)? {
          records::forget(&ground.db, &agent, &held.model, Some(&held.account))
            .await
            .map_err(store_fault)?;
        }
      }
    }

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

    Ok(listed)
  }
}

/// The backend shared by the routes that answer the contract.
pub type Shared = Arc<HerdrSessions>;
