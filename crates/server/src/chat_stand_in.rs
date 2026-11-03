//! The chat's turn across rooms on a stand-in (docs/cache-fallback.md, 「While a fallback stands, the room is
//! not silent」): where the agent's own key walks onto a provider before Claude Code, or every login of
//! Claude Code is closed and a stand-in answers, the turn goes out there — Codex on the agent's kept thread,
//! OpenRouter through a tap of its own, a registered model's Claude Code on the gateway — as that provider's
//! program standing in the agent's pane. A stand-in answering where the key walks is published as the
//! fallback it is; Claude Code answering again while a fallback stands clears it and publishes its end.

use agent_session::GateAsk;
use harness_providers::Machine;
use policy::providers::Turns;
use policy::providers::registered::REGISTERED;
use serde::Serialize;

use crate::cards;
use crate::control::harness::provider_enabled;
use crate::control::registry;
use crate::events::Event;
use crate::job_run::claude::failure::request_refused;
use crate::lines::chat::{ChatLaunch, ChatRun, Door, chat_run, compose_stand_in};
use crate::lines::codex::lost_thread;
use crate::lines::standing::{GatewayModel, member_model, place_registered};
use crate::places::Ground;
use crate::queue::settings;
use crate::road::{self, Closed};
use crate::run_log::RunLog;
use crate::run_log::lines::Line;
use crate::session_output::OutputWatch;
use crate::stand_in::{CLAUDE, Step, announce, listed, step_of};
use crate::supervisor_turn::run::{NoAnswer, Reply, run_turn};
use crate::{Refusal, now_ms};
use store::{one, run as written};

const CODEX: &str = "codex";

// Whether a provider's turns go through its pane program's inbox, as its declaration says.
pub(crate) fn inbox_declared(provider: &str) -> bool {
  harness_providers::provider(provider)
    .and_then(|declared| declared.declared().resident.as_ref())
    .is_some_and(|resident| resident.turns == Turns::Inbox)
}

/// Whether a provider's program carries its own turns, as its declaration says: a surface of its own puts a
/// turn in and reads it back, so a run on it stands in a place of its own rather than walking Claude Code's
/// logins — an inbox's, a member road's and the registry's provider's alike.
pub fn carries_own_turns(provider: &str) -> bool {
  harness_providers::provider(provider)
    .and_then(|declared| declared.declared().resident.as_ref())
    .is_some_and(|resident| resident.turns != Turns::Transcript)
}

// Whether a provider's turns are Claude Code on the gateway under a member of its own road. The registry's
// provider is Claude Code too, but its models are registrations, not members.
pub(crate) fn member_declared(provider: &str) -> bool {
  provider != registry::PROVIDER
    && harness_providers::provider(provider)
      .and_then(|declared| declared.declared().resident.as_ref())
      .is_some_and(|resident| resident.turns == Turns::Endpoint)
}

// The variable the owner's id stands in, which the fallback's events carry.
const OWNER_VARIABLE: &str = "BMO_OWNER_ID";

/// A standing fallback has ended: Claude Code answered again (docs/api.md, 「Events」), the owner it is told
/// to and the stand-in that answered last. What the owner is told is the consumer's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackEnded {
  pub owner_id: i64,
  pub previous: Option<String>,
}

impl Event for FallbackEnded {
  const NAME: &'static str = "fallback.changed";
}

/// The owner the fallback's events are told to, as the deployment names them; none is nought.
pub fn owner_id(ground: &Ground) -> i64 {
  ground.vars().get(OWNER_VARIABLE).and_then(|id| id.trim().parse().ok()).unwrap_or(0)
}

fn failed(why: impl Into<String>) -> NoAnswer {
  NoAnswer::Failed(why.into())
}

fn refusal(why: Refusal) -> NoAnswer {
  failed(why.detail)
}

// Why a walk stopped from outside went out on no further road.
const STOPPED: &str = "The turn was stopped from outside, so it went out on no further provider.";

// Whether the turn a walk goes out for was stopped from outside: its interrupt stands a cancellation over
// its run (docs/cache-fallback.md, 「Automatic walks only on providers, in order」).
async fn stopped(ground: &Ground, log: &RunLog) -> bool {
  crate::door::execution::cancelled(&ground.db, &[log.run()]).await.unwrap_or(false)
}

