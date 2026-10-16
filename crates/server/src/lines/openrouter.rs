//! The worker's run on OpenRouter, as a job's walk sends it there (docs/cache-fallback.md, 「The OpenRouter
//! substitute is the dsh road」): how dsh takes the run, and the deployment's own loop that stands in the
//! worker's pane for a road whose program keeps no conversation of its own, running one dsh child a turn.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_host::place::ResidentCommand;

use super::place::is_turn_variable;
use super::stand_in::Taking;
use crate::places::Ground;

/// The provider the run goes out on.
pub const PROVIDER: &str = "openrouter";

/// How dsh takes a worker's run: the prompt as its argument, and no session kept.
pub const TAKING: Taking = Taking {
  prompt_delivery: "argument",
  keep_session: false,
};

/// The variable the address a turn's tap stands at is handed to dsh in, which travels with the turn rather
/// than the loop.
pub const LOCAL_URL: &str = "BMO_DSH_LOCAL_URL";

/// The command the deployment's loop stands on for a provider in an agent's place: the loop script under
/// the code root, on the agent's place, the line its runs go out on and the model, under the process's own
/// variables less any one turn's.
#[allow(clippy::too_many_arguments)]
pub fn loop_command(
  ground: &Ground,
  script: &str,
  provider: &str,
  place: &Path,
  line: &str,
  model: &str,
  cwd: &str,
  env: &BTreeMap<String, String>,
) -> ResidentCommand {
  let env = env
    .iter()
    .filter(|(name, _)| name.as_str() != LOCAL_URL && !is_turn_variable(name))
    .map(|(name, value)| (name.clone(), value.clone()))
    .collect();

  ResidentCommand {
    args: vec![
      ground.roots.bun.to_string_lossy().into_owned(),
      ground.code_root().join(script).to_string_lossy().into_owned(),
      "--provider".to_owned(),
      provider.to_owned(),
      "--place".to_owned(),
      place.to_string_lossy().into_owned(),
      "--line".to_owned(),
      line.to_owned(),
      "--model".to_owned(),
      model.to_owned(),
    ],
    cwd: PathBuf::from(cwd),
    env,
  }
}
