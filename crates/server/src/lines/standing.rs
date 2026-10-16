//! The program a worker stands with nothing put in (docs/agent-control.md, 「A worker standing with
//! nothing to do」): Claude Code on the worker's own line, on its kept session or a fresh one; or, for a
//! worker pinned to a provider whose surface stands with nothing put in, that provider's program — the
//! registry's Claude Code on the gateway, or the deployment's loop for OpenRouter.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use harness_providers::claude_code::ClaudeCode;
use harness_providers::registered::Registered;
use harness_providers::{Machine, Provider};
use policy::providers::registered::REGISTERED;
use policy::run_lines::RUN_LINES;
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use super::claude::{self, Command, Session};
use super::place::{
  Holds, Place, Program, Standby, command_digest, read_settings, resident_of, write_place_settings,
};
use super::prefix::{PrefixReading, prefix_digest};
use super::worker::{
  self, LINE, Reasoning, WorkerRun, configured, effort_of, worker_run, worker_variables,
};
use super::{Trees, fault};
use crate::control::claude_accounts::{self, Account, SEED_ACCOUNT};
use crate::control::registry;
use crate::places::Ground;
use crate::places::trust::{config_file, mark_onboarded, trust_folder};
use crate::queue::routes::JOB_WORK_KEY;
use crate::{Refusal, now_ms};
use store::{all, one, run};

/// The model a worker's run is first composed on, before the job key's tier lays its own on it: the
/// value a base prompt's `{{model}}` reads in a pinned provider's program.
pub const WORKER_MODEL: &str = "claude-opus-5";

/// The program a worker stands, and the session it opened where it opened one with nothing said in it
/// yet.
#[derive(Serialize)]
pub struct Standing {
  pub program: Program,
  pub standby: Option<Standby>,
  /// What the program stands on, for the line the pass writes.
  pub on: String,
}

// The login a session record names: its account, or the seed account a record naming none stands for.
fn recorded(account: Option<&Account>) -> String {
  account.map_or_else(|| SEED_ACCOUNT.to_owned(), |account| account.id.clone())
}

/// How long a worker's kept session stands unused before it is retired, where the environment gives
/// one; none by default, since a warm agent stays warm.
pub fn session_ttl_ms(ground: &Ground) -> Option<i64> {
  ground
    .vars()
    .get("BMO_WORKER_SESSION_TTL_MINUTES")
    .and_then(|minutes| minutes.trim().parse::<i64>().ok())
    .filter(|minutes| *minutes > 0)
    .map(|minutes| minutes * 60_000)
}

// The worker's kept session on this model and login, where one is resumable: one idle past the life a
// session is given is retired.
async fn resumable_session(
  ground: &Ground,
  worker: &str,
  model: &str,
  account: &str,
) -> Result<Option<(String, String, Option<String>)>, Refusal> {
  let row = one(
    &ground.db,
    r#"SELECT "claude_session_id", "last_reply_at", "answered_model", "account" FROM "worker_sessions"
       WHERE "worker" = $1 AND "model" = $2 AND "account" = $3"#,
    vec![worker.into(), model.into(), account.into()],
  )
  .await?;
  let Some(row) = row else { return Ok(None) };
  let last_reply_at: i64 = row.try_get("", "last_reply_at")?;

  if let Some(ttl) = session_ttl_ms(ground)
    && now_ms() - last_reply_at * 1000 > ttl
  {
    run(
      &ground.db,
      r#"DELETE FROM "worker_sessions" WHERE "worker" = $1 AND "model" = $2 AND "account" = $3"#,
      vec![worker.into(), model.into(), account.into()],
    )
    .await?;

    return Ok(None);
  }

  Ok(Some((
    row.try_get("", "claude_session_id")?,
    row.try_get("", "account")?,
    row.try_get("", "answered_model")?,
  )))
}

