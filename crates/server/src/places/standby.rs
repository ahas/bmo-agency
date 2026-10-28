//! A worker standing with nothing to do (docs/agent-control.md, 「A worker standing with nothing to
//! do」): every worker's place holds the program its next job would start, written with nothing put in,
//! so its pane is the program's own screen where a person can type the worker's first request; and a
//! pinned worker's place holding another provider's program has its road's program stood there at once: its
//! pin's while the pin can answer, the next road of its walk while it cannot (docs/cache-fallback.md, 「One
//! road resolution」).
//!
//! The pass runs before every head is stood — as this process attaches to herdr and on the heads'
//! clock — and over the pinned workers as a pin changes and after a job's end, then waiting a while for
//! the checkout's lock the ended runner still holds. A job or a word at work holds a lock, and its place
//! is left to it; a person's turn at work leaves the place to a later pass. A process on a copy of the
//! data root stands none, so a copy never holds a program per worker.

use std::path::Path;
use std::time::Duration;

use harness_providers::{Machine, provider, standing_provider};
use policy::fill_marks;
use policy::place_words::PLACE_WORDS;
use policy::providers::Turns;
use serde_json::{Value, json};
use store::{all, one, run};

use super::locks::{Held, hold_within, lock_path, pane_lock_path, try_hold};
use super::trust::trust_worker_folder;
use super::{AgentRow, Ground};
use crate::control::fallback::fallback_provider;
use crate::control::harness::provider_enabled;
use crate::control::registry;
use crate::lines::place::{Place, Program, token_of};
use crate::lines::standing::{
  Standing, claim_standby, claude_program, inbox_program, member_program, registered_program,
};
use crate::lines::token;
use crate::queue::routes::{JOB_WORK_KEY, worker_pin_held};
use crate::road::{self, Ask};
use crate::{Refusal, now_ms};

/// How long the pass after a job's end waits for the checkout's lock the ended runner still holds.
pub const AFTER_JOB_LOCK_WAIT: Duration = Duration::from_secs(60);

const CLAUDE_CODE: &str = "claude-code";

// The upkeep of a worker's pane takes the checkout's job lock and then the worker's pane lock, both
// without waiting, so it never stands in the way of a job or a word, and is given up that round where
// either is held; a caller that knows a lock is about to be let go says how long to wait for it.
async fn worker_pane(
  ground: &Ground,
  worker: &str,
  cwd: &str,
  wait: Option<Duration>,
) -> Option<(Held, Held)> {
  let take = async |path: std::path::PathBuf| match wait {
    None => try_hold(&path),
    Some(wait) => hold_within(path, wait).await,
  };
  let job = take(lock_path(&ground.roots.data, cwd)).await?;
  let pane = take(pane_lock_path(&ground.roots.data, worker)).await?;

  Some((job, pane))
}

// Whether the pane program's record holds a turn begun since the program started and not ended.
fn record_turn_open(place: &Path, since: u64) -> bool {
  let records: Vec<Value> = std::fs::read_to_string(place.join("loop.jsonl"))
    .unwrap_or_default()
    .lines()
    .filter_map(|line| serde_json::from_str(line).ok())
    .collect();
  let Some(start) = records
    .iter()
    .rev()
    .find(|record| record.get("kind").and_then(Value::as_str) == Some("start"))
  else {
    return false;
  };

  if start.get("at").and_then(Value::as_u64).unwrap_or(0) < since {
    return false;
  }

  !records.iter().any(|record| {
    record.get("id") == start.get("id")
      && matches!(record.get("kind").and_then(Value::as_str), Some("end" | "failed" | "interrupted"))
  })
}

