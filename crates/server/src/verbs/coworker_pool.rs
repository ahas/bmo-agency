//! A job's native `Agent` calls held to the common co-worker pool (docs/coworker-pool.md, 「Native co-worker
//! places」): the hooks the worker's settings run around every call — before it, a wait for a place and the
//! call let out only once it has one; after it returns or fails, and when a background child stops, the
//! place given back — and the job's own word about the calls it is about to make, seats reserved under the
//! CLI process it is run from and given back by `release` or once that process ends. A place whose waiter
//! or caller process is gone is reclaimed on that evidence alone, never for its age.

use std::collections::BTreeMap;
use std::time::Duration;

use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::VerbError;
use crate::control::harness::named;
use crate::control::pool::roles;
use crate::control::tiers::{self, Mapping};
use crate::now_ms;
use crate::order_run::dispatch::{
  self, Asked, NATIVE, Seats, Wanted, acquire, bind, enqueue_native, finish, held_reservations,
  reclaim_native, release_reservation, request,
};
use store::{all, one, run};

/// What a co-worker's own call to open a co-worker meets: the fan-out is one level deep.
pub const NESTED_REFUSAL: &str = "The co-worker gate refused this: a co-worker cannot open a co-worker of its own — the fan-out is one level deep. Research this angle yourself with the tools you have, and report what you found.";

/// What a call on a run that stands on a single model meets: it has no co-worker apart from itself.
pub const SINGLE_MODEL_REFUSAL: &str = "The co-worker gate refused this: this run stands on a single model, which has no co-workers apart from the run itself. Do this part yourself with the tools you have.";

/// How long a native call waits for a place before it is refused.
pub const NATIVE_WAIT: Duration = Duration::from_secs(4 * 60);
const POLL: Duration = Duration::from_millis(500);
const RECLAIM_EVERY: Duration = Duration::from_secs(5);

/// The usage the reservation verb answers a wrong line with.
pub const USAGE: &str =
  "usage: bmo-agency coworker pool reserve --role <role> [--tier <tier>] --count <n> | release";

/// A process marked by its id and the kernel's start time, so a pid the kernel handed to a later process is
/// not taken for the one marked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessMark {
  pub pid: i64,
  pub start: String,
}

fn stat_fields(pid: i64) -> Option<Vec<String>> {
  let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;

  Some(stat[stat.rfind(')')? + 2..].split(' ').map(str::to_owned).collect())
}

/// A process's mark, where it stands.
pub fn process_mark(pid: i64) -> Option<ProcessMark> {
  stat_fields(pid)?.get(19).map(|start| ProcessMark {
    pid,
    start: start.clone(),
  })
}

// The programs a hook command may stand inside without being the CLI that called it.
const SHELLS: [&str; 6] = ["sh", "bash", "dash", "zsh", "timeout", "env"];

/// The CLI process a hook was started by: the first ancestor that is not the shell the hook command runs
/// in.
pub fn hook_caller(parent: i64) -> Option<ProcessMark> {
  let mut current = parent;

  for _ in 0..8 {
    if current <= 1 {
      return None;
    }

    let comm = std::fs::read_to_string(format!("/proc/{current}/comm")).ok()?;

    if !SHELLS.contains(&comm.trim()) {
      return process_mark(current);
    }

    current = stat_fields(current)?.get(1)?.parse().ok()?;
  }

  None
}

// The owner a CLI's reservations are held under: the process itself, so its end is the evidence.
fn cli_owner(mark: &ProcessMark) -> String {
  format!("native-cli:{}:{}", mark.pid, mark.start)
}

/// One hook's input, as the CLI hands it on standard input.
#[derive(Debug, PartialEq)]
pub enum HookInput {
  Pre {
    session: String,
    tool_use_id: String,
    coworker_type: Option<String>,
    caller_agent_id: Option<String>,
    tool_input: Value,
  },
  Post {
    session: String,
    tool_use_id: String,
    status: Option<String>,
    agent_id: Option<String>,
  },
  Failure {
    session: String,
    tool_use_id: String,
  },
  Stop {
    agent_id: String,
  },
}

fn field(value: &Value, name: &str) -> Option<String> {
  value
    .get(name)
    .and_then(Value::as_str)
    .filter(|text| !text.is_empty())
    .map(str::to_owned)
}

fn required(value: &Value, name: &str) -> Result<String, String> {
  field(value, name).ok_or_else(|| format!("the hook input carries no {name}"))
}