// The model a kept session goes on: the one it fell to, where the worker does not heal a downgrade.
async fn session_launch_model(
  ground: &Ground,
  worker: &str,
  fallen: Option<String>,
  model: String,
) -> Result<String, Refusal> {
  let Some(fallen) = fallen else { return Ok(model) };
  let heals = one(
    &ground.db,
    r#"SELECT "downgrade_heal" FROM "agents" WHERE "name" = $1"#,
    vec![worker.into()],
  )
  .await?
  .map(|row| row.try_get::<i64>("", "downgrade_heal"))
  .transpose()?
  .is_none_or(|heal| heal != 0);

  Ok(if heals { model } else { fallen })
}

/// The prefix the worker line opens a session on in `cwd`, on `model` at `effort` — the line's own where
/// none is named — as one short word.
pub async fn worker_prefix_digest(
  ground: &Ground,
  trees: &Trees,
  cwd: &str,
  model: &str,
  effort: Option<&str>,
) -> Result<String, Refusal> {
  let source = trees.source.to_string_lossy().into_owned();
  let mut line = worker_run(ground, trees, None, model, &source).await?;

  if let Some(effort) = effort {
    line.effort = effort.to_owned();
  }

  let args = claude::worker_args(&line, &line.settings, None)?;

  Ok(prefix_digest(&PrefixReading {
    cwd: Path::new(cwd),
    args: &args,
    code_root: &trees.code,
    source_root: &trees.source,
    data_root: &trees.data,
    home: &trees.home,
    plugin_dir: &line.plugin_dir,
    settings: &line.settings,
    rules: &line.rules,
    mcp_config: &line.mcp_config,
  }))
}

/// Claude Code's standing program for a worker: the job's line on the configured job model and the
/// login the walk offers now, on the worker's kept session where one is resumable and on a fresh one
/// otherwise, its settings the worker's own with the place's status line and hooks.
pub async fn claude_program(
  ground: &Ground,
  machine: &Machine,
  worker: &str,
  cwd: &str,
) -> Result<Standing, Refusal> {
  let trees = Trees::of(ground);
  let walked = claude_accounts::walk_for(&ground.db, JOB_WORK_KEY)
    .await
    .map_err(|refused| fault(refused.error))?
    .into_iter()
    .next();

  // A program is stood only on a login that answers now, never on the machine's own in place of one.
  if walked.is_none()
    && !claude_accounts::carried_for_user(&ground.db, JOB_WORK_KEY, None)
      .await
      .map_err(|refused| fault(refused.error))?
      .is_empty()
  {
    return Err(fault(format!("{worker}: no login answers now, so no program is stood")));
  }
  let model = crate::control::tiers::resolve_execution(&ground.db, JOB_WORK_KEY, "claude-code", None)
    .await
    .map_err(fault)?
    .model;
  let held = resumable_session(ground, worker, &model, &recorded(walked.as_ref())).await?;
  let account = match &held {
    None => walked.clone(),
    Some((_, account, _)) => claude_accounts::accounts(&ground.db)
      .await
      .map_err(|refused| fault(refused.error))?
      .into_iter()
      .find(|one| &one.id == account),
  };
  let mut line = configured(
    ground,
    machine,
    worker_run(ground, &trees, Some(worker), &model, cwd).await?,
    "claude-code",
  )
  .await?;

  if let Some((_, _, fallen)) = &held {
    line.model = session_launch_model(ground, worker, fallen.clone(), line.model).await?;
  }

  let place = Place::of(&trees.data, worker);
  let settings = write_place_settings(
    &place.path,
    read_settings(&line.settings),
    &status_root(ground),
    Holds {
      tasks: true,
      wording: true,
      compose_typed: RUN_LINES.composes_typed_lines(LINE),
      shell_guard: false,
    },
  )?;
  let session = match &held {
    Some((id, _, _)) => Session::Resume(id.clone()),
    None => Session::Open(
      Standby::opening(&place.path, &line.model, &recorded(account.as_ref()))
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
    ),
  };
  let command = Command {
    args: claude::worker_args(&line, &settings, Some(&session))?,
    cwd: line.cwd.clone(),
    env: claude::command_env(
      ground.env(),
      account.as_ref(),
      &claude::Runs::of_place(&trees.data, &place.path),
      line.thinking,
    )?,
  };
  let mut env = claude::line_environment(&command, &format!("bmo-standby-{worker}"));

  env.extend(worker_variables());
  let program = resident_of(machine, &place.path, &command.cwd, &command.args, &env, session.id());
  let standby = match (&session, program.opened.clone().flatten()) {
    (Session::Open(id), Some(record)) => Some(Standby {
      identity: program.identity.clone(),
      session_id: id.clone(),
      model: line.model.clone(),
      account: recorded(account.as_ref()),
      record,
      prefix_digest: Some(worker_prefix_digest(ground, &trees, cwd, &line.model, None).await?),
    }),
    _ => None,
  };
  let on = match &session {
    Session::Resume(id) => format!("session {id}"),
    Session::Open(_) => "a fresh session".to_owned(),
  };

  Ok(Standing {
    program,
    standby,
    on,
  })
}