// A line of the turn's log under the stand-in's own name, saying why a step did not answer.
async fn note_failed(ground: &Ground, log: &mut RunLog, step: &Step, why: &str) {
  log
    .write(
      &ground.db,
      Line {
        source: Some(step.provider.clone()),
        state: Some("failed"),
        ..Line::note(format!("{} · {} did not answer — {why}", step.provider, step.model))
      },
    )
    .await;
}

/// The turn across rooms on the road resolution's walk of the agent's key (docs/cache-fallback.md, 「One road
/// resolution」): each road of it that can answer now in turn, until one answers. None where the walk reaches
/// Claude Code, whose own session carries the turn — and where no stand-in answered while Claude Code is on,
/// whose own failure then says why; the stand-in's reply where one answered; and the closed road's sentence
/// where nothing of the order answered and Claude Code is off. A road that failed for the work's own sake —
/// its request refused for its shape — ends the turn with that failure, and a turn stopped from outside
/// goes out on no further road.
#[allow(clippy::too_many_arguments)]
pub async fn stand_in_turn(
  ground: &Ground,
  log: &mut RunLog,
  run: &ChatRun,
  key: &str,
  answers: &[i64],
  began: i64,
  shown: Option<OutputWatch<'_>>,
) -> Result<Option<Reply>, NoAnswer> {
  let machine = ground.machine();
  let agent = run.agent.as_str();
  let supervisor = crate::supervisors::named(&ground.db, agent)
    .await
    .map_err(|why| failed(why.detail))?
    .ok_or_else(|| failed(format!("No supervisor named {agent} is held")))?;
  let ask = road::Ask::of(road::Row::Supervisor(agent), "chat", &supervisor.order);
  let resolution = road::resolve(ground, &ask).await.map_err(failed)?;
  let chosen = "auto".to_owned();
  let mut tried: Vec<Closed> = vec![];
  let mut noted = false;

  for taken in resolution.open() {
    if stopped(ground, log).await {
      return Err(failed(STOPPED));
    }

    if taken.provider == CLAUDE {
      road::went(ground, &resolution, Some(taken), &tried).await;

      return Ok(None);
    }

    if !noted {
      run.note_persona(ground, log).await;
      noted = true;
    }

    let step = match road_step(ground, &machine, key, taken, &run.reasoning, None).await {
      Ok(step) => step,
      Err(why) => {
        tried.push(Closed::plain(&taken.provider, why));
        continue;
      }
    };
    let outcome = match taken.provider.as_str() {
      name if inbox_declared(name) => {
        inbox_road(ground, &machine, run, &step, log, answers, began, shown).await?
      }
      name if member_declared(name) => {
        member_road(ground, &machine, run, &step, name, log, answers, began, shown).await?
      }
      _ => registered_road(ground, &machine, run, &step, log, answers, began, shown).await?,
    };

    match outcome {
      // The road left for its provider's failure is blocked as the walk goes on, so the next run passes it
      // over (docs/provider-order.md, 「Falling over and coming back」).
      Err(why) => {
        note_failed(ground, log, &step, &why).await;
        road::leave(ground, taken, &why).await;
        tried.push(Closed::plain(&taken.provider, why));
      }
      Ok(reply) => {
        let provider = &taken.provider;

        if provider != &chosen
          && let Err(why) = announce(ground, &ground.events, owner_id(ground), provider, &step.model).await
        {
          eprintln!("fallback {agent}: its answering on {provider} was not marked: {}", why.detail);
        }

        road::went(ground, &resolution, Some(taken), &tried).await;

        return Ok(Some(reply));
      }
    }
  }

  road::went(ground, &resolution, None, &tried).await;

  // Claude Code on, though it cannot answer now: its own failure, the limit it rests on among them,
  // carries the turn's end as it always has.
  if provider_enabled(&ground.db, CLAUDE).await.map_err(|why| failed(why.error))? {
    return Ok(None);
  }

  let mut sentence = resolution.sentence();

  for closed in &tried {
    sentence.push_str(&format!("\n{}: {}", closed.provider, closed.reason));
  }

  Err(failed(sentence))
}

