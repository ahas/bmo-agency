//! The worker's run on OpenRouter, as a job's walk sends it there (docs/cache-fallback.md, 「The OpenRouter
//! substitute is the dsh road」): how dsh takes the run; and the resident pane program every provider whose
//! turns go through dsh stands — OpenRouter's and DeepSeek's alike — which keeps a dsh session on the Agent
//! Client Protocol, its turns put in through its inbox and read back out of its record.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_host::place::ResidentCommand;
use harness_providers::Machine;

use super::dsh::{self, Composed};
use super::place::is_turn_variable;
use super::stand_in::Taking;
use crate::Refusal;
use crate::places::Ground;

/// The provider the run goes out on.
pub const PROVIDER: &str = "openrouter";

/// How dsh takes a worker's run on its headless profile: the prompt as its argument, and no session kept.
pub const TAKING: Taking = Taking {
  prompt_delivery: "argument",
  keep_session: false,
};

/// The variable the address a turn's tap stands at is handed to a headless dsh in, which travels with the
/// turn rather than the loop.
pub const LOCAL_URL: &str = "BMO_DSH_LOCAL_URL";

/// The program a provider declares for its pane, as this machine stands it: the path its PATH finds, or
/// the name itself where it finds none.
fn program_word(machine: &Machine, program: &str) -> String {
  machine
    .program(program)
    .map_or_else(|| program.to_owned(), |path| path.to_string_lossy().into_owned())
}

/// The model as dsh's session lists it: the route it stands on, `[provider, model]`.
pub fn choice(provider: &str, model: &str) -> String {
  serde_json::json!([provider, model]).to_string()
}

/// The command a provider's pane program stands on in an agent's place: bmo-tui on its `dsh` subcommand,
/// the harness at `--profile acp`, the agent's place, the line's model choice and the overlay dsh reads,
/// under the process's own variables less any one turn's, with the key the provider's dsh route reads from
/// the data root's env directory put into the program's environment alone.
#[allow(clippy::too_many_arguments)]
pub fn resident_command(
  ground: &Ground,
  machine: &Machine,
  provider: &str,
  program: &str,
  dsh_program: &str,
  place: &Path,
  agent: &str,
  run: &Composed<'_>,
  env: &BTreeMap<String, String>,
) -> Result<ResidentCommand, Refusal> {
  let mut env = env
    .iter()
    .filter(|(name, _)| name.as_str() != LOCAL_URL && !is_turn_variable(name))
    .map(|(name, value)| (name.clone(), value.clone()))
    .collect::<BTreeMap<_, _>>();

  // dsh keeps its profiles and sessions under the data root, which a deployment lays nothing over,
  // rather than in anybody's home or a tree a deployment rebuilds.
  env.insert("DSH_HOME".to_owned(), dsh::dsh_home(&ground.roots.data).to_string_lossy().into_owned());

  // dsh pairs the overlay's sandbox with whether a run stops to ask and refuses a pair it names no preset
  // for, so the session is handed the mode as the one name it resolves both ends of, as a headless run is.
  match run.permission_mode() {
    Some(mode) => env.insert("DSH_PERMISSION_MODE".to_owned(), mode.to_owned()),
    None => env.remove("DSH_PERMISSION_MODE"),
  };

  let route = harness_providers::provider(provider)
    .and_then(|declared| declared.declared().dsh.as_ref())
    .ok_or_else(|| super::fault(format!("{provider} names no dsh route")))?;

  // A route whose key stands in a file of the deployment's own is handed it as the variable the route
  // reads; a route read from the process's own variables already has it.
  if let Some(file) = route.key_file.as_deref() {
    let path = ground.roots.data.join("env").join(file);

    if let Ok(key) = std::fs::read_to_string(&path) {
      env.insert(route.key_env.clone(), key.trim().to_owned());
    }
  }
  let patch = dsh::session_patch(run, &env, &ground.roots.data, route)?;
  let mut args = vec![
    program_word(machine, program),
    "dsh".to_owned(),
    "--program".to_owned(),
    program_word(machine, dsh_program),
    "--cwd".to_owned(),
    run.cwd.to_owned(),
    "--place".to_owned(),
    place.to_string_lossy().into_owned(),
    "--agent".to_owned(),
    agent.to_owned(),
    "--model".to_owned(),
    choice(provider, run.model),
  ];

  if !run.effort.is_empty() {
    args.push("--effort".to_owned());
    args.push(run.effort.to_owned());
  }

  // The agency's own MCP servers, read from the same configuration the Claude Code line is handed: the
  // session opens with them, and dsh's own tools stand beside them, its web tools off in the overlay so the
  // deployment's web and site tools are the web the model reaches.
  if let Some(mcp) = run.mcp {
    args.push("--mcp-config".to_owned());
    args.push(mcp.to_string_lossy().into_owned());
  }

  args.push("--patch".to_owned());
  args.push(patch.to_string_lossy().into_owned());

  Ok(ResidentCommand {
    args,
    cwd: PathBuf::from(run.cwd),
    env,
  })
}
