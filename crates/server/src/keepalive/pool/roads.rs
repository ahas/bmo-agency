//! A reserved place's ping on its provider's own program, carried the way the work it warms for is carried:
//! on Claude Code the worker line as a turn of the place's own program, on the logins its key is offered in
//! turn; on Codex and OpenRouter the place's run once more with the keepalive line in the host, keeping
//! nothing, OpenRouter's to the upstream that took the place's last turn and nowhere else. A ping takes the
//! place's own lock without waiting, keeps no session, and publishes what it spent under the place's own
//! name.

use std::time::Instant;

use agent_host::envelope::Cause;
use policy::fill_marks;
use policy::keepalive::KEEPALIVE;
use policy::pool_warm::POOL_WARM;
use policy::providers::openrouter::OPENROUTER;
use serde_json::Value;

use super::{Pinged, WarmPlace};
use crate::Refusal;
use crate::control::claude_accounts::Account;
use crate::control::harness::provider_enabled;
use crate::door::execution::{execution_root, own_unit};
use crate::door::record::Reply;
use crate::door::turn::{Ending, Quiet, put};
use crate::host_run::{self, Command};
use crate::job_run::claude::failure::Failure;
use crate::job_run::claude::walk::{Attempt, Walking, on_accounts};
use crate::job_run::openrouter::observed;
use crate::job_run::openrouter_tap::{self, Pin};
use crate::lines::stand_in::{StandInRun, Taking};
use crate::lines::{Trees, codex, dsh, fault, openrouter};
use crate::places::Ground;
use crate::places::locks::try_hold;
use crate::worker_session::records::{note_spend, note_usage};
use crate::worker_session::{Composed, compose_at};

const CLAUDE_CODE: &str = "claude-code";

/// One place pinged on its provider's program.
pub async fn ping(ground: &Ground, place: &WarmPlace) -> Result<Pinged, Refusal> {
  match place.provider.as_str() {
    CLAUDE_CODE => claude(ground, place).await,
    codex::PROVIDER => codex_ping(ground, place).await,
    openrouter::PROVIDER => openrouter_ping(ground, place).await,
    // A registered model publishes no lifetime, so its places are never due.
    _ => Ok(Pinged::Failed(POOL_WARM.pings.no_prefix.clone())),
  }
}

async fn enabled(ground: &Ground, provider: &str) -> Result<bool, Refusal> {
  provider_enabled(&ground.db, provider).await.map_err(|refused| fault(refused.error))
}

fn off(provider: &str) -> Pinged {
  Pinged::Failed(fill_marks(&POOL_WARM.pings.off, &[("mode", provider)]))
}

// The run's tier as an execution row keeps it.
fn tier_of(tier: Option<&String>) -> Option<Value> {
  tier.map(|tier| Value::String(tier.clone()))
}

async fn claude(ground: &Ground, place: &WarmPlace) -> Result<Pinged, Refusal> {
  let Some(_held) = try_hold(&place.lock) else { return Ok(Pinged::Busy) };

  if !enabled(ground, CLAUDE_CODE).await? {
    return Ok(off(CLAUDE_CODE));
  }

  // A place's ping warms one login's prefix, so it goes out on the login its line's key is offered first
  // and is carried to the next where that login is what failed.
  let walking = Walking { ground };

  match on_accounts(&walking, crate::lines::worker::LINE, None, &mut ClaudePing { ground, place }).await {
    Ok(pinged) => Ok(pinged),
    Err(failed) if failed.account_failure().is_some() => {
      Ok(Pinged::Failed(POOL_WARM.pings.no_account.clone()))
    }
    Err(failed) => Err(fault(failed.message())),
  }
}

// One ping on one login.
struct ClaudePing<'a> {
  ground: &'a Ground,
  place: &'a WarmPlace,
}