/// The step a road goes out on for a key: the model and effort the key maps to on its provider, and the
/// road's own model where it names another — a registration of its own on the registry's provider.
pub async fn road_step(
  ground: &Ground,
  machine: &Machine,
  key: &str,
  taken: &road::Road,
  reasoning: &crate::lines::worker::Reasoning,
  supervisor: Option<&str>,
) -> Result<Step, String> {
  let mut step = step_of(ground, machine, key, &taken.provider, reasoning, supervisor).await?;

  if step.model != taken.model {
    if taken.provider == registry::PROVIDER {
      return crate::stand_in::registration_step(ground, key, &taken.model, reasoning).await;
    }

    step.model.clone_from(&taken.model);
  }

  Ok(step)
}

/// One worker's job turn carried on the road its launch resolved, on the worker line the run composed: the
/// road's own program standing in the worker's pane, never another provider's.
#[allow(clippy::too_many_arguments)]
pub async fn job_turn_on(
  ground: &Ground,
  log: &mut RunLog,
  run: &ChatRun,
  key: &str,
  taken: &road::Road,
  began: i64,
  shown: Option<OutputWatch<'_>>,
  supervisor: Option<&str>,
) -> Result<Reply, NoAnswer> {
  let machine = ground.machine();
  let step = road_step(ground, &machine, key, taken, &run.reasoning, supervisor)
    .await
    .map_err(failed)?;

  run.note_persona(ground, log).await;

  let outcome = match taken.provider.as_str() {
    name if inbox_declared(name) => {
      inbox_road(ground, &machine, run, &step, log, &[], began, shown).await?
    }
    name if member_declared(name) => {
      member_road(ground, &machine, run, &step, name, log, &[], began, shown).await?
    }
    _ => registered_road(ground, &machine, run, &step, log, &[], began, shown).await?,
  };

  match outcome {
    Ok(reply) => Ok(reply),
    Err(why) => {
      note_failed(ground, log, &step, &why).await;
      Err(failed(why))
    }
  }
}

/// One conversational turn on the provider and model the caller named rather than on the agent's walk:
/// the chat line's run on that provider's road, the session its last answered turn left resumed where the
/// road keeps one. The reply's session id is the thread a Codex turn or the session a registered model's
/// gateway turn continues, which the caller keeps for its next turn.
#[allow(clippy::too_many_arguments)]
pub async fn turn_on(
  ground: &Ground,
  log: &mut RunLog,
  launch: &ChatLaunch<'_>,
  provider: &str,
  model: &str,
  prompt: &str,
  answers: &[i64],
  began: i64,
  shown: Option<OutputWatch<'_>>,
) -> Result<Reply, NoAnswer> {
  let machine = ground.machine();
  let run = chat_run(ground, &machine, launch, prompt, false).await.map_err(refusal)?;

  run.note_persona(ground, log).await;

  let step = step_on(ground, launch.agent, provider, model, &run.reasoning).await?;

  carried_on(ground, log, &run, &step, answers, began, shown).await
}

/// One gate's turn on the provider its conversation names, on the run the gate composed rather than the chat
/// line's: its instructions as the whole system prompt, no tools, no MCP server, and the gate's mapping.
#[allow(clippy::too_many_arguments)]
pub async fn gate_turn_on(
  ground: &Ground,
  log: &mut RunLog,
  run: &ChatRun,
  provider: &str,
  answers: &[i64],
  began: i64,
  gate: &GateAsk,
  shown: Option<OutputWatch<'_>>,
) -> Result<Reply, NoAnswer> {
  let step = Step {
    provider: provider.to_owned(),
    model: run.model.clone(),
    effort: run.effort.clone(),
    thinking: run.thinking,
    answer_tokens: gate.answer_tokens,
  };

  carried_on(ground, log, run, &step, answers, began, shown).await
}

/// One turn carried on the road its step names: the program stood in the agent's pane on the step's model,
/// the session the run resumes where its provider keeps one.
#[allow(clippy::too_many_arguments)]
async fn carried_on(
  ground: &Ground,
  log: &mut RunLog,
  run: &ChatRun,
  step: &Step,
  answers: &[i64],
  began: i64,
  shown: Option<OutputWatch<'_>>,
) -> Result<Reply, NoAnswer> {
  let machine = ground.machine();

  match step.provider.as_str() {
    name if inbox_declared(name) => {
      let outcome = deliver(
        ground,
        &machine,
        run,
        step,
        &Door::Inbox {
          provider: name.to_owned(),
        },
        log,
        answers,
        began,
        shown,
      )
      .await?;
      let mut reply = outcome.map_err(NoAnswer::Failed)?;

      reply.effort = None;

      Ok(reply)
    }
    name if member_declared(name) => {
      let outcome = member_road(ground, &machine, run, step, name, log, answers, began, shown).await?;

      outcome.map_err(NoAnswer::Failed)
    }
    _ => {
      let outcome = registered_road(ground, &machine, run, step, log, answers, began, shown).await?;

      outcome.map_err(NoAnswer::Failed)
    }
  }
}