/// The tree the place's status verb is run out of: the deployed one where it holds the verbs' binary, the
/// running code's otherwise.
pub fn status_root(ground: &Ground) -> PathBuf {
  if ground.roots.deploy.join(super::place::STATUS_BINARY).exists() {
    ground.roots.deploy.clone()
  } else {
    ground.code_root()
  }
}

/// The worker's line on a pinned provider: the job key's mapping there, or, for a pin naming one of the
/// provider's own models, that model on its own defaults unless the key's tier maps to it.
async fn pinned_run(
  ground: &Ground,
  machine: &Machine,
  trees: &Trees,
  worker: &str,
  cwd: &str,
  provider: &str,
  model: Option<&str>,
) -> Result<(WorkerRun, Option<i64>), Refusal> {
  let base = worker_run(ground, trees, Some(worker), WORKER_MODEL, cwd).await?;
  let Some(model) = model else {
    return Ok((configured(ground, machine, base, provider).await?, None));
  };
  let mapping = crate::control::tiers::resolve_execution(&ground.db, JOB_WORK_KEY, provider, None)
    .await
    .ok();
  let (tier, mapped, answer_tokens) = match mapping {
    Some(mapping) if mapping.model == model => (Some(mapping.tier), mapping.effort, mapping.answer_tokens),
    other => (other.map(|mapping| mapping.tier).or_else(|| Some(String::new())), None, None),
  };
  let mut run = base;

  run.effort = effort_of(ground, machine, provider, model, mapped.as_deref(), run.reasoning.as_ref())
    .await?
    .unwrap_or_default();
  run.model = model.to_owned();
  run.tier = tier;

  Ok((run, answer_tokens))
}

/// The registry's Claude Code on the gateway for a worker pinned to it: the worker's line on the
/// registration, placed on one of its nodes where it has several, with the place's settings and its
/// home's first-run questions answered, on a session it opens itself.
pub async fn registered_program(
  ground: &Ground,
  machine: &Machine,
  worker: &str,
  cwd: &str,
  model: Option<&str>,
) -> Result<Standing, Refusal> {
  let trees = Trees::of(ground);
  let (run, answer_tokens) = pinned_run(ground, machine, &trees, worker, cwd, "registered", model).await?;
  let (command, held) =
    registered_command(ground, machine, worker, &run, answer_tokens, Placing::Composed, None).await?;

  Ok(Standing {
    program: Program {
      identity: command_digest(&command),
      command,
      restart: None,
      opened: None,
    },
    standby: None,
    on: held,
  })
}

/// Where a run's session on a registration stands as its command is composed.
pub enum Placing {
  /// Placed as the command is composed, after the place's settings are written: a program stood with
  /// nothing put in, whose picker names the registration alone.
  Composed,
  /// Placed before, on the node named or on none in particular, which the picker names too.
  At(Option<String>),
}

