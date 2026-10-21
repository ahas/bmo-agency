//! DeepSeek's road (docs/cache-fallback.md, 「The DeepSeek road」): Claude Code on the Anthropic-compatible
//! endpoint the endpoint account holds, under a home of the model's own. A run's effort is the program's
//! own `--effort` word, and a run that does not think is sent with its thinking shut off.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use harness_providers::Machine;
use harness_providers::deepseek::DeepSeek;
use policy::providers::claude_code::CLAUDE_CODE;
use policy::providers::deepseek::DEEPSEEK;
use policy::run_lines::RUN_LINES;
use serde_json::{Map, Value, json};

use super::claude::{self, Session};
use super::place::{Holds, Place, Program, command_digest, read_settings, write_place_settings};
use super::standing::{Standing, fold_share, keyed_settings_at, pinned_run, status_root};
use super::worker::{LINE, WorkerRun};
use super::{Trees, fault};
use crate::Refusal;
use crate::places::Ground;
use crate::places::trust::{config_file, mark_onboarded, trust_folder};

/// The provider the road is, which every answer on it is filed under.
pub const PROVIDER: &str = "deepseek";

/// The word a mapping may name for a run that does not think.
const OFF: &str = "none";

/// Where a model of this provider keeps its configuration and records: under the data root, so a
/// session resumed across a deployment finds its record.
pub fn home(machine: &Machine, model: &str) -> PathBuf {
  DeepSeek::home(machine, model)
}

/// The model one id names, where this provider serves it.
pub fn model(id: &str) -> Result<&'static policy::providers::deepseek::Model, Refusal> {
  DeepSeek::model(id).ok_or_else(|| fault(format!("{}: {id}", DEEPSEEK.presence.not_held)))
}

/// Whether a run of this road thinks: its own switch, and no thinking where its effort names `none`.
pub fn thinks(effort: &str, thinking: bool) -> bool {
  thinking && effort != OFF
}

// The effort word Claude Code is handed: the run's own where it thinks, and the lowest the program
// takes where it does not.
fn program_effort(effort: &str, thinking: bool) -> String {
  if !thinking {
    return "low".to_owned();
  }

  if effort.is_empty() || effort == OFF { "high".to_owned() } else { effort.to_owned() }
}

/// A line's run on DeepSeek's road, as its place stands it: the model, the effort its thinking is read
/// from, whether it thinks, the folder it works in, and the settings its place is given.
pub struct OnEndpoint<'a> {
  pub agent: &'a str,
  pub model: &'a str,
  pub effort: &'a str,
  pub thinking: bool,
  pub cwd: &'a str,
  pub settings: Map<String, Value>,
  pub compose_typed: bool,
  pub shell_guard: bool,
}