// What herdr says the agent in a pane is doing, its list call sent again once where it timed out; none
// where herdr did not answer or no longer lists the pane.
async fn pane_status(ground: &Ground, pane: &str) -> Option<String> {
  let herdr = ground.herdr.as_ref()?;
  let mut answer = herdr.call("agent.list", json!({}), None).await;

  if answer.as_ref().is_err_and(|fault| fault.code == "timeout") {
    answer = herdr.call("agent.list", json!({}), None).await;
  }

  let Ok(answer) = answer else { return Some("silent".to_owned()) };
  let found = answer.get("agents").and_then(Value::as_array).and_then(|agents| {
    agents.iter().find(|agent| agent.get("pane_id").and_then(Value::as_str) == Some(pane))
  });

  Some(
    found
      .and_then(|agent| agent.get("agent_status"))
      .and_then(Value::as_str)
      .unwrap_or("unknown")
      .to_owned(),
  )
}

// Whether the program standing in a place is at work on a turn a person put in: herdr reads its pane
// working or asking, or does not answer about it, or the pane program's record holds a turn begun since
// the program started and not ended.
async fn pane_at_work(ground: &Ground, place: &Place) -> bool {
  let Some(running) = place.program_alive() else { return false };

  if record_turn_open(&place.path, running.started_at) {
    return true;
  }

  let Some(pane) = running.pane_id.as_deref() else { return false };

  matches!(pane_status(ground, pane).await.as_deref(), Some("working" | "blocked" | "silent"))
}

// One note in the worker's own log.
async fn note(ground: &Ground, worker: &str, text: &str) -> Result<(), Refusal> {
  run(
    &ground.db,
    r#"INSERT INTO "agent_log" ("agent", "run", "chat_id", "at", "kind", "name", "parent", "ref", "truncated", "text",
         "job_id", "sequence", "source", "source_event", "source_block", "source_id", "state", "upstream_truncated",
         "detail_complete", "detail_length", "metadata_length", "detail", "metadata")
       VALUES ($1, $2, NULL, $3, 'note', NULL, NULL, NULL, 0, $4, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 0, 1, $5, 0, NULL, NULL)"#,
    vec![
      worker.into(),
      format!("bmo-standby-{worker}-{}", now_ms()).into(),
      now_ms().into(),
      text.into(),
      (text.chars().count() as i64).into(),
    ],
  )
  .await?;

  Ok(())
}

// The program a pin stands with nothing put in, where its provider's surface stands one: the
// registry's Claude Code on the gateway, or the provider's own pane program.
async fn pinned_program(
  ground: &Ground,
  machine: &Machine,
  worker: &str,
  cwd: &str,
  pin: &str,
  model: Option<&str>,
) -> Result<Option<Standing>, Refusal> {
  match pin {
    "registered" => Ok(Some(registered_program(ground, machine, worker, cwd, model).await?)),
    road if crate::chat_stand_in::member_declared(road) => {
      let Some(provider) = provider(road) else {
        return Ok(None);
      };

      Ok(Some(member_program(ground, machine, worker, cwd, provider.name(), model).await?))
    }
    _ => {
      let resident = provider(pin).and_then(|provider| provider.declared().resident.as_ref());
      let program = resident.and_then(|resident| resident.program.clone());
      let dsh = resident.and_then(|resident| resident.dsh.clone());

      match (program, dsh) {
        (Some(program), Some(dsh)) => {
          Ok(Some(inbox_program(ground, machine, worker, cwd, pin, &program, &dsh, model).await?))
        }
        _ => Ok(None),
      }
    }
  }
}

// A worker's standing program handed its session token, where its line takes one.
async fn tokened(
  ground: &Ground,
  worker: &str,
  place: &Place,
  cwd: &str,
  mut program: Program,
) -> Result<(Program, Option<String>), Refusal> {
  let checkout = token::record_checkout(ground, &place.path, cwd);
  let issued = token::issue(ground, crate::lines::worker::LINE, worker, &place.path, &checkout).await?;

  if let Some(token) = &issued {
    for command in std::iter::once(&mut program.command).chain(program.restart.as_mut()) {
      command.env.insert(token::TOKEN_VARIABLE.to_owned(), token.clone());
    }
  }

  Ok((program, issued))
}

