//! The chat's turn across rooms on a stand-in (docs/cache-fallback.md, 「While a fallback stands, the room is
//! not silent」): where the agent's own key walks onto a provider before Claude Code, or every login of
//! Claude Code is closed and a stand-in answers, the turn goes out there — Codex on the agent's kept thread,
//! OpenRouter through a tap of its own, a registered model's Claude Code on the gateway — as that provider's
//! program standing in the agent's pane. A stand-in answering where the key walks is published as the
//! fallback it is; Claude Code answering again while a fallback stands clears it and publishes its end.

use harness_providers::Machine;
use policy::providers::openrouter::OPENROUTER;
use policy::providers::registered::REGISTERED;
use serde::Serialize;

use crate::companion::run::{NoAnswer, Reply, run_turn};
use crate::control::harness::provider_enabled;
use crate::control::registry;
use crate::events::Event;
use crate::job_run::openrouter::spent_of;
use crate::job_run::openrouter_tap;
use crate::lines::chat::{ChatLaunch, ChatRun, Door, chat_run, compose_stand_in};
use crate::lines::codex::lost_thread;
use crate::lines::standing::place_registered;
use crate::places::Ground;
use crate::queue::settings;
use crate::run_log::RunLog;
use crate::run_log::lines::Line;
use crate::session_output::OutputWatch;
use crate::stand_in::{CLAUDE, Step, Walk, announce, listed, step_of, walk_of};
use crate::{Refusal, now_ms};
use store::{one, run as written};

const CODEX: &str = "codex";
const OPENROUTER_NAME: &str = "openrouter";

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

