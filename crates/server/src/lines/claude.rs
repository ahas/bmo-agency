//! Claude Code as a resident program of the worker line (docs/launch-lines.md; docs/agent-control.md,
//! 「The interactive surface a provider declares」): its words, its environment, and the environment
//! the pane it stands in is handed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use harness_providers::claude_code::ClaudeCode;
use policy::providers::claude_code::CLAUDE_CODE;

use super::fault;
use super::worker::{LINE, WorkerRun};
use crate::Refusal;
use crate::control::claude_accounts::Account;
use crate::watches::links::place_node;

// The variable a run is told its own name in, and the job runner's marker.
const RUN_NAME: &str = "BMO_RUN_NAME";
const JOB_RUNNER_MARKER: &str = "BMO_TASK_LOCKED";

/// Which session a program opens or continues.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Session {
  Resume(String),
  Open(String),
}

impl Session {
  pub fn id(&self) -> &str {
    match self {
      Self::Resume(id) | Self::Open(id) => id,
    }
  }
}

/// A command as a line builds it, before the pane's environment is made of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
  pub args: Vec<String>,
  pub cwd: String,
  /// Every variable the program is started with, an unset one among them as none.
  pub env: BTreeMap<String, Option<String>>,
}

/// A path the worker line hands the program, and never one that is not there.
pub fn handed_path(path: &Path) -> Result<String, Refusal> {
  handed(LINE, path)
}

/// A path a line hands the program, and never one that is not there: a program handed a path that is
/// not on the disk exits before it has read a word of the prompt.
pub fn handed(line: &str, path: &Path) -> Result<String, Refusal> {
  if path.exists() {
    return Ok(path.to_string_lossy().into_owned());
  }

  Err(fault(format!(
    "The {line} line hands the CLI {}, which is missing. Without it the CLI exits with code 1 before it \
     reads a word of the prompt, and the run's record says only that no answer came. This happens when the \
     deploy tree lags the checkout: deploy, then run again.",
    path.display()
  )))
}

/// The worker line's words as a person starts the program: no print mode, the flags of the line in
/// their fixed order, and the session it opens or continues last. `settings` is the settings the
/// program is handed, its place's for a resident program.
pub fn worker_args(
  run: &WorkerRun,
  settings: &Path,
  session: Option<&Session>,
) -> Result<Vec<String>, Refusal> {
  let mut args = vec![
    ClaudeCode::PROGRAM.to_owned(),
    "--model".to_owned(),
    run.model.clone(),
    "--effort".to_owned(),
    run.effort.clone(),
    "--permission-mode".to_owned(),
    "bypassPermissions".to_owned(),
    "--disallowedTools".to_owned(),
    run.tools_denied.clone(),
  ];

  match &run.system_prompt {
    None => args.extend(["--append-system-prompt-file".to_owned(), handed_path(&run.rules)?]),
    Some(prompt) => args.extend(["--system-prompt-file".to_owned(), handed_path(prompt)?]),
  }

  args.extend([
    "--setting-sources".to_owned(),
    "project,local".to_owned(),
    "--settings".to_owned(),
    handed_path(settings)?,
    "--mcp-config".to_owned(),
    handed_path(&run.mcp_config)?,
    "--plugin-dir".to_owned(),
    handed_path(&run.plugin_dir)?,
    "--agents".to_owned(),
    run.agents_json.clone(),
  ]);

  match session {
    Some(Session::Resume(id)) => args.extend(["--resume".to_owned(), id.clone()]),
    Some(Session::Open(id)) => args.extend(["--session-id".to_owned(), id.clone()]),
    None => {}
  }

  Ok(args)
}

/// A run of a line Claude Code runs apart from the bot — an editor's batch, a research run — as its words
/// are made of it (docs/agents.md, 「Runs apart」): its model and effort, the instructions standing in
/// place of the CLI's own, the one MCP configuration attached, the tools it is shown and allowed, and
/// whether the CLI's own commands are reachable.
pub struct ApartRun<'a> {
  pub line: &'a str,
  pub model: &'a str,
  pub effort: &'a str,
  pub system_prompt: Option<&'a Path>,
  pub mcp_config: Option<&'a Path>,
  pub tools_shown: &'a str,
  pub tools_allowed: &'a str,
  pub host_commands: bool,
}