/// A worker whose place holds no program stands the program its next job would start, with nothing put
/// in; a session a person opened in it is taken as the worker's kept session first, and a place that
/// already holds a program is checked against the worker's pin instead.
pub async fn stand_idle(ground: &Ground, row: &AgentRow) -> Result<(), Refusal> {
  if row.kind != "worker" || ground.copy().is_some() {
    return Ok(());
  }

  let cwd = ground.checkout_of(row);

  trust_worker_folder(ground, &cwd, &row.name).await;

  let place = Place::of(&ground.roots.data, &row.name);

  claim_standby(ground, &row.name, &place).await?;

  if place.program().is_some() {
    return restand_pinned(ground, row, None).await;
  }

  if !Path::new(&cwd).exists() {
    return Ok(());
  }

  let machine = ground.machine();
  let pin = pinned_road(ground, &row.name).await?;

  if let Some((pinned, model)) = pin.as_ref().filter(|(pinned, _)| pinned != CLAUDE_CODE) {
    let Some(_locks) = worker_pane(ground, &row.name, &cwd, None).await else {
      return Ok(());
    };
    let Some(standing) =
      pinned_program(ground, &machine, &row.name, &cwd, pinned, model.as_deref()).await?
    else {
      return Ok(());
    };
    let (program, issued) = tokened(ground, &row.name, &place, &cwd, standing.program).await?;

    if place.offer(&program)? {
      println!(
        "standby {}: its {pinned} program stands with nothing put in, on {}",
        row.name, standing.on
      );
    } else if let Some(issued) = issued {
      token::revoke(ground, &issued).await;
    }

    return Ok(());
  }

  let road = match &pin {
    Some((pinned, _)) => pinned.clone(),
    None => job_road(ground).await?,
  };

  if road != CLAUDE_CODE
    || !provider_enabled(&ground.db, CLAUDE_CODE)
      .await
      .map_err(|refused| crate::lines::fault(refused.error))?
  {
    return Ok(());
  }

  let Some(_locks) = worker_pane(ground, &row.name, &cwd, None).await else {
    return Ok(());
  };
  let standing = claude_program(ground, &machine, &row.name, &cwd).await?;
  let (program, issued) = tokened(ground, &row.name, &place, &cwd, standing.program).await?;

  if !place.offer(&program)? {
    if let Some(issued) = issued {
      token::revoke(ground, &issued).await;
    }

    return Ok(());
  }

  if let Some(standby) = standing.standby {
    standby.write(&place.path)?;
  }

  println!("standby {}: its program stands with nothing put in, on {}", row.name, standing.on);

  Ok(())
}

// The provider the job key's runs go out on now, as the road resolution answers it for the job key; the
// stand-in answering while the fallback stands, or Claude Code, where nothing of its walk can answer.
async fn job_road(ground: &Ground) -> Result<String, Refusal> {
  let fault = |refused: crate::control::Refused| crate::lines::fault(refused.error);

  if let Some(taken) = road::resolve(ground, &Ask::key(JOB_WORK_KEY))
    .await
    .map_err(crate::lines::fault)?
    .road()
  {
    return Ok(taken.provider.clone());
  }

  Ok(
    fallback_provider(&ground.db)
      .await
      .map_err(fault)?
      .unwrap_or_else(|| CLAUDE_CODE.to_owned()),
  )
}

// The road a pinned worker's pane stands on now: its pin where the pin can answer, the next road of its walk
// that can where it cannot (docs/cache-fallback.md, 「One road resolution」), with the model where that
// road's provider names its models one by one; its pin as it stands where nothing of the walk can answer.
// None for a worker with no pin.
async fn pinned_road(ground: &Ground, worker: &str) -> Result<Option<(String, Option<String>)>, Refusal> {
  let Some(pin) = worker_pin_held(&ground.db, worker)
    .await
    .map_err(|unrouted| crate::lines::fault(unrouted.0))?
  else {
    return Ok(None);
  };
  let ask = Ask {
    worker: Some(worker),
    ..Ask::key(JOB_WORK_KEY)
  };
  let resolution = road::resolve(ground, &ask).await.map_err(crate::lines::fault)?;
  let Some(taken) = resolution.road() else { return Ok(Some(pin)) };
  let names_models = provider(&taken.provider).is_some_and(|one| one.not_registered().is_some());

  Ok(Some((taken.provider.clone(), names_models.then(|| taken.model.clone()))))
}