/// The hook's input read, or why it could not be.
pub fn hook_input(text: &str) -> Result<HookInput, String> {
  let input: Value = serde_json::from_str(text).map_err(|why| why.to_string())?;
  let event = required(&input, "hook_event_name")?;
  let session = required(&input, "session_id")?;

  match event.as_str() {
    "PreToolUse" => {
      let tool = input.get("tool_input").cloned().unwrap_or_else(|| json!({}));

      Ok(HookInput::Pre {
        session,
        tool_use_id: required(&input, "tool_use_id")?,
        coworker_type: field(&tool, "subagent_type"),
        caller_agent_id: field(&input, "agent_id"),
        tool_input: tool,
      })
    }
    "PostToolUse" => {
      let response = input
        .get("tool_response")
        .filter(|one| one.is_object())
        .cloned()
        .unwrap_or_else(|| json!({}));

      Ok(HookInput::Post {
        session,
        tool_use_id: required(&input, "tool_use_id")?,
        status: field(&response, "status"),
        agent_id: field(&response, "agentId"),
      })
    }
    "PostToolUseFailure" => Ok(HookInput::Failure {
      session,
      tool_use_id: required(&input, "tool_use_id")?,
    }),
    "SubagentStop" => Ok(HookInput::Stop {
      agent_id: required(&input, "agent_id")?,
    }),
    other => Err(format!("the native co-worker hook does not handle {other}")),
  }
}

/// What the pre-call hook answers a call that got its place: the call let out with its prompt carrying the
/// guide the caller hands over after whatever the caller wrote; a call whose input has no prompt is let out
/// untouched.
pub fn allowance(tool_input: &Value, guide: &str) -> Option<String> {
  let prompt = tool_input.get("prompt").and_then(Value::as_str)?;
  let mut updated = tool_input.clone();

  updated["prompt"] = Value::String(format!("{prompt}\n\n{guide}"));

  Some(
    json!({
      "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "allow", "updatedInput": updated },
    })
    .to_string(),
  )
}

/// What the CLI is told where a call is refused: the hook's own refusal shape.
pub fn refusal(reason: &str) -> String {
  json!({
    "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": reason },
  })
  .to_string()
}

/// Whether the run this process stands on lends co-workers: a run on a single model has none apart from
/// itself. A program no provider owns is Claude Code's, which lends them.
pub fn caller_lends_coworkers(machine: &harness_providers::Machine) -> bool {
  let env: BTreeMap<String, String> = std::env::vars().collect();

  harness_providers::standing_provider(machine, &["claude".to_owned()], &env)
    .and_then(|provider| named(provider.name()).ok())
    .is_none_or(|provider| provider.declared().lends_coworkers)
}

/// The provider, tier and model a pooled role's native calls run on.
pub async fn role_mapping(db: &DatabaseConnection, role: &str) -> Result<(String, String, String), String> {
  let mapping = tiers::resolve(db, &format!("role:{role}"), "claude-code").await?;

  Ok((mapping.provider, mapping.tier, mapping.model))
}

async fn pooled_roles(db: &DatabaseConnection) -> Result<Vec<String>, String> {
  Ok(
    roles(db, true)
      .await
      .map_err(|why| why.error)?
      .into_iter()
      .map(|role| role.name)
      .collect(),
  )
}

/// How a pre-call wait ended: the call let out on its place, or refused with why.
#[derive(Debug, PartialEq)]
pub enum Admission {
  Allowed { token: String },
  Refused(String),
}

