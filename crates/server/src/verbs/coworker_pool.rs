//! A job's native `Agent` calls held to their roles' slots (docs/role-slots.md, 「Native co-worker slots」):
//! the hooks the worker settings run around every call — before it, a wait for a slot of the role the call
//! names and the call let out only once it has one; after it returns or fails, and when a background child
//! stops, the slot given back — and the job's own word about the calls it is about to make, slots reserved
//! under the CLI process it is run from and given back by `release` or once that process ends. A slot whose
//! waiter or caller process is gone is reclaimed on that evidence alone, never for its age.

use std::collections::BTreeMap;
use std::time::Duration;

use harness_machine::card_master::{Master, MasterAddress};
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::VerbError;
use crate::agent_roles::{self, AgentRole};
use crate::control::harness::named;
use crate::control::pool::NATIVE_PROVIDER;
use crate::now_ms;
use crate::order::Entry;
use crate::order_run::dispatch::{
  self, Asked, NATIVE, Seats, Wanted, acquire_inside, bind, finish, held_reservations, reclaim_native,
  release_reservation, request,
};
use store::{all, one, run};

/// What a co-worker's own call to open a co-worker meets: the fan-out is one level deep.
pub const NESTED_REFUSAL: &str = "The co-worker gate refused this: a co-worker cannot open a co-worker of its own — the fan-out is one level deep. Research this angle yourself with the tools you have, and report what you found.";

/// What a call on a run that stands on a single model meets: it has no co-worker apart from itself.
pub const SINGLE_MODEL_REFUSAL: &str = "The co-worker gate refused this: this run stands on a single model, which has no co-workers apart from the run itself. Do this part yourself with the tools you have.";

const POLL: Duration = Duration::from_millis(500);
const RECLAIM_EVERY: Duration = Duration::from_secs(5);

/// The usage the reservation verb answers a wrong line with.
pub const USAGE: &str = "usage: bmo-agency coworker pool reserve --role <role> --count <n> | release";

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

/// The provider owning the program the run on this machine stands on, read off the environment it hands
/// its hook; none where no provider owns it, which makes it Claude Code's.
fn caller_provider(
  machine: &harness_providers::Machine,
) -> Option<&'static dyn harness_providers::Provider> {
  let env: BTreeMap<String, String> =
    machine.vars().map(|(name, value)| (name.to_owned(), value.to_owned())).collect();

  harness_providers::standing_provider(machine, &["claude".to_owned()], &env)
    .and_then(|provider| named(provider.name()).ok())
}

/// Whether the run this process stands on lends co-workers: a run on a provider that lends none has none
/// apart from itself. A program no provider owns is Claude Code's, which lends them.
pub fn caller_lends_coworkers(machine: &harness_providers::Machine) -> bool {
  caller_provider(machine).is_none_or(|provider| provider.declared().lends_coworkers)
}

/// The provider a native co-worker of the run this process stands on takes its seat on: the run's own,
/// where that provider lends co-workers; none where it lends none or no provider owns the program, which
/// leaves the seat on Claude Code.
pub fn coworker_provider(machine: &harness_providers::Machine) -> Option<&'static str> {
  caller_provider(machine)
    .filter(|provider| provider.declared().lends_coworkers)
    .map(|provider| provider.name())
}

/// The road a callable role's native calls go out on from a program standing on `provider`: the model and
/// effort the role's order names for that provider. A role whose order holds no entry for it is refused, and
/// the caller does that part itself (docs/role-slots.md, 「Native co-worker slots」).
pub fn native_road(role: &AgentRole, provider: &str) -> Result<Entry, String> {
  role.order.entry(provider).cloned().ok_or_else(|| {
    format!(
      "the {} role's order holds no entry for {provider}, the provider this program stands on",
      role.name
    )
  })
}

// The roles a native call may name: the callable ones.
async fn callable_roles(db: &DatabaseConnection) -> Result<Vec<AgentRole>, String> {
  agent_roles::callable(db).await.map_err(|why| why.detail)
}

/// How a pre-call wait ended: the call let out on its place, or refused with why.
#[derive(Debug, PartialEq)]
pub enum Admission {
  Allowed { token: String },
  Refused(String),
}