/// The words of a run apart, the interactive program opening the session named, which no run resumes: no
/// settings sources, the strict MCP flag beside the one configuration, no Chrome, and don't-ask mode over
/// what it is shown.
pub fn apart_args(run: &ApartRun, session: &str) -> Result<Vec<String>, Refusal> {
  let mut args = vec![ClaudeCode::PROGRAM.to_owned()];

  args.extend(["--model", run.model, "--effort", run.effort, "--setting-sources", ""].map(str::to_owned));

  if let Some(prompt) = run.system_prompt {
    args.extend(["--system-prompt-file".to_owned(), handed(run.line, prompt)?]);
  }

  args.push("--strict-mcp-config".to_owned());

  if let Some(config) = run.mcp_config {
    args.extend(["--mcp-config".to_owned(), handed(run.line, config)?]);
  }

  if !run.host_commands {
    args.push("--disable-slash-commands".to_owned());
  }

  args.extend(
    [
      "--no-chrome",
      "--permission-mode",
      "dontAsk",
      "--tools",
      run.tools_shown,
      "--allowedTools",
      run.tools_allowed,
      "--session-id",
      session,
    ]
    .map(str::to_owned),
  );

  Ok(args)
}

/// Where a program goes out from, as its login is handed to it: this machine, whose logins are the
/// accounts' own, or a work node, which is handed each account's token for the nodes, kept under the
/// data root named (docs/work-nodes.md, 「The logins on a node」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Runs {
  Here,
  OnNode { data_root: PathBuf },
}

impl Runs {
  /// Where a program standing in a place goes out from: the node the place is marked with, or here.
  pub fn of_place(data_root: &Path, place: &Path) -> Self {
    match place_node(place) {
      Some(_) => Self::OnNode {
        data_root: data_root.to_path_buf(),
      },
      None => Self::Here,
    }
  }
}

/// The login as the program is handed it: a directory account names its configuration home and clears
/// any token, or on a node hands its token for the nodes beside it where this machine holds one; a token
/// account hands the token read from its file.
pub fn account_env(
  account: Option<&Account>,
  runs: &Runs,
) -> Result<BTreeMap<String, Option<String>>, Refusal> {
  let Some(account) = account else { return Ok(BTreeMap::new()) };

  if account.credential_kind == "token" {
    let token = std::fs::read_to_string(&account.credential_path)
      .map_err(|_| fault(format!("The token file of account {} cannot be read.", account.id)))?;

    return Ok(BTreeMap::from([("CLAUDE_CODE_OAUTH_TOKEN".to_owned(), Some(token.trim().to_owned()))]));
  }

  let token = match runs {
    Runs::Here => None,
    Runs::OnNode { data_root } => account.node_token(data_root),
  };

  Ok(BTreeMap::from([
    ("CLAUDE_CONFIG_DIR".to_owned(), Some(account.credential_path.clone())),
    ("CLAUDE_CODE_OAUTH_TOKEN".to_owned(), token),
  ]))
}

/// The program's environment: the launching process's with the line's own variables, the login's over
/// it as handed where the program runs, and thinking turned off where the run thinks not.
pub fn command_env(
  process: &BTreeMap<String, String>,
  account: Option<&Account>,
  runs: &Runs,
  thinking: bool,
) -> Result<BTreeMap<String, Option<String>>, Refusal> {
  let mut env: BTreeMap<String, Option<String>> =
    process.iter().map(|(name, value)| (name.clone(), Some(value.clone()))).collect();

  for (name, value) in &CLAUDE_CODE.launch.variables {
    env.insert(name.clone(), Some(value.clone()));
  }

  env.extend(account_env(account, runs)?);

  if !thinking {
    env.insert("MAX_THINKING_TOKENS".to_owned(), Some("0".to_owned()));
  }

  Ok(env)
}

/// The window, in tokens, Claude Code compacts a session against inside a turn where it is set under its
/// model's own.
pub const COMPACT_WINDOW: &str = "CLAUDE_CODE_AUTO_COMPACT_WINDOW";

/// What a pane running the line's own command is handed, by name: the search path, every `BMO_`
/// variable but the job runner's marker, the marker where the launcher holds it, the line's own
/// variables and the login's, and the run's name last.
pub fn line_environment(command: &Command, name: &str) -> BTreeMap<String, String> {
  let set = |variable: &str| command.env.get(variable).cloned().flatten().filter(|value| !value.is_empty());
  let mut env =
    BTreeMap::from([("PATH".to_owned(), command.env.get("PATH").cloned().flatten().unwrap_or_default())]);

  for (variable, value) in &command.env {
    if let Some(value) = value
      && variable.starts_with("BMO_")
      && variable != JOB_RUNNER_MARKER
    {
      env.insert(variable.clone(), value.clone());
    }
  }

  if let Some(marker) = set(JOB_RUNNER_MARKER) {
    env.insert(JOB_RUNNER_MARKER.to_owned(), marker);
  }

  env.extend(CLAUDE_CODE.launch.variables.clone());

  for variable in &CLAUDE_CODE.launch.login_variables {
    if let Some(value) = set(variable) {
      env.insert(variable.clone(), value);
    }
  }

  env.insert(RUN_NAME.to_owned(), name.to_owned());
  env
}