/// A native call admitted to the pool: its role held to the pooled ones and resolved to its Claude tier,
/// its request put in the line — counted under a reservation the calling CLI holds for that tier, where it
/// holds one — and waited on until it is let in, ended, or the wait runs out. `waiting` hears the request's
/// token once it stands, so a stopped hook can end it.
pub async fn admit(
  db: &DatabaseConnection,
  input: &HookInput,
  waiter: &ProcessMark,
  owner: &ProcessMark,
  wait: Duration,
  waiting: &mut impl FnMut(&str),
) -> Result<Admission, String> {
  let HookInput::Pre {
    session,
    tool_use_id,
    coworker_type,
    caller_agent_id,
    ..
  } = input
  else {
    return Err("only a call about to be made is admitted".into());
  };

  if caller_agent_id.is_some() {
    return Ok(Admission::Refused(NESTED_REFUSAL.into()));
  }

  let pooled = pooled_roles(db).await?;
  let Some(role) = coworker_type.as_deref().filter(|role| pooled.iter().any(|one| one == role)) else {
    let named = coworker_type.as_deref().map_or_else(
      || "a call with no subagent_type".to_owned(),
      |role| format!("the subagent_type {role}"),
    );
    let roles = if pooled.is_empty() {
      "(no pooled role is enabled)".to_owned()
    } else {
      pooled.join(", ")
    };

    return Ok(Admission::Refused(format!(
      "The co-worker gate refused this: {named} is not a role of the common spawning pool, and a type outside it would run on the parent's model. Name one of: {roles}; or do this part yourself."
    )));
  };
  let mapping = match tiers::resolve(db, &format!("role:{role}"), "claude-code").await {
    Ok(mapping) => mapping,
    Err(why) => {
      return Ok(Admission::Refused(format!(
        "The co-worker gate refused this: the {role} role resolves to no Claude model ({why})."
      )));
    }
  };

  reclaim_native(db).await?;

  // A call of a CLI that reserved this tier is counted under that reservation's owner, which is what lets
  // it take a held seat instead of waiting in the line.
  let reserved = held_reservations(db, Some(&cli_owner(owner)))
    .await?
    .iter()
    .any(|held| held.provider == mapping.provider && held.tier == mapping.tier);
  let asked = Asked {
    request_key: format!("native:{session}:{tool_use_id}"),
    owner: if reserved { cli_owner(owner) } else { format!("native:{session}") },
    wanted: Wanted {
      role,
      selection: "",
      requested_tier: None,
      provider: None,
      work_key: None,
    },
    generation: 1,
    yielding: false,
    token: None,
    place_key: None,
  };
  let row = (session.clone(), tool_use_id.clone(), waiter.clone(), owner.clone());
  let held = match enqueue_native(db, &asked, &mapping, async move |unit, held| {
    run(
      unit,
      r#"INSERT INTO "coworker_pool_native" ("token","session","tool_use_id","waiter_pid","waiter_start","owner_pid","owner_start")
         VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING"#,
      vec![
        held.token.clone().into(),
        row.0.into(),
        row.1.into(),
        row.2.pid.into(),
        row.2.start.into(),
        row.3.pid.into(),
        row.3.start.into(),
      ],
    )
    .await
    .map(drop)
    .map_err(|why| why.detail)
  })
  .await
  {
    Ok(held) => held,
    Err(why) => return Ok(Admission::Refused(format!("The co-worker gate refused this: {why}."))),
  };

  waiting(&held.token);

  let deadline = tokio::time::Instant::now() + wait;
  let mut reclaimed = tokio::time::Instant::now();

  loop {
    let Some(current) = request(db, &held.token).await? else {
      return Ok(ended(role));
    };

    if acquire(db, &current).await?.is_some() {
      let admitted = request(db, &held.token).await?.ok_or("the pool request admitted is gone")?;

      bind(db, &admitted, tool_use_id).await?;

      return Ok(Admission::Allowed { token: held.token });
    }

    if request(db, &held.token).await?.is_none_or(|now| now.state != "waiting") {
      return Ok(ended(role));
    }

    if tokio::time::Instant::now() >= deadline {
      finish(db, &held.token, "native call found no place in time", true).await?;

      return Ok(Admission::Refused(no_place(role, &mapping, wait)));
    }

    if reclaimed.elapsed() >= RECLAIM_EVERY {
      reclaim_native(db).await?;
      reclaimed = tokio::time::Instant::now();
    }

    // The last look falls on the deadline, so a wait shorter than a poll is refused once it has run out.
    tokio::time::sleep(POLL.min(deadline.saturating_duration_since(tokio::time::Instant::now()))).await;
  }
}

fn ended(role: &str) -> Admission {
  Admission::Refused(format!(
    "The co-worker gate refused this: the pool request for this {role} call was ended before it got a place."
  ))
}

fn no_place(role: &str, mapping: &Mapping, wait: Duration) -> String {
  format!(
    "The co-worker gate refused this: the common spawning pool had no free {} place ({}) for a {role} within {} seconds. Do this part yourself with the tools you have. Before opening several co-workers of one tier, hold their places first with `~/.local/share/bmo-agency/deploy/bin/bmo-agency coworker pool reserve --role <role> --count <n>` and give them back with `~/.local/share/bmo-agency/deploy/bin/bmo-agency coworker pool release`.",
    mapping.tier,
    mapping.model,
    wait.as_secs()
  )
}

/// A waiting request whose hook is being stopped, ended so it never stands at the head of its line.
pub async fn cancel(db: &DatabaseConnection, token: &str) -> Result<(), String> {
  finish(db, token, "native call's waiting hook was stopped", true).await.map(drop)
}