/// The turn across rooms on the first stand-in of the agent's walk that answers: none where Claude Code is
/// the road it stands on now or no stand-in answered and Claude Code may still be asked, the stand-in's reply
/// where one did, and why where the walk had to answer and none did.
#[allow(clippy::too_many_arguments)]
pub async fn stand_in_turn(
  ground: &Ground,
  log: &mut RunLog,
  launch: &ChatLaunch<'_>,
  prompt: &str,
  answers: &[i64],
  began: i64,
  shown: Option<OutputWatch<'_>>,
) -> Result<Option<Reply>, NoAnswer> {
  let machine = ground.machine();
  let agent = launch.agent;
  let Some(Walk {
    steps,
    required,
    chosen,
  }) = walk_of(ground, &machine, agent).await
  else {
    return Ok(None);
  };
  let db = &ground.db;
  let run = chat_run(ground, &machine, launch, prompt, false).await.map_err(refusal)?;
  let mut last: Option<String> = None;

  for provider in &steps {
    if !provider_enabled(db, provider).await.map_err(|why| failed(why.error))? {
      continue;
    }

    let step = match step_of(ground, &machine, agent, provider, &run.reasoning).await {
      Ok(step) => step,
      Err(why) => {
        last = Some(why);
        continue;
      }
    };
    let outcome = match provider.as_str() {
      CODEX => codex_road(ground, &machine, &run, &step, log, answers, began, shown).await?,
      OPENROUTER_NAME => openrouter_road(ground, &machine, &run, &step, log, answers, began, shown).await?,
      _ => registered_road(ground, &machine, &run, &step, log, answers, began, shown).await?,
    };

    match outcome {
      Err(why) => {
        note_failed(ground, log, &step, &why).await;
        last = Some(why);
      }
      Ok(reply) => {
        if provider != &chosen
          && let Err(why) = announce(ground, &ground.events, owner_id(ground), provider, &step.model).await
        {
          eprintln!("fallback {agent}: its answering on {provider} was not marked: {}", why.detail);
        }

        return Ok(Some(reply));
      }
    }
  }

  if required || !provider_enabled(db, CLAUDE).await.map_err(|why| failed(why.error))? {
    return Err(failed(last.unwrap_or_else(|| format!("{agent}: no stand-in of its walk answered"))));
  }

  Ok(None)
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
  let step = step_on(ground, launch.agent, provider, model, &run.reasoning).await?;

  match provider {
    CODEX => {
      let kept = launch.resume.clone();
      let on = |thread: Option<String>| Door::Codex { thread };
      let mut outcome =
        deliver(ground, &machine, &run, &step, &on(kept.clone()), log, answers, began, shown).await?;

      if let (Err(why), Some(_)) = (&outcome, &kept)
        && lost_thread(why)
      {
        outcome = deliver(ground, &machine, &run, &step, &on(None), log, answers, began, shown).await?;
      }

      outcome.map_err(NoAnswer::Failed)
    }
    OPENROUTER_NAME => {
      let api = OPENROUTER.gateway.api_on(ground.vars()).to_owned();
      let tap = match openrouter_tap::open(None, &api).await {
        Ok(tap) => tap,
        Err(why) => return Err(NoAnswer::Failed(format!("the OpenRouter tap: {why}"))),
      };
      let door = Door::Loop {
        tap: tap.url.clone(),
      };
      let outcome = deliver(ground, &machine, &run, &step, &door, log, answers, began, shown).await;
      let seen = tap.observed();

      tap.close();

      let mut reply = match outcome? {
        Ok(reply) => reply,
        Err(why) => return Err(NoAnswer::Failed(why)),
      };

      if let Some(spent) = spent_of(&reply.mode, &step.model, &seen) {
        reply.spend = vec![spent];
      }

      reply.effort = None;

      Ok(reply)
    }
    _ => {
      let Some(service) = registry::service_of(&ground.db, &step.model)
        .await
        .map_err(|why| NoAnswer::Failed(why.error))?
      else {
        return Err(NoAnswer::Failed(format!("{}: {}", REGISTERED.presence.not_registered, step.model)));
      };

      if let Err(why) = listed(ground, &service, &step.model).await {
        return Err(NoAnswer::Failed(why));
      }

      let placement = place_registered(ground, &step.model).await.map_err(refusal)?;
      let outcome =
        deliver(ground, &machine, &run, &step, &Door::Gateway { placement }, log, answers, began, shown)
          .await?;
      let mut reply = outcome.map_err(NoAnswer::Failed)?;

      reply.effort = step.thinking.then(|| "thinking".to_owned());

      Ok(reply)
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
    return Ok(Step {
      provider: provider.to_owned(),
      model: model.to_owned(),
      effort: reasoning.level.clone().unwrap_or_default(),
      thinking: reasoning.thinking.unwrap_or(true),
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
    Err(NoAnswer::Cut(why) | NoAnswer::Failed(why) | NoAnswer::Errored { error: why, .. }) => Ok(Err(why)),
  }
}

// The agent's kept Codex thread for the step's model and effort, where one is kept.
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

// One turn on the OpenRouter road through a tap of its own, which reads what the turn spent off the stream.
#[allow(clippy::too_many_arguments)]
async fn openrouter_road(
  ground: &Ground,
  machine: &Machine,
  run: &ChatRun,
  step: &Step,
  log: &mut RunLog,
  answers: &[i64],
  began: i64,
  shown: Option<OutputWatch<'_>>,
) -> Result<Result<Reply, String>, NoAnswer> {
  let api = OPENROUTER.gateway.api_on(ground.vars()).to_owned();
  let tap = match openrouter_tap::open(None, &api).await {
    Ok(tap) => tap,
    Err(why) => return Ok(Err(format!("the OpenRouter tap: {why}"))),
  };
  let door = Door::Loop {
    tap: tap.url.clone(),
  };
  let outcome = deliver(ground, machine, run, step, &door, log, answers, began, shown).await;
  let seen = tap.observed();

  tap.close();

  let mut reply = match outcome? {
    Ok(reply) => reply,
    Err(why) => return Ok(Err(why)),
  };

  if let Some(spent) = spent_of(&reply.mode, &step.model, &seen) {
    reply.spend = vec![spent];
  }

  reply.effort = None;

  Ok(Ok(reply))
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

  if let Err(why) = listed(ground, &service, &step.model).await {
    return Ok(Err(why));
  }

  let placement = place_registered(ground, &step.model).await.map_err(refusal)?;
  let outcome =
    deliver(ground, machine, run, step, &Door::Gateway { placement }, log, answers, began, shown).await?;
  let Ok(mut reply) = outcome else { return Ok(outcome) };

  reply.effort = step.thinking.then(|| "thinking".to_owned());

  Ok(Ok(reply))
}