/// The program a worker pinned to this provider stands with nothing put in: the worker's line on the
/// model it is pinned to, at its home, on a session it opens itself.
pub async fn deepseek_program(
  ground: &Ground,
  machine: &Machine,
  worker: &str,
  cwd: &str,
  model: Option<&str>,
) -> Result<Standing, Refusal> {
  let trees = Trees::of(ground);
  let (run, answer_tokens) = pinned_run(ground, machine, &trees, worker, cwd, PROVIDER, model).await?;
  let (command, held) = deepseek_command(ground, machine, worker, &run, answer_tokens, None).await?;

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

/// Claude Code on the endpoint for a run of the worker's line: the line on the model it names, at the
/// model's home, with the place's settings and its home's first-run questions answered, on the session
/// named or one the program opens itself.
pub async fn deepseek_command(
  ground: &Ground,
  machine: &Machine,
  worker: &str,
  run: &WorkerRun,
  answer_tokens: Option<i64>,
  session: Option<&Session>,
) -> Result<(agent_host::place::ResidentCommand, String), Refusal> {
  let on = OnEndpoint {
    agent: worker,
    model: &run.model,
    effort: &run.effort,
    thinking: run.thinking,
    cwd: &run.cwd,
    settings: read_settings(&run.settings),
    compose_typed: RUN_LINES.composes_typed_lines(LINE),
    shell_guard: false,
  };

  endpoint_command(ground, machine, on, answer_tokens, |name, effort, thinking, settings| {
    let mut sent = run.clone();

    sent.model = name.to_owned();
    sent.effort = effort.to_owned();
    sent.thinking = thinking;

    claude::worker_args(&sent, settings, session)
  })
  .await
}

/// The command a run of any line on the endpoint is: the model's home settled, the place's settings with
/// the model's row in the program's picker, the words the line makes of the model and its thinking, and
/// the endpoint's variables and the key file the program reads its key from; with the model's id.
pub async fn endpoint_command(
  ground: &Ground,
  machine: &Machine,
  on: OnEndpoint<'_>,
  answer_tokens: Option<i64>,
  words: impl FnOnce(&str, &str, bool, &Path) -> Result<Vec<String>, Refusal>,
) -> Result<(agent_host::place::ResidentCommand, String), Refusal> {
  let held = model(on.model)?;
  let thinking = thinks(on.effort, on.thinking);
  let home = home(machine, &held.id);
  let trees = Trees::of(ground);

  std::fs::create_dir_all(&home).map_err(|why| fault(format!("{}: {why}", home.display())))?;
  mark_onboarded(&config_file(&home));
  trust_folder(&config_file(&home), on.cwd);

  let place = Place::of(&trees.data, on.agent);
  let mut picked = on.settings;

  picked.insert(
    "modelPicker".to_owned(),
    json!({ "options": [{ "model": held.id, "label": held.label }] }),
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
  let args = words(&held.id, &program_effort(on.effort, thinking), thinking, &settings)?;
  let settings_path = settings.to_string_lossy().into_owned();
  let settings = keyed_settings_at(&home, Some(&settings_path), &DeepSeek::key_file(machine))?;
  let mut args = args;

  if let Some(at) = args.iter().position(|arg| arg == "--settings")
    && let Some(_given) = args.get(at + 1)
  {
    args[at + 1] = settings.to_string_lossy().into_owned();
  }

  let answer = answer_tokens.unwrap_or(held.answer_tokens);
  let wait_ms = DEEPSEEK.endpoint.wait_seconds * 1_000;
  let mut env: BTreeMap<String, Option<String>> =
    ground.env().iter().map(|(name, value)| (name.clone(), Some(value.clone()))).collect();

  env.extend(
    CLAUDE_CODE
      .launch
      .variables
      .iter()
      .map(|(name, value)| (name.clone(), Some(value.clone()))),
  );

  for (variable, value) in
    endpoint_variables(machine, &home, &held.id, held.context_tokens, answer, wait_ms)
  {
    env.insert(variable.to_owned(), value);
  }

  if !thinking {
    env.insert("MAX_THINKING_TOKENS".to_owned(), Some("0".to_owned()));
  }

  let env: BTreeMap<String, String> =
    env.into_iter().filter_map(|(name, value)| Some((name, value?))).collect();

  Ok((
    agent_host::place::ResidentCommand {
      args,
      cwd: PathBuf::from(on.cwd),
      env,
    },
    held.id.clone(),
  ))
}

/// What Claude Code is pointed at DeepSeek with for a run of one model: no login of its own, the model's
/// home, the endpoint's address, the model in every model slot, the window and answer it is sized to and
/// the fold that leaves the answer room, and how long a request is waited for by the program and by the
/// runtime under it.
fn endpoint_variables(
  machine: &Machine,
  home: &Path,
  model: &str,
  context_tokens: i64,
  answer_tokens: i64,
  wait_ms: i64,
) -> Vec<(&'static str, Option<String>)> {
  vec![
    ("CLAUDE_CODE_OAUTH_TOKEN", None),
    ("ANTHROPIC_API_KEY", None),
    ("ANTHROPIC_AUTH_TOKEN", None),
    ("CLAUDE_CONFIG_DIR", Some(home.to_string_lossy().into_owned())),
    ("ANTHROPIC_BASE_URL", Some(DeepSeek::base_url(machine))),
    ("ANTHROPIC_MODEL", Some(model.to_owned())),
    ("ANTHROPIC_DEFAULT_OPUS_MODEL", Some(model.to_owned())),
    ("ANTHROPIC_DEFAULT_SONNET_MODEL", Some(model.to_owned())),
    ("ANTHROPIC_DEFAULT_HAIKU_MODEL", Some(model.to_owned())),
    ("CLAUDE_CODE_SUBAGENT_MODEL", Some(model.to_owned())),
    ("CLAUDE_CODE_ATTRIBUTION_HEADER", Some("0".to_owned())),
    ("CLAUDE_CODE_MAX_CONTEXT_TOKENS", Some(context_tokens.to_string())),
    ("CLAUDE_CODE_MAX_OUTPUT_TOKENS", Some(answer_tokens.to_string())),
    (claude::COMPACT_SHARE, Some(fold_share(context_tokens, answer_tokens))),
    ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", Some("1".to_owned())),
    ("API_TIMEOUT_MS", Some(wait_ms.to_string())),
    ("BUN_CONFIG_HTTP_IDLE_TIMEOUT", Some(((wait_ms + 999) / 1_000).to_string())),
  ]
}