/// Claude Code on the gateway for a run of the worker's line on a registration: the line on the
/// registration's name, on its node where it stands on one, with the place's settings and its home's
/// first-run questions answered, on the session named or one the program opens itself; the command, and
/// the registration's name.
pub async fn registered_command(
  ground: &Ground,
  machine: &Machine,
  worker: &str,
  run: &WorkerRun,
  answer_tokens: Option<i64>,
  placing: Placing,
  session: Option<&Session>,
) -> Result<(agent_host::place::ResidentCommand, String), Refusal> {
  let sampling = worker_sampling(ground, worker)
    .await?
    .map(|sampling| serde_json::to_string(&sampling))
    .transpose()
    .map_err(|why| fault(why.to_string()))?;
  let on = OnGateway {
    agent: worker,
    model: &run.model,
    effort: &run.effort,
    cwd: &run.cwd,
    settings: read_settings(&run.settings),
    compose_typed: RUN_LINES.composes_typed_lines(LINE),
    shell_guard: false,
    sampling,
  };

  gateway_command(ground, machine, on, answer_tokens, placing, |name, effort, thinking, settings| {
    let mut sent = run.clone();

    sent.model = name.to_owned();
    sent.effort = effort.to_owned();
    sent.thinking = thinking;

    claude::worker_args(&sent, settings, session)
  })
  .await
}

/// A line's run on a registered model's road, as its place stands it: the registration it names, the effort
/// its thinking is read from, the folder it works in, the settings its place is given, whether a person's
/// typed line is composed, whether its shell is guarded, and the sampling it asks the gateway for.
pub struct OnGateway<'a> {
  pub agent: &'a str,
  pub model: &'a str,
  pub effort: &'a str,
  pub cwd: &'a str,
  pub settings: serde_json::Map<String, Value>,
  pub compose_typed: bool,
  pub shell_guard: bool,
  pub sampling: Option<String>,
}

/// The command Claude Code stands on for a line on a registered model's road: the registration's home
/// settled, the place's settings with the model's row in the program's picker, the words the line makes of
/// the name the run is sent under and its thinking — the place's settings handed through the home's own —
/// and the gateway's variables; with the registration's name.
pub async fn gateway_command(
  ground: &Ground,
  machine: &Machine,
  on: OnGateway<'_>,
  answer_tokens: Option<i64>,
  placing: Placing,
  words: impl FnOnce(&str, &str, bool, &Path) -> Result<Vec<String>, Refusal>,
) -> Result<(agent_host::place::ResidentCommand, String), Refusal> {
  let trees = Trees::of(ground);
  let not_registered = || fault(format!("{}: {}", REGISTERED.presence.not_registered, on.model));
  let held = registry::registration(&ground.db, on.model)
    .await
    .map_err(|refused| fault(refused.error))?
    .filter(|held| !held.addresses.is_empty())
    .ok_or_else(not_registered)?;
  let home = Registered::home(machine, &held.name);
  let thinking = match on.effort {
    "thinking" => true,
    "no-thinking" => false,
    _ => held.thinking,
  };

  std::fs::create_dir_all(&home).map_err(|why| fault(format!("{}: {why}", home.display())))?;
  mark_onboarded(&config_file(&home));
  trust_folder(&config_file(&home), on.cwd);

  let place = Place::of(&trees.data, on.agent);
  let on_node = |node: &Option<String>| {
    node.as_ref().map_or_else(|| held.name.clone(), |node| format!("{}@{node}", held.name))
  };
  // The model's row in the program's picker, whose label the program shows at the top of its screen in
  // place of the name the run is sent under (docs/own-llm.md, 「Claude Code in the pane」): the run as it
  // is placed when the settings are written.
  let mut picked = on.settings;
  let shown = match &placing {
    Placing::Composed => held.name.clone(),
    Placing::At(node) => on_node(node),
  };

  picked.insert(
    "modelPicker".to_owned(),
    json!({ "options": [{ "model": shown, "label": held.label }] }),
  );

  let settings = write_place_settings(
    &place.path,
    picked,
    &status_root(ground),
    Holds {
      compose_typed: on.compose_typed,
      shell_guard: on.shell_guard,
      ..Holds::default()
    },
  )?;
  let placement = match placing {
    Placing::Composed => place_registration(ground, &held).await?,
    Placing::At(node) => node,
  };
  let name = on_node(&placement);
  let mut args = words(&name, if thinking { "medium" } else { "low" }, thinking, &settings)?;

  if let Some(at) = args.iter().position(|arg| arg == "--settings")
    && let Some(given) = args.get(at + 1).cloned()
  {
    args[at + 1] = keyed_settings(machine, &home, Some(&given))?.to_string_lossy().into_owned();
  }

  let wait_ms = held.wait_seconds * 1_000;
  let headers = gateway_headers(wait_ms, on.sampling.as_deref(), ClaudeCode.long_tools());
  let mut env: BTreeMap<String, Option<String>> =
    ground.env().iter().map(|(name, value)| (name.clone(), Some(value.clone()))).collect();

  env.extend(
    policy::providers::claude_code::CLAUDE_CODE
      .launch
      .variables
      .iter()
      .map(|(name, value)| (name.clone(), Some(value.clone()))),
  );

  for (variable, value) in
    gateway_variables(ground, &held, &home, &name, answer_tokens.unwrap_or(held.answer_tokens), &headers)
  {
    env.insert(variable.to_owned(), value);
  }

  if !thinking {
    env.insert("MAX_THINKING_TOKENS".to_owned(), Some("0".to_owned()));
  }

  let env: BTreeMap<String, String> =
    env.into_iter().filter_map(|(name, value)| Some((name, value?))).collect();
  let command = agent_host::place::ResidentCommand {
    args,
    cwd: PathBuf::from(on.cwd),
    env,
  };

  Ok((command, held.name))
}