impl Attempt<Pinged> for ClaudePing<'_> {
  async fn attempt(&mut self, account: Option<Account>) -> Result<Pinged, Failure> {
    let (ground, place) = (self.ground, self.place);
    let pinged = in_place(ground, place, account.as_ref()).await?;

    if let Pinged::Answered(reply) = &pinged {
      note_usage(&ground.db, &ground.windows, &ground.events, &place.usage_name, reply)
        .await
        .map_err(|why| Failure::error(why.detail))?;
    }

    Ok(pinged)
  }

  // An endpoint account is never pinged: its endpoint keeps its own cache, and a ping would only be a
  // paid request (docs/keepalive-schedule.md).
  fn takes(&self, account: &Account) -> bool {
    account.credential.warmed()
  }
}

// A standing checkout's ping: a turn of the place's own program, where its branches run.
async fn in_place(
  ground: &Ground,
  place: &WarmPlace,
  account: Option<&Account>,
) -> Result<Pinged, Failure> {
  let run = &place.run;
  let Some(home) = run.home.as_deref().cloned() else {
    return Ok(Pinged::Failed(POOL_WARM.pings.no_pane.clone()));
  };
  let composed = Composed {
    name: place.name.clone(),
    title: place.title.clone(),
    prompt: KEEPALIVE.prompt.clone(),
    bound: Some(place.timeout),
  };
  let turn = compose_at(ground, &run.agent, home, &run.line, account, None, composed)
    .await
    .map_err(|why| Failure::error(why.detail))?;

  Ok(match put(ground, &turn, &mut Quiet).await.map_err(Failure::error)? {
    Ending::Answered { reply, .. } => Pinged::Answered(reply),
    other => Pinged::Failed(other.word().to_owned()),
  })
}

// How a stand-in's program takes a place's run, and reads what it printed.
trait StandIn {
  fn mode(&self) -> &'static str;

  fn taking(&self) -> Taking;

  fn command(&self, ground: &Ground, run: &StandInRun) -> Result<Command, Refusal>;

  fn answer(&self, ground: &Ground, run: &StandInRun, ran: &host_run::Ran) -> Reply;

  fn failure(&self, ran: &host_run::Ran) -> String;
}

struct CodexPing;

impl StandIn for CodexPing {
  fn mode(&self) -> &'static str {
    codex::PROVIDER
  }

  fn taking(&self) -> Taking {
    codex::TAKING
  }

  fn command(&self, ground: &Ground, run: &StandInRun) -> Result<Command, Refusal> {
    codex::exec_command(run, None, &KEEPALIVE.prompt, ground.env())
  }

  fn answer(&self, ground: &Ground, run: &StandInRun, ran: &host_run::Ran) -> Reply {
    let sessions = std::path::PathBuf::from(ground.machine().var("HOME").unwrap_or_default())
      .join(".codex")
      .join("sessions");

    codex::exec_answer(&ran.stdout, ran.exit_code, run, None, &sessions)
  }

  fn failure(&self, ran: &host_run::Ran) -> String {
    codex::exec_failure(&ran.stdout, &ran.stderr, ran.exit_code)
  }
}

struct OpenRouterPing<'k> {
  key: &'k str,
  tap: &'k str,
}

impl StandIn for OpenRouterPing<'_> {
  fn mode(&self) -> &'static str {
    openrouter::PROVIDER
  }

  fn taking(&self) -> Taking {
    openrouter::TAKING
  }

  fn command(&self, ground: &Ground, run: &StandInRun) -> Result<Command, Refusal> {
    let mut env = ground.env().clone();

    env.insert(OPENROUTER.presence.key_env.clone(), self.key.to_owned());
    env.insert(openrouter::LOCAL_URL.to_owned(), self.tap.to_owned());
    dsh::command(run, &KEEPALIVE.prompt, &env, &Trees::of(ground).agents)
  }

  fn answer(&self, _ground: &Ground, run: &StandInRun, ran: &host_run::Ran) -> Reply {
    dsh::answer(&ran.stdout, ran.exit_code, run)
  }

  fn failure(&self, ran: &host_run::Ran) -> String {
    dsh::failure(&ran.stderr, ran.exit_code)
  }
}