async fn token_of(
  db: &DatabaseConnection,
  session: &str,
  tool_use_id: &str,
) -> Result<Option<String>, String> {
  one(
    db,
    r#"SELECT "token" FROM "coworker_pool_native" WHERE "session" = $1 AND "tool_use_id" = $2"#,
    vec![session.into(), tool_use_id.into()],
  )
  .await
  .map_err(|why| why.detail)?
  .map(|row| row.try_get::<String>("", "token"))
  .transpose()
  .map_err(|why| why.to_string())
}

/// The place a call held given back once the call returned, failed or its background child stopped; a call
/// that went on as a background child keeps its place until that child stops.
pub async fn give_back(db: &DatabaseConnection, input: &HookInput) -> Result<(), String> {
  match input {
    HookInput::Pre { .. } => Err("a call about to be made gives nothing back".into()),
    HookInput::Post {
      session,
      tool_use_id,
      status,
      agent_id,
    } => {
      let Some(token) = token_of(db, session, tool_use_id).await? else { return Ok(()) };

      if let (Some("async_launched"), Some(agent)) = (status.as_deref(), agent_id.as_deref()) {
        let stopped = one(
          db,
          r#"SELECT "agent_id" FROM "coworker_pool_native_stops" WHERE "agent_id" = $1"#,
          vec![agent.into()],
        )
        .await
        .map_err(|why| why.detail)?;

        if stopped.is_none() {
          run(
            db,
            r#"UPDATE "coworker_pool_native" SET "agent_id" = $1 WHERE "token" = $2"#,
            vec![agent.into(), token.into()],
          )
          .await
          .map_err(|why| why.detail)?;

          return Ok(());
        }
      }

      finish(db, &token, "native call returned", false).await.map(drop)
    }
    HookInput::Failure {
      session,
      tool_use_id,
    } => match token_of(db, session, tool_use_id).await? {
      Some(token) => finish(db, &token, "native call failed", false).await.map(drop),
      None => Ok(()),
    },
    HookInput::Stop { agent_id } => {
      let row = one(
        db,
        r#"SELECT "token" FROM "coworker_pool_native" WHERE "agent_id" = $1"#,
        vec![agent_id.clone().into()],
      )
      .await
      .map_err(|why| why.detail)?;

      if let Some(row) = row {
        let token: String = row.try_get("", "token").map_err(|why| why.to_string())?;

        return finish(db, &token, "native co-worker stopped", false).await.map(drop);
      }

      run(
        db,
        r#"INSERT INTO "coworker_pool_native_stops" VALUES ($1, $2) ON CONFLICT ("agent_id") DO UPDATE SET "at" = excluded."at""#,
        vec![agent_id.clone().into(), now_ms().into()],
      )
      .await
      .map(drop)
      .map_err(|why| why.detail)
    }
  }
}

/// Seats held for the native calls one CLI is about to make, on the Claude Code tier its role — or the tier
/// it names — resolves to; the line saying how many are held and how many still wait for a place.
pub async fn reserve(
  db: &DatabaseConnection,
  owner: &ProcessMark,
  role: &str,
  tier: Option<&str>,
  count: i64,
) -> Result<Vec<String>, VerbError> {
  let failed = |why: String| VerbError::Ended {
    code: 1,
    said: format!("coworker-pool: {why}"),
  };
  let pooled = pooled_roles(db).await.map_err(failed)?;

  if !pooled.iter().any(|one| one == role) {
    return Err(failed(format!(
      "{role} is not a role of the common spawning pool; name one of: {}",
      pooled.join(", ")
    )));
  }

  let mapping = match tier {
    None => tiers::resolve(db, &format!("role:{role}"), "claude-code").await,
    Some(tier) => tiers::resolve_tier(db, tier, "claude-code").await,
  }
  .map_err(failed)?;
  let key = cli_owner(owner);
  let seats = Seats {
    reservation_key: format!("{key}:{role}:{}", mapping.tier),
    owner: key,
    source: NATIVE,
    wanted: Wanted {
      role,
      selection: "",
      requested_tier: None,
      provider: None,
      work_key: None,
    },
    count,
  };
  let held = dispatch::reserve_on(db, &seats, &mapping).await.map_err(failed)?;

  run(
    db,
    r#"INSERT INTO "coworker_pool_native_reservations" VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"#,
    vec![held.token.clone().into(), owner.pid.into(), owner.start.clone().into()],
  )
  .await?;

  let states = all(
    db,
    r#"SELECT "state" FROM "coworker_pool_requests" WHERE "reservation" = $1"#,
    vec![held.token.clone().into()],
  )
  .await?;
  let count_of = |state: &str| {
    states
      .iter()
      .filter(|row| row.try_get::<String>("", "state").ok().as_deref() == Some(state))
      .count()
  };

  Ok(vec![format!(
    "{}/{} {}: {} of {} seats held, {} waiting for a place",
    held.provider,
    held.tier,
    held.model,
    count_of("reserved"),
    held.wanted,
    count_of("waiting")
  )])
}