/// Where a new session on the registration a model names stands, recorded as given now; nowhere in
/// particular on a registration of one address, or a model the registry does not hold.
pub async fn place_registered(ground: &Ground, model: &str) -> Result<Option<String>, Refusal> {
  match registry::registration(&ground.db, model)
    .await
    .map_err(|refused| fault(refused.error))?
  {
    Some(held) => place_registration(ground, &held).await,
    None => Ok(None),
  }
}

// Where a new session on a registration stands: on one of several addresses, the one given a new
// session least recently — an address never given one first, and the registration's own order among
// equals — recorded as given now; on a registration of one address, nowhere in particular.
async fn place_registration(
  ground: &Ground,
  held: &registry::Registration,
) -> Result<Option<String>, Refusal> {
  if held.addresses.len() < 2 {
    return Ok(None);
  }

  let rows = all(
    &ground.db,
    r#"SELECT "node", "placed_at" FROM "registered_model_addresses" WHERE "registration" = $1
       ORDER BY "position", "node""#,
    vec![held.name.as_str().into()],
  )
  .await?;
  let mut placed = Vec::with_capacity(rows.len());

  for row in rows {
    placed.push((
      row.try_get::<String>("", "node")?,
      row.try_get::<Option<i64>>("", "placed_at")?.unwrap_or(-1),
    ));
  }

  let Some((node, _)) = placed.iter().min_by_key(|(_, at)| *at).cloned() else {
    return Ok(None);
  };

  run(
    &ground.db,
    r#"UPDATE "registered_model_addresses" SET "placed_at" = $1 WHERE "registration" = $2 AND "node" = $3"#,
    vec![(now_ms() / 1000).into(), held.name.as_str().into(), node.as_str().into()],
  )
  .await?;

  Ok(Some(node))
}