// The place's run once more on a stand-in's program in the host, with the keepalive line, keeping nothing,
// under the place's own lock asked for without waiting.
async fn stand_in<S: StandIn>(ground: &Ground, place: &WarmPlace, road: &S) -> Result<Pinged, Refusal> {
  let mode = road.mode();

  if !enabled(ground, mode).await? {
    return Ok(off(mode));
  }

  let Some(_held) = try_hold(&place.lock) else { return Ok(Pinged::Busy) };
  let mut run = StandInRun::of(&place.run.line, None, None, road.taking());

  run.keep_session = false;
  run.ephemeral = true;

  let command = match road.command(ground, &run) {
    Ok(command) => command,
    Err(why) => return Ok(Pinged::Failed(why.detail)),
  };
  let own = own_unit(ground.env());
  let started = Instant::now();
  let ran = host_run::run(
    ground,
    host_run::Asked {
      agent: &place.work_key,
      line: &run.line,
      provider: mode,
      command,
      root: Some(execution_root(own.as_deref().unwrap_or_default())),
      parent: own,
      work_key: Some(place.work_key.clone()),
      model: Some(run.model.clone()),
      effort: (!run.effort.is_empty()).then(|| run.effort.clone()),
      tier: tier_of(run.tier.as_ref()),
      session: None,
      timeout: Some(place.timeout),
    },
  )
  .await;
  let ran = match ran {
    Ok(ran) => ran,
    Err(host_run::Failure::Transport(why)) => return Err(fault(why)),
    Err(failure) => return Ok(Pinged::Failed(failure.said())),
  };

  // A run cut at its wait is that, whatever its program last said on the way out.
  if ran.cause != Cause::Exited && started.elapsed() >= place.timeout {
    let minutes = (place.timeout.as_secs_f64() / 60.0).round().to_string();

    return Ok(Pinged::Failed(fill_marks(&POOL_WARM.pings.limit, &[("minutes", &minutes)])));
  }

  let reply = road.answer(ground, &run, &ran);

  if reply.text.is_empty() {
    return Ok(Pinged::Failed(road.failure(&ran)));
  }

  Ok(Pinged::Answered(Box::new(reply)))
}

// What an answered stand-in ping spent, published under the place's own name.
fn spent(ground: &Ground, place: &WarmPlace, reply: &Reply) {
  note_spend(
    &ground.events,
    &place.usage_name,
    &reply.spend,
    reply.account.as_deref().unwrap_or_default(),
  );
}

async fn codex_ping(ground: &Ground, place: &WarmPlace) -> Result<Pinged, Refusal> {
  let pinged = stand_in(ground, place, &CodexPing).await?;

  if let Pinged::Answered(reply) = &pinged {
    spent(ground, place, reply);
  }

  Ok(pinged)
}

// A ping another upstream answers warms a cache nobody will read, so it goes to the upstream that took the
// place's last turn and nowhere else, and counts only where that upstream answered it.
async fn openrouter_ping(ground: &Ground, place: &WarmPlace) -> Result<Pinged, Refusal> {
  let variable = &OPENROUTER.presence.key_env;
  let Some(key) = ground.vars().get(variable).cloned() else {
    return Ok(Pinged::Failed(fill_marks(&POOL_WARM.pings.no_key, &[("variable", variable)])));
  };
  let Some(upstream) = place.upstream.clone() else {
    return Ok(Pinged::Failed(POOL_WARM.pings.no_upstream.clone()));
  };
  let api = OPENROUTER.gateway.api_on(ground.vars()).to_owned();
  let tap = openrouter_tap::open(
    Some(Pin {
      upstream: upstream.clone(),
      allow_fallbacks: false,
    }),
    &api,
  )
  .await
  .map_err(|why| fault(format!("the OpenRouter tap: {why}")))?;
  let pinged = stand_in(
    ground,
    place,
    &OpenRouterPing {
      key: &key,
      tap: &tap.url,
    },
  )
  .await;
  let seen = tap.observed();

  tap.close();

  Ok(match pinged? {
    Pinged::Answered(reply) if seen.upstream.as_deref() == Some(upstream.as_str()) => {
      let reply = observed(*reply, &place.model, &seen);

      spent(ground, place, &reply);
      Pinged::Answered(Box::new(reply))
    }
    Pinged::Answered(_) => {
      Pinged::Failed(fill_marks(&POOL_WARM.pings.other_upstream, &[("upstream", &upstream)]))
    }
    other => other,
  })
}