/// What a pinned worker's pane holding another provider's program, or its own pin's program composed on
/// words that have since changed, is replaced by, at once rather than at the next turn on the pin: the program the pin stands with nothing put in, written before the old
/// one is ended, or — for a pin whose program stands only with a turn — the place handed back to the
/// host. A worker with no pin, a place holding its pin's own program, and a checkout that is not there
/// are left as they are; so is a place whose locks are held or whose pane is at work on a person's turn.
pub async fn restand_pinned(
  ground: &Ground,
  row: &AgentRow,
  wait: Option<Duration>,
) -> Result<(), Refusal> {
  if row.kind != "worker" || ground.copy().is_some() {
    return Ok(());
  }

  let Some((pinned, model)) = pinned_road(ground, &row.name).await? else {
    return Ok(());
  };
  let cwd = ground.checkout_of(row);
  let place = Place::of(&ground.roots.data, &row.name);
  let machine = ground.machine();
  let Some(pin) = provider(&pinned) else { return Ok(()) };

  if !Path::new(&cwd).exists() {
    return Ok(());
  }

  // The program a pin whose turns go through its pane program's inbox would stand now, by its identity:
  // one whose words changed under it is stood afresh. A program on Claude Code, the gateway's among them,
  // opens or places a session of its own as it is composed, so its own is left as it stands.
  let inbox = pin.declared().resident.as_ref().is_some_and(|resident| resident.turns == Turns::Inbox);
  let fresh = match inbox {
    true => pinned_program(ground, &machine, &row.name, &cwd, &pinned, model.as_deref())
      .await?
      .map(|standing| standing.program.identity),
    false => None,
  };
  let stray = || {
    place.program().filter(|program| {
      !pin.owns_program(&machine, &program.command.args, &program.command.env)
        || fresh.as_ref().is_some_and(|identity| *identity != program.identity)
    })
  };

  if stray().is_none() {
    return Ok(());
  }

  let Some(_locks) = worker_pane(ground, &row.name, &cwd, wait).await else {
    return Ok(());
  };

  // A turn that took the lock first may have stood the pin's program already.
  let Some(standing) = stray() else { return Ok(()) };

  if pane_at_work(ground, &place).await {
    return Ok(());
  }

  let was = standing_provider(&machine, &standing.command.args, &standing.command.env);
  let was_label =
    was.map_or(PLACE_WORDS.standby.unknown_provider.as_str(), |was| was.declared().label.as_str());
  let pin_label = &pin.declared().label;
  let replacement = if pinned == CLAUDE_CODE {
    Some(claude_program(ground, &machine, &row.name, &cwd).await?)
  } else {
    pinned_program(ground, &machine, &row.name, &cwd, &pinned, model.as_deref()).await?
  };
  let text = match replacement {
    Some(replacement) => {
      let (program, _) = tokened(ground, &row.name, &place, &cwd, replacement.program).await?;
      let (before, _) = place.replace(&program).await?;

      if let Some(old) = before.as_ref().and_then(token_of)
        && token_of(&program) != Some(old)
      {
        token::revoke(ground, old).await;
      }

      if pinned == CLAUDE_CODE {
        if let Some(standby) = replacement.standby {
          standby.write(&place.path)?;
        }

        fill_marks(&PLACE_WORDS.standby.replaced, &[("was", was_label), ("pin", pin_label)])
      } else {
        fill_marks(
          &PLACE_WORDS.standby.replaced_on,
          &[("was", was_label), ("pin", pin_label), ("model", &replacement.on)],
        )
      }
    }
    None => {
      if let Some(old) = place.release().await.as_ref().and_then(token_of) {
        token::revoke(ground, old).await;
      }

      fill_marks(&PLACE_WORDS.standby.released, &[("was", was_label), ("pin", pin_label)])
    }
  };

  note(ground, &row.name, &text).await?;
  println!(
    "standby {}: the {} program in its pane was replaced for its pin, {pinned}",
    row.name,
    was.map_or("unknown", |was| was.name())
  );

  Ok(())
}