/// The settings a run on the gateway is handed: those given, where a run is given any, with the helper that
/// prints the gateway's key beside them and the warning a program started in bypass mode stands on passed
/// over, written under the registration's home by what they were made from.
pub(crate) fn keyed_settings(
  machine: &Machine,
  home: &Path,
  given: Option<&str>,
) -> Result<PathBuf, Refusal> {
  let mut base = given
    .and_then(|given| std::fs::read_to_string(given).ok())
    .and_then(|text| serde_json::from_str::<Map<String, Value>>(&text).ok())
    .unwrap_or_default();
  let given = given.unwrap_or_default();
  let key = Registered::key_file(machine).to_string_lossy().replace('\'', r"'\''");

  base.insert("apiKeyHelper".to_owned(), Value::String(format!("cat '{key}'")));
  base.insert("skipDangerousModePermissionPrompt".to_owned(), Value::Bool(true));

  let dir = home.join("settings");
  let path = dir.join(format!("{}.json", &format!("{:x}", Sha256::digest(given.as_bytes()))[..16]));
  let text = serde_json::to_string_pretty(&base).map_err(|why| fault(why.to_string()))?;
  let fresh = dir.join(format!(
    ".{}.{}",
    path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default(),
    std::process::id()
  ));

  std::fs::create_dir_all(&dir).map_err(|why| fault(format!("{}: {why}", dir.display())))?;
  agent_host::disk::private_file(&fresh)
    .and_then(|mut file| std::io::Write::write_all(&mut file, format!("{text}\n").as_bytes()))
    .and_then(|()| std::fs::rename(&fresh, &path))
    .map_err(|why| fault(format!("{}: {why}", path.display())))?;

  Ok(path)
}

/// The headers every request of a run on the gateway carries: how long it is waited for, the run's own
/// sampling where it names any, and the tools of its program whose calls wait long where it names any.
pub(crate) fn gateway_headers(wait_ms: i64, sampling: Option<&str>, long_tools: &[String]) -> Vec<String> {
  let seconds = ((wait_ms + 999) / 1_000).max(1);
  let mut headers =
    vec![format!("x-litellm-timeout: {seconds}"), format!("x-litellm-stream-timeout: {seconds}")];

  if let Some(sampling) = sampling {
    headers.push(format!("x-bmo-sampling: {sampling}"));
  }

  if !long_tools.is_empty() {
    headers.push(format!("x-bmo-long-tools: {}", long_tools.join(",")));
  }

  headers
}

/// The share of a registration's window, in percent, a run on it is told to fold its conversation at: what
/// the answer ceiling leaves of the window, rounded down to a ten-thousandth, so a request at the fold
/// still carries the whole ceiling (docs/own-llm.md, 「A turn at the input limit」).
pub(crate) fn fold_share(window: i64, answer: i64) -> String {
  let millionths =
    ((window - answer).max(0) as i128 * 1_000_000 / window.max(1) as i128).clamp(1, 1_000_000);

  format!("{}.{:04}", millionths / 10_000, millionths % 10_000)
}

/// What Claude Code is pointed at the gateway with for a run on a registration, under the name the run is
/// sent as (docs/own-llm.md, 「The gateway」): no login of its own, the registration's home, the gateway's
/// address, the name in every model slot, the window and answer it is sized to and the fold that leaves the
/// answer room, and how long a request is waited for, by the program and by the runtime under it, with the
/// headers given.
pub(crate) fn gateway_variables(
  ground: &Ground,
  held: &registry::Registration,
  home: &Path,
  name: &str,
  answer_tokens: i64,
  headers: &[String],
) -> Vec<(&'static str, Option<String>)> {
  let wait_ms = held.wait_seconds * 1_000;

  vec![
    ("CLAUDE_CODE_OAUTH_TOKEN", None),
    ("ANTHROPIC_API_KEY", None),
    ("ANTHROPIC_AUTH_TOKEN", None),
    ("CLAUDE_CONFIG_DIR", Some(home.to_string_lossy().into_owned())),
    (
      "ANTHROPIC_BASE_URL",
      Some(harness_machine::gateway::url(
        ground.vars().get("BMO_LLM_GATEWAY_URL").map(String::as_str),
      )),
    ),
    ("ANTHROPIC_MODEL", Some(name.to_owned())),
    ("ANTHROPIC_DEFAULT_OPUS_MODEL", Some(name.to_owned())),
    ("ANTHROPIC_DEFAULT_SONNET_MODEL", Some(name.to_owned())),
    ("ANTHROPIC_DEFAULT_HAIKU_MODEL", Some(name.to_owned())),
    ("CLAUDE_CODE_SUBAGENT_MODEL", Some(name.to_owned())),
    ("CLAUDE_CODE_ATTRIBUTION_HEADER", Some("0".to_owned())),
    ("CLAUDE_CODE_MAX_CONTEXT_TOKENS", Some(held.context_tokens.to_string())),
    ("CLAUDE_CODE_MAX_OUTPUT_TOKENS", Some(answer_tokens.to_string())),
    ("CLAUDE_AUTOCOMPACT_PCT_OVERRIDE", Some(fold_share(held.context_tokens, answer_tokens))),
    ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", Some("1".to_owned())),
    ("API_TIMEOUT_MS", Some(wait_ms.to_string())),
    ("BUN_CONFIG_HTTP_IDLE_TIMEOUT", Some(((wait_ms + 999) / 1_000).to_string())),
    ("ANTHROPIC_CUSTOM_HEADERS", (!headers.is_empty()).then(|| headers.join("\n"))),
  ]
}