// The step a conversation's turn goes out on: the registration's own settings on the registry's provider,
// and the agent's own choice of thinking and effort on every other.
async fn step_on(
  ground: &Ground,
  key: &str,
  provider: &str,
  model: &str,
  reasoning: &crate::lines::worker::Reasoning,
) -> Result<Step, NoAnswer> {
  if provider != registry::PROVIDER {
    let held = harness_providers::member::road_members(&ground.machine(), provider)
      .map_err(|why| failed(format!("{provider}: {why}")))?
      .into_iter()
      .find(|one| one.member.name == model)
      .ok_or_else(|| failed(format!("{provider}: {model} is no member of its road")))?;
    let thinking = reasoning.thinking.unwrap_or(held.thinking);
    let effort = if !thinking { "none".to_owned() } else { reasoning.level.clone().unwrap_or_default() };

    return Ok(Step {
      provider: provider.to_owned(),
      model: model.to_owned(),
      effort,
      thinking,
      answer_tokens: None,
    });
  }

  crate::stand_in::registration_step(ground, key, model, reasoning)
    .await
    .map_err(NoAnswer::Failed)
}

/// The mark of a standing fallback cleared once Claude Code answers again, and its end published.
pub async fn fallback_ended(ground: &Ground) {
  let db = &ground.db;
  let previous = match crate::control::fallback::fallback_provider(db).await {
    Ok(None) => return,
    Ok(Some(previous)) => previous,
    Err(why) => return eprintln!("fallback: its mark could not be read: {}", why.error),
  };

  for key in ["fallback_since", "fallback_provider"] {
    if let Err(why) = settings::forget(db, key).await {
      return eprintln!("fallback: its mark was not cleared: {}", why.detail);
    }
  }

  println!("fallback: Claude Code answers again after {previous}");
  ground.events.publish(FallbackEnded {
    owner_id: owner_id(ground),
    previous: Some(previous),
  });
}

// One step's turn carried in the agent's pane on the road the door names: its program stood on the step's
// model and the turn put in, its record fed into the turn's log under the stand-in's name.
#[allow(clippy::too_many_arguments)]
async fn deliver(
  ground: &Ground,
  machine: &Machine,
  run: &ChatRun,
  step: &Step,
  door: &Door,
  log: &mut RunLog,
  answers: &[i64],
  began: i64,
  shown: Option<OutputWatch<'_>>,
) -> Result<Result<Reply, String>, NoAnswer> {
  let composed = match compose_stand_in(ground, machine, run, step, door, answers, began).await {
    Ok(composed) => composed,
    Err(why) => return Ok(Err(why.detail)),
  };
  let resumed = match door {
    Door::Codex { thread } => thread.clone(),
    _ => None,
  };

  match run_turn(
    ground,
    &run.agent,
    run.line,
    &step.provider,
    &composed,
    resumed.as_deref(),
    log,
    false,
    None,
    shown,
  )
  .await
  {
    Ok(reply) => Ok(Ok(reply)),
    Err(NoAnswer::Overtaken) => Err(NoAnswer::Overtaken),
    // A pane that left the words unsubmitted is no road's failure: the next road would put the same words
    // before a program of its own, on a session of its own, for a dialog somebody left open.
    Err(NoAnswer::Unsubmitted(why)) => Err(NoAnswer::Unsubmitted(why)),
    // Nor is a request the road refused for its shape: the next road would be sent the same request
    // (docs/provider-order.md, 「Falling over and coming back」).
    Err(NoAnswer::Errored { error, kind }) if request_refused(&error, kind.as_deref()) => {
      Err(NoAnswer::Errored { error, kind })
    }
    Err(NoAnswer::Cut(why) | NoAnswer::Failed(why) | NoAnswer::Errored { error: why, .. }) => Ok(Err(why)),
  }
}