/// A native call let into its role's slots: its role held to the callable ones and its road the entry the
/// role's order names for `on`, the provider its program stands on (Claude Code where none is named), its
/// request put in the role's line — counted under a reservation the calling CLI holds for that role, where it
/// holds one — and waited on until it is let in or ended, however long that takes. `waiting` hears the request's token
/// once it stands, so a stopped hook can end it.
pub async fn admit(
  db: &DatabaseConnection,
  input: &HookInput,
  on: Option<&str>,
  waiter: &ProcessMark,
  owner: &ProcessMark,
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

  let callable = callable_roles(db).await?;
  let Some(role) = coworker_type.as_deref().and_then(|role| callable.iter().find(|one| one.name == role))
  else {
    let named = coworker_type.as_deref().map_or_else(
      || "a call with no subagent_type".to_owned(),
      |role| format!("the subagent_type {role}"),
    );
    let roles = if callable.is_empty() {
      "(no role may be called)".to_owned()
    } else {
      callable.iter().map(|role| role.name.as_str()).collect::<Vec<_>>().join(", ")
    };

    return Ok(Admission::Refused(format!(
      "The co-worker gate refused this: {named} is no role a run may call, and a type outside the roles would run on the parent's model. Name one of: {roles}; or do this part yourself."
    )));
  };
  let provider = on.unwrap_or(NATIVE_PROVIDER);
  let road = match native_road(role, provider) {
    Ok(road) => road,
    Err(why) => {
      return Ok(Admission::Refused(format!(
        "The co-worker gate refused this: {why}; do this part yourself."
      )));
    }
  };

  reclaim_native(db).await?;

  // A call of a CLI that reserved this role is counted under that reservation's owner, which is what lets it
  // take a held slot instead of waiting in the line.
  let reserved = held_reservations(db, Some(&cli_owner(owner)))
    .await?
    .iter()
    .any(|held| held.role == role.name);
  let asked = Asked {
    request_key: format!("native:{session}:{tool_use_id}"),
    owner: if reserved { cli_owner(owner) } else { format!("native:{session}") },
    wanted: Wanted {
      role: &role.name,
      path: None,
      fixed: Some(road),
    },
    ..Asked::default()
  };
  let role = role.name.as_str();
  let row = (session.clone(), tool_use_id.clone(), waiter.clone(), owner.clone());
  let held = match dispatch::enqueue(db, &asked, NATIVE, async move |unit, held| {
    run(
      unit,
      r#"INSERT INTO "role_slot_native" ("token","session","tool_use_id","waiter_pid","waiter_start","owner_pid","owner_start")
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

  let mut reclaimed = tokio::time::Instant::now();

  loop {
    let Some(current) = request(db, &held.token).await? else {
      return Ok(ended(role));
    };

    if acquire_inside(db, &current).await?.is_some() {
      let admitted = request(db, &held.token).await?.ok_or("the slot request let in is gone")?;

      bind(db, &admitted, tool_use_id).await?;

      return Ok(Admission::Allowed { token: held.token });
    }

    if request(db, &held.token).await?.is_none_or(|now| now.state != "waiting") {
      return Ok(ended(role));
    }

    if reclaimed.elapsed() >= RECLAIM_EVERY {
      reclaim_native(db).await?;
      reclaimed = tokio::time::Instant::now();
    }

    tokio::time::sleep(POLL).await;
  }
}

fn ended(role: &str) -> Admission {
  Admission::Refused(format!(
    "The co-worker gate refused this: the slot request for this {role} call was ended before it got a slot."
  ))
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
    r#"SELECT "token" FROM "role_slot_native" WHERE "session" = $1 AND "tool_use_id" = $2"#,
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
          r#"SELECT "agent_id" FROM "role_slot_native_stops" WHERE "agent_id" = $1"#,
          vec![agent.into()],
        )
        .await
        .map_err(|why| why.detail)?;

        if stopped.is_none() {
          run(
            db,
            r#"UPDATE "role_slot_native" SET "agent_id" = $1 WHERE "token" = $2"#,
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
        r#"SELECT "token" FROM "role_slot_native" WHERE "agent_id" = $1"#,
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
        r#"INSERT INTO "role_slot_native_stops" VALUES ($1, $2) ON CONFLICT ("agent_id") DO UPDATE SET "at" = excluded."at""#,
        vec![agent_id.clone().into(), now_ms().into()],
      )
      .await
      .map(drop)
      .map_err(|why| why.detail)
    }
  }
}

/// Slots of a callable role held for the native calls one CLI is about to make; the line saying how many are
/// held and how many still wait.
pub async fn reserve(
  db: &DatabaseConnection,
  owner: &ProcessMark,
  role: &str,
  count: i64,
) -> Result<Vec<String>, VerbError> {
  let failed = |why: String| VerbError::Ended {
    code: 1,
    said: format!("coworker-pool: {why}"),
  };
  let callable = callable_roles(db).await.map_err(failed)?;

  if !callable.iter().any(|one| one.name == role) {
    let names: Vec<&str> = callable.iter().map(|role| role.name.as_str()).collect();

    return Err(failed(format!("{role} is no role a run may call; name one of: {}", names.join(", "))));
  }

  let key = cli_owner(owner);
  let seats = Seats {
    reservation_key: format!("{key}:{role}"),
    owner: key,
    source: NATIVE,
    role,
    count,
  };
  let held = dispatch::reserve(db, &seats).await.map_err(failed)?;

  run(
    db,
    r#"INSERT INTO "role_slot_native_reservations" VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"#,
    vec![held.token.clone().into(), owner.pid.into(), owner.start.clone().into()],
  )
  .await?;

  let states = all(
    db,
    r#"SELECT "state" FROM "role_slot_requests" WHERE "reservation" = $1"#,
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
    "{}: {} of {} slots held, {} waiting",
    held.role,
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
  let master = MasterAddress::of_process()?.map(Master::new);

  if !matches!(input, HookInput::Pre { .. }) {
    give_back(&db, &input).await?;

    return returned_card(&db, master.as_ref(), data_root, &input).await;
  }

  // A run on a provider that lends no co-workers has none apart from itself, so its call takes no place
  // and goes nowhere: the worker does that part itself.
  let machine = harness_providers::Machine::of_process(data_root);

  if !caller_lends_coworkers(&machine) {
    return Ok(vec![refusal(SINGLE_MODEL_REFUSAL)]);
  }

  let on = coworker_provider(&machine);

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
    admitted = admit(&db, &input, on, &waiter, &owner, &mut heard) => Some(admitted?),
    _ = interrupt.recv() => None,
    _ = terminate.recv() => None,
  };
  // The token is read once the wait has let go of it, never while it is held across a step.
  let Some(admitted) = admitted else {
    let token = waiting.borrow().clone();

    return stopped(&db, token).await;
  };

  // A seat on a registration whose engine the arbiter stands waits for its card, asked within the demand
  // its caller's turn holds, however long that takes (docs/own-llm.md, 「An engine the arbiter stands」).
  if let Admission::Allowed { token } = &admitted
    && let Some(seat) = request(&db, token).await.map_err(|why| why.to_string())?
  {
    let caller = std::env::var(crate::lines::place::PLACE_VARIABLE).ok().and_then(|place| {
      std::path::Path::new(&place).file_name().map(|name| name.to_string_lossy().into_owned())
    });
    let carded = tokio::select! {
      carded = crate::cards::native_card(&db, master.as_ref(), &seat, caller.as_deref()) => Some(carded),
      _ = interrupt.recv() => None,
      _ = terminate.recv() => None,
    };

    match carded {
      Some(Ok(_)) => {}
      Some(Err(why)) => {
        finish(&db, token, "native call found no card", true).await?;

        return Ok(vec![refusal(&format!("The co-worker gate refused this: {}", why.detail))]);
      }
      None => {
        finish(&db, token, "native call's waiting hook was stopped", true).await?;

        return Err("stopped while waiting for a card".into());
      }
    }
  }

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

// The card a native call's seat held, given back once the call has returned or failed and its place with
// it; a call that went on as a background child keeps its card until the pool's clock finds its lease
// ended. Where the caller's own card was lent for it, the hook holds the caller's next request until the
// caller's engine is back, and tells it so.
async fn returned_card(
  db: &DatabaseConnection,
  master: Option<&Master>,
  data_root: &std::path::Path,
  input: &HookInput,
) -> Result<Vec<String>, String> {
  let (
    Some(master),
    HookInput::Post {
      session,
      tool_use_id,
      ..
    }
    | HookInput::Failure {
      session,
      tool_use_id,
    },
  ) = (master, input)
  else {
    return Ok(vec![]);
  };
  let Some(token) = token_of(db, session, tool_use_id).await? else { return Ok(vec![]) };

  if request(db, &token)
    .await
    .map_err(|why| why.to_string())?
    .is_some_and(|seat| seat.state == "active")
  {
    return Ok(vec![]);
  }

  let said = crate::cards::give_back_lease(db, master, data_root, &token)
    .await
    .map_err(|why| why.detail)?;
  let event = if matches!(input, HookInput::Post { .. }) {
    "PostToolUse"
  } else {
    "PostToolUseFailure"
  };

  Ok(
    said
      .map(|said| {
        json!({ "hookSpecificOutput": { "hookEventName": event, "additionalContext": said } }).to_string()
      })
      .into_iter()
      .collect(),
  )
}

// A hook stopped while it waited: its request ended, and the hook's blocking exit.
async fn stopped(db: &DatabaseConnection, token: Option<String>) -> Result<Vec<String>, String> {
  if let Some(token) = token {
    cancel(db, &token).await?;
  }

  Err("stopped while waiting for a place".into())
}