/// Every reservation one CLI holds given back; the line saying how many.
pub async fn release(db: &DatabaseConnection, owner: &ProcessMark) -> Result<Vec<String>, VerbError> {
  let failed = |why: String| VerbError::Ended {
    code: 1,
    said: format!("coworker-pool: {why}"),
  };
  let mut given = 0;

  for held in held_reservations(db, Some(&cli_owner(owner))).await.map_err(failed)? {
    if release_reservation(db, &held.token, "released by the job").await.map_err(failed)? {
      given += 1;
    }
  }

  Ok(vec![format!("{given} reservation(s) given back")])
}

/// One run of the hook on the input the CLI handed it: the refusal or the allowance a call about to be made
/// is answered with, nothing for the hooks that give a place back. A pre-call hook that cannot judge
/// refuses with the hook's blocking exit; the hooks that give a place back only say a failure, since the
/// caller's own end reclaims that place.
pub async fn hook(data_root: &std::path::Path, text: &str) -> super::Answer {
  let pre = hook_input(text).is_ok_and(|input| matches!(input, HookInput::Pre { .. }))
    || text.contains("\"PreToolUse\"");

  match judged(data_root, text).await {
    Ok(lines) => lines.into(),
    Err(why) => {
      eprintln!("BMO's co-worker pool hook: {why}");

      super::Answer {
        lines: vec![],
        code: if pre { 2 } else { 0 },
      }
    }
  }
}

async fn judged(data_root: &std::path::Path, text: &str) -> Result<Vec<String>, String> {
  use std::cell::RefCell;
  use tokio::signal::unix::{SignalKind, signal};

  let input = hook_input(text)?;
  let db = super::store(data_root).await.map_err(|why| why.to_string())?;

  if !matches!(input, HookInput::Pre { .. }) {
    give_back(&db, &input).await?;

    return Ok(vec![]);
  }

  // A run on a single model has no co-worker apart from itself, so its call takes no place and goes
  // nowhere: the worker does that part itself.
  if !caller_lends_coworkers(&harness_providers::Machine::of_process(data_root)) {
    return Ok(vec![refusal(SINGLE_MODEL_REFUSAL)]);
  }

  // SAFETY: getppid has no preconditions.
  let parent = i64::from(unsafe { libc::getppid() });
  let (Some(waiter), Some(owner)) = (process_mark(i64::from(std::process::id())), hook_caller(parent))
  else {
    return Err("the hook could not identify its own process or the CLI that called it".into());
  };
  let waiting: RefCell<Option<String>> = RefCell::new(None);
  let mut interrupt = signal(SignalKind::interrupt()).map_err(|why| why.to_string())?;
  let mut terminate = signal(SignalKind::terminate()).map_err(|why| why.to_string())?;
  let mut heard = |token: &str| *waiting.borrow_mut() = Some(token.to_owned());
  let admitted = tokio::select! {
    admitted = admit(&db, &input, &waiter, &owner, NATIVE_WAIT, &mut heard) => Some(admitted?),
    _ = interrupt.recv() => None,
    _ = terminate.recv() => None,
  };
  // The token is read once the wait has let go of it, never while it is held across a step.
  let Some(admitted) = admitted else {
    let token = waiting.borrow().clone();

    return stopped(&db, token).await;
  };

  Ok(match admitted {
    Admission::Refused(reason) => vec![refusal(&reason)],
    Admission::Allowed { .. } => {
      let HookInput::Pre { tool_input, .. } = &input else {
        unreachable!("judged as a call about to be made")
      };
      let guide = std::fs::read_to_string(super::code_tree().join("prompts/agents/worker/graph.md"))
        .unwrap_or_default();

      allowance(tool_input, guide.trim()).into_iter().collect()
    }
  })
}

// A hook stopped while it waited: its request ended, and the hook's blocking exit.
async fn stopped(db: &DatabaseConnection, token: Option<String>) -> Result<Vec<String>, String> {
  if let Some(token) = token {
    cancel(db, &token).await?;
  }

  Err("stopped while waiting for a place".into())
}