// The agent's kept Codex thread for the step's model and effort, where one is kept.
#[allow(dead_code)]
async fn kept_thread(ground: &Ground, agent: &str, step: &Step) -> Result<Option<String>, Refusal> {
  Ok(
    one(
      &ground.db,
      r#"SELECT "session_id" FROM "fallback_sessions" WHERE "scope" = $1 AND "mode" = $2 AND "model" = $3 AND "effort" = $4"#,
      vec![agent.into(), CODEX.into(), step.model.clone().into(), step.effort.clone().into()],
    )
    .await?
    .and_then(|row| row.try_get::<String>("", "session_id").ok()),
  )
}

async fn forget_thread(ground: &Ground, agent: &str, step: &Step) -> Result<(), Refusal> {
  written(
    &ground.db,
    r#"DELETE FROM "fallback_sessions" WHERE "scope" = $1 AND "mode" = $2 AND "model" = $3 AND "effort" = $4"#,
    vec![agent.into(), CODEX.into(), step.model.clone().into(), step.effort.clone().into()],
  )
  .await
  .map(drop)
  .map_err(Refusal::from)
}

// One turn on the Codex road: on the agent's kept thread, or one opened afresh where Codex no longer holds
// it; the thread that answered kept with the run it is resumed and warmed by.
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
async fn codex_road(
  ground: &Ground,
  machine: &Machine,
  run: &ChatRun,
  step: &Step,
  log: &mut RunLog,
  answers: &[i64],
  began: i64,
  shown: Option<OutputWatch<'_>>,
) -> Result<Result<Reply, String>, NoAnswer> {
  let kept = kept_thread(ground, &run.agent, step).await.map_err(refusal)?;
  let on = |thread: Option<String>| Door::Codex { thread };
  let mut outcome =
    deliver(ground, machine, run, step, &on(kept.clone()), log, answers, began, shown).await?;

  if let (Err(why), Some(_)) = (&outcome, &kept)
    && lost_thread(why)
  {
    forget_thread(ground, &run.agent, step).await.map_err(refusal)?;
    outcome = deliver(ground, machine, run, step, &on(None), log, answers, began, shown).await?;
  }

  let Ok(reply) = outcome else { return Ok(outcome) };
  let stored = run.stand_in(&step.model, &step.effort, "input", true).stored();

  written(
    &ground.db,
    r#"INSERT INTO "fallback_sessions" ("scope", "key", "mode", "model", "effort", "session_id", "last_reply_at", "run_json")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
       ON CONFLICT ("scope", "mode", "model", "effort") DO UPDATE SET "key" = excluded."key", "session_id" = excluded."session_id",
         "last_reply_at" = excluded."last_reply_at", "run_json" = excluded."run_json""#,
    vec![
      run.agent.clone().into(),
      run.agent.clone().into(),
      CODEX.into(),
      step.model.clone().into(),
      step.effort.clone().into(),
      reply.session_id.clone().into(),
      (now_ms() / 1000).into(),
      stored.into(),
    ],
  )
  .await
  .map_err(|why| refusal(why.into()))?;

  Ok(Ok(reply))
}

// One turn on a provider's inbox road, through the pane program on the dsh session it keeps.
#[allow(clippy::too_many_arguments)]
async fn inbox_road(
  ground: &Ground,
  machine: &Machine,
  run: &ChatRun,
  step: &Step,
  log: &mut RunLog,
  answers: &[i64],
  began: i64,
  shown: Option<OutputWatch<'_>>,
) -> Result<Result<Reply, String>, NoAnswer> {
  let outcome = deliver(
    ground,
    machine,
    run,
    step,
    &Door::Inbox {
      provider: step.provider.clone(),
    },
    log,
    answers,
    began,
    shown,
  )
  .await?;
  let mut reply = match outcome {
    Ok(reply) => reply,
    Err(why) => return Ok(Err(why)),
  };

  reply.effort = None;

  Ok(Ok(reply))
}