// The worker's own sampling as the gateway's header carries it, in the order it was saved: every
// setting but the answer ceiling, a setting the chat template reads under the template's own field;
// none where it names none.
pub(crate) async fn worker_sampling(
  ground: &Ground,
  worker: &str,
) -> Result<Option<indexmap::IndexMap<String, Value>>, Refusal> {
  let named = worker::documents_of(ground, worker).await?;
  let Some(text) = crate::queue::settings::get(&ground.db, &format!("agent_sampling:{named}")).await?
  else {
    return Ok(None);
  };
  let Ok(saved) = serde_json::from_str::<indexmap::IndexMap<String, Value>>(&text) else {
    return Ok(None);
  };
  let generation = &policy::companions::GENERATION;
  let mut sampling: indexmap::IndexMap<String, Value> = indexmap::IndexMap::new();
  let mut template: indexmap::IndexMap<String, Value> = indexmap::IndexMap::new();

  for (name, value) in saved {
    let usable = value.as_f64().is_some_and(f64::is_finite) || value.is_boolean();

    if !usable || name == "max_tokens" {
      continue;
    }

    match generation.setting(&name) {
      Some(setting) if setting.template => {
        template.insert(name, value);
      }
      _ if value.is_number() => {
        sampling.insert(name, value);
      }
      _ => {}
    }
  }

  if !template.is_empty() {
    sampling.insert(
      "chat_template_kwargs".to_owned(),
      serde_json::to_value(&template).map_err(|why| fault(why.to_string()))?,
    );
  }

  Ok((!sampling.is_empty()).then_some(sampling))
}

/// The run the deployment's loop is laid down on for a line typed at it, as the loop reads it: the worker's
/// run with no prompt, no variable and no place.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoopRun<'a> {
  line: &'a str,
  model: &'a str,
  effort: &'a str,
  system_prompt_path: Option<&'a Path>,
  tools_shown: Option<()>,
  tools_allowed: Option<()>,
  tools_denied: &'a str,
  mcp_config_path: &'a Path,
  prompt: &'a str,
  prompt_delivery: &'a str,
  resume_session_id: Option<()>,
  keep_session: bool,
  host_commands: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  reasoning: Option<LoopReasoning<'a>>,
  thinking: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  sampling: Option<indexmap::IndexMap<String, Value>>,
  answer_shape: &'a str,
  open_session_id: Option<()>,
  cwd: &'a str,
  env: Map<String, Value>,
  account: Option<()>,
  extensions: LoopExtensions<'a>,
  #[serde(skip_serializing_if = "Option::is_none")]
  tier: Option<&'a str>,
  resident: Option<()>,
}

#[derive(Serialize)]
struct LoopReasoning<'a> {
  thinking: Option<bool>,
  effort: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoopExtensions<'a> {
  settings_path: &'a Path,
  plugin_dir: &'a Path,
  agents_json: &'a str,
  rules_path: &'a Path,
}