/// The program a worker would stand with nothing put in, composed and not written: its pin's where it is
/// pinned to a provider other than Claude Code whose surface stands one, Claude Code's otherwise. None
/// for a name with no worker's row.
pub async fn compose(ground: &Ground, worker: &str) -> Result<Option<Standing>, Refusal> {
  let Some(row) = AgentRow::named(&ground.db, worker).await? else { return Ok(None) };
  let cwd = ground.checkout_of(&row);
  let machine = ground.machine();
  let pin = pinned_road(ground, worker).await?;

  match pin.as_ref().filter(|(pinned, _)| pinned != CLAUDE_CODE) {
    Some((pinned, model)) => pinned_program(ground, &machine, worker, &cwd, pinned, model.as_deref()).await,
    None => Ok(Some(claude_program(ground, &machine, worker, &cwd).await?)),
  }
}

/// A companion's program let go where its pin or its registration moved under it: a program on another
/// model than the pin names now, or on an address its registration no longer holds, is taken down so the
/// next turn stands the pin's own (docs/companions.md, 「The program on an ended session」). A program on
/// the pin's model and an address it holds is left as it stands.
pub async fn release_stale_companion(ground: &Ground, agent: &str) {
  let place = Place::of(&ground.roots.data, agent);
  let Some(program) = place.program() else { return };
  let Some(model) = program
    .command
    .args
    .windows(2)
    .find_map(|pair| (pair[0] == "--model").then(|| pair[1].clone()))
  else {
    return;
  };
  let pin: Option<String> = one(
    &ground.db,
    r#"SELECT "pinned_model" FROM "agents" WHERE "name" = $1 AND "pinned_provider" = 'registered'"#,
    vec![agent.into()],
  )
  .await
  .ok()
  .flatten()
  .and_then(|row| row.try_get::<Option<String>>("", "pinned_model").ok().flatten())
  .filter(|pin| !pin.trim().is_empty());
  let Some(pin) = pin else { return };
  let (name, node) =
    model.split_once('@').map_or((model.as_str(), None), |(name, node)| (name, Some(node)));
  let held = registry::registration(&ground.db, &pin).await.ok().flatten();
  let address_held = node.is_none_or(|node| {
    held
      .as_ref()
      .is_some_and(|held| held.addresses.iter().any(|address| address.node == node))
  });

  if name != pin || !address_held {
    let before = place.release().await;

    if before.is_some() {
      println!("companion {agent}: its program stood on {model}, not its pin {pin}; it was let go");
    }
  }
}

/// Every companion's standing program checked against its pin, as a pin or a registration changes: one on
/// another model, or on an address its registration lost, is let go.
pub async fn release_stale_companions(ground: &Ground) {
  let Ok(rows) = all(&ground.db, r#"SELECT "name" FROM "agents" WHERE "kind" = 'chat'"#, vec![]).await
  else {
    return;
  };

  for row in rows {
    if let Ok(name) = row.try_get::<String>("", "name") {
      release_stale_companion(ground, &name).await;
    }
  }
}

/// Every worker's pane checked against its pin, as a pin change or a job's end asks.
pub async fn restand_all(ground: &Ground, wait: Option<Duration>) {
  let rows = match AgentRow::standing(&ground.db).await {
    Ok(rows) => rows,
    Err(refusal) => return eprintln!("standby: {}", refusal.detail),
  };

  for row in &rows {
    if let Err(refusal) = restand_pinned(ground, row, wait).await {
      eprintln!("standby {}: {}", row.name, refusal.detail);
    }
  }
}