/// The door a turn on a registration goes out through: Claude Code on the gateway under the registration's
/// own home, its address and key the gateway's and its model the registration's name, placed on the node a
/// new session on it is given.
pub async fn registered_door(
  ground: &Ground,
  machine: &Machine,
  registration: &str,
) -> Result<Door, Refusal> {
  let placement = place_registered(ground, registration).await?;
  let held = crate::lines::pieces::registration_named(ground, registration).await?;

  Ok(Door::Gateway {
    held: Box::new(GatewayModel::registration(machine, &held)),
    placement,
  })
}

/// The door a turn on a registration naming a card service goes out through: as `registered_door`, placed
/// on the node of the card its demand was granted.
pub async fn carded_door(
  ground: &Ground,
  machine: &Machine,
  registration: &str,
  node: &str,
) -> Result<Door, Refusal> {
  let held = crate::lines::pieces::registration_named(ground, registration).await?;

  Ok(Door::Gateway {
    held: Box::new(GatewayModel::registration(machine, &held)),
    placement: Some(node.to_owned()),
  })
}

// One turn on a registered model's road: Claude Code on the gateway in the agent's pane, the registration
// first held to the gateway's list, on a session of its own placed where the registration places it.
#[allow(clippy::too_many_arguments)]
async fn registered_road(
  ground: &Ground,
  machine: &Machine,
  run: &ChatRun,
  step: &Step,
  log: &mut RunLog,
  answers: &[i64],
  began: i64,
  shown: Option<OutputWatch<'_>>,
) -> Result<Result<Reply, String>, NoAnswer> {
  let Some(service) =
    registry::service_of(&ground.db, &step.model).await.map_err(|why| failed(why.error))?
  else {
    return Ok(Err(format!("{}: {}", REGISTERED.presence.not_registered, step.model)));
  };

  // A registration whose engine the arbiter stands is asked a card first, which the turn holds until its
  // end; its turn goes out on the card granted (docs/own-llm.md, 「An engine the arbiter stands」).
  let card = match registry::registration(&ground.db, &step.model).await.map_err(|why| failed(why.error))? {
    Some(held) => {
      match cards::hold_logged(
        ground,
        &held,
        &run.agent,
        run.resume.as_deref(),
        run.cards.clone(),
        log,
        shown,
      )
      .await
      {
        Ok(card) => card,
        Err(why) => return Ok(Err(why.detail)),
      }
    }
    None => None,
  };
  let watching = card.as_ref().map(|card| cards::watch_turn(ground, card, log.run()));
  let outcome = async {
    if let Err(why) = listed(ground, &service, &step.model).await {
      return Ok(Err(why));
    }

    let door = match &card {
      Some(card) => carded_door(ground, machine, &step.model, &card.node).await.map_err(refusal)?,
      None => registered_door(ground, machine, &step.model).await.map_err(refusal)?,
    };

    deliver(ground, machine, run, step, &door, log, answers, began, shown).await
  }
  .await;

  if let Some(watching) = watching {
    watching.finish(ground).await;
  }

  if let Some(card) = &card
    && let Some(said) = cards::give_back(ground, card).await
  {
    log.write(&ground.db, Line::note(said)).await;
  }

  let outcome = outcome?;
  let Ok(mut reply) = outcome else { return Ok(outcome) };

  reply.effort = step.thinking.then(|| "thinking".to_owned());

  Ok(Ok(reply))
}

// A turn on a member of a provider's own road: Claude Code on the gateway, its home, window and wait the
// member's, and its session the one the run resumes where one stands.
#[allow(clippy::too_many_arguments)]
async fn member_road(
  ground: &Ground,
  machine: &Machine,
  run: &ChatRun,
  step: &Step,
  road: &str,
  log: &mut RunLog,
  answers: &[i64],
  began: i64,
  shown: Option<OutputWatch<'_>>,
) -> Result<Result<Reply, String>, NoAnswer> {
  let Some(provider) = harness_providers::provider(road) else {
    return Ok(Err(format!("{road} is no provider of this build")));
  };
  let held =
    member_model(machine, provider.name(), &step.model).map_err(|refused| failed(refused.detail))?;
  let door = Door::Gateway {
    held: Box::new(held),
    placement: None,
  };
  let outcome = deliver(ground, machine, run, step, &door, log, answers, began, shown).await?;
  let Ok(mut reply) = outcome else { return Ok(outcome) };

  reply.effort = (!step.effort.is_empty()).then(|| step.effort.clone());

  Ok(Ok(reply))
}