/// The deployment's loop for a worker pinned to OpenRouter: the loop script started for the provider on the
/// worker's place, line and model, and the worker's run laid down as the one a line typed at it goes
/// out on, where no run of the bot's is laid down yet.
pub async fn loop_program(
  ground: &Ground,
  machine: &Machine,
  worker: &str,
  cwd: &str,
  provider: &str,
  script: &str,
) -> Result<Standing, Refusal> {
  let trees = Trees::of(ground);
  let (run, _) = pinned_run(ground, machine, &trees, worker, cwd, provider, None).await?;
  let place = Place::of(&trees.data, worker);
  let laid = place.path.join("loop-run.json");

  if !laid.exists() {
    let reasoning = run.reasoning.as_ref().map(|Reasoning { thinking, level }| LoopReasoning {
      thinking: *thinking,
      effort: level.as_deref(),
    });
    let laid_run = LoopRun {
      line: LINE,
      model: &run.model,
      effort: &run.effort,
      system_prompt_path: run.system_prompt.as_deref(),
      tools_shown: None,
      tools_allowed: None,
      tools_denied: &run.tools_denied,
      mcp_config_path: &run.mcp_config,
      prompt: "",
      prompt_delivery: "channel",
      resume_session_id: None,
      keep_session: true,
      host_commands: true,
      reasoning,
      thinking: run.thinking,
      sampling: worker_sampling(ground, worker).await?,
      answer_shape: "unread",
      open_session_id: None,
      cwd: &run.cwd,
      env: Map::new(),
      account: None,
      extensions: LoopExtensions {
        settings_path: &run.settings,
        plugin_dir: &run.plugin_dir,
        agents_json: &run.agents_json,
        rules_path: &run.rules,
      },
      tier: run.tier.as_deref(),
      resident: None,
    };
    let mut bytes = serde_json::to_vec(&laid_run).map_err(|why| fault(why.to_string()))?;

    bytes.push(b'\n');
    agent_host::disk::private_dir(&place.path)
      .map_err(|why| fault(format!("{}: {why}", place.path.display())))?;
    agent_host::disk::publish_bytes(&laid, &bytes)
      .map_err(|why| fault(format!("{}: {why}", laid.display())))?;
  }

  let mut env: BTreeMap<String, String> = ground.env().clone();

  env.extend(policy::providers::claude_code::CLAUDE_CODE.launch.variables.clone());

  let command = super::openrouter::loop_command(
    ground,
    script,
    provider,
    &place.path,
    LINE,
    &run.model,
    &run.cwd,
    &env,
  );

  Ok(Standing {
    program: Program {
      identity: command_digest(&command),
      command,
      restart: None,
      opened: None,
    },
    standby: None,
    on: run.model,
  })
}

/// A session a person opened in the standing program, taken as the worker's kept session on that
/// program's model and login. A session nothing has been said in is left standing, unless the program
/// that opened it is no longer the place's: then there is nothing to take and nothing to keep. Answers
/// the session taken.
pub async fn claim_standby(
  ground: &Ground,
  worker: &str,
  place: &Place,
) -> Result<Option<String>, Refusal> {
  let Some(standby) = Standby::read(&place.path) else { return Ok(None) };

  if !standby.record.exists() {
    if place.program().is_none_or(|program| program.identity != standby.identity) {
      Standby::drop_from(&place.path);
    }

    return Ok(None);
  }

  let replied = std::fs::metadata(&standby.record)
    .and_then(|meta| meta.modified())
    .ok()
    .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
    .map_or(0, |since| since.as_secs() as i64);

  run(
    &ground.db,
    r#"INSERT INTO "worker_sessions"
         ("worker", "model", "account", "claude_session_id", "last_reply_at", "prefix_digest", "answered_model")
       VALUES ($1, $2, $3, $4, $5, $6, NULL)
       ON CONFLICT ("worker", "model", "account") DO UPDATE SET
         "claude_session_id" = excluded."claude_session_id", "last_reply_at" = excluded."last_reply_at",
         "prefix_digest" = excluded."prefix_digest", "answered_model" = excluded."answered_model""#,
    vec![
      worker.into(),
      standby.model.as_str().into(),
      standby.account.as_str().into(),
      standby.session_id.as_str().into(),
      replied.into(),
      standby.prefix_digest.clone().into(),
    ],
  )
  .await?;
  Standby::drop_from(&place.path);

  Ok(Some(standby.session_id))
}
