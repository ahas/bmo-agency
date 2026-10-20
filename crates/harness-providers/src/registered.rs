//! The registered models: present on this machine where the gateway's key file stands private under
//! the data root. Which registrations there are, and whether one has an address, is the archive's, read
//! by each side from its own (docs/provider-presence.md).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use policy::providers::Declared;
use policy::providers::registered::REGISTERED;

use crate::claude_code::ClaudeCode;
use crate::{Machine, Provider};

pub struct Registered;

impl Registered {
  /// Where the gateway's key file stands.
  pub fn key_file(machine: &Machine) -> PathBuf {
    machine.data_root().join("env").join(&REGISTERED.presence.key_file)
  }

  /// The home Claude Code is handed for a run on one registration, under the data root so a session
  /// resumed across a deployment finds its record (docs/own-llm.md, 「How requests go」).
  pub fn home(machine: &Machine, registration: &str) -> PathBuf {
    Self::homes(machine).join(registration)
  }

  /// The folder every registration's home stands under.
  pub fn homes(machine: &Machine) -> PathBuf {
    machine.data_root().join("claude-roads").join(Self.name())
  }
}

/// Whether an environment stands a program on a registration's road: its records home is a registration's,
/// or its base URL is agency's gateway (docs/agent-control.md, 「The registered models through Claude Code
/// on the gateway」). Any other endpoint of its own — a Claude account's, such as a DeepSeek one — leaves the
/// program on the Claude Code road.
pub fn registered_road(machine: &Machine, env: &BTreeMap<String, String>) -> bool {
  let homes = format!("{}/", Registered::homes(machine).display());
  let under_home = env.get("CLAUDE_CONFIG_DIR").is_some_and(|dir| dir.starts_with(&homes));
  let at_gateway = env.get("ANTHROPIC_BASE_URL").is_some_and(|url| {
    let url = url.trim_end_matches('/');

    !url.is_empty() && url == harness_machine::gateway::url(machine.var("BMO_LLM_GATEWAY_URL"))
  });

  under_home || at_gateway
}

/// Whether a secret file may be used: a file of its own, private to its owner, not empty.
fn usable_secret(path: &std::path::Path) -> bool {
  use std::os::unix::fs::PermissionsExt;

  std::fs::metadata(path)
    .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o777 == 0o600 && meta.len() > 0)
}

impl Provider for Registered {
  fn name(&self) -> &'static str {
    "registered"
  }

  fn declared(&self) -> &'static Declared {
    &REGISTERED.declared
  }

  // No registered model publishes a cache lifetime.
  fn place_cache(&self, _model: &str, _upstream: Option<&str>) -> Result<crate::PlaceCache, String> {
    Err(
      "A registered model publishes no cache lifetime, so there is no clock to keep it warm by.".to_owned(),
    )
  }

  fn missing(&self, machine: &Machine) -> Option<String> {
    (!usable_secret(&Self::key_file(machine))).then(|| REGISTERED.presence.missing.clone())
  }

  fn unregistered(&self) -> Option<&'static str> {
    Some(&REGISTERED.presence.no_address)
  }

  fn not_registered(&self) -> Option<&'static str> {
    Some(&REGISTERED.presence.not_registered)
  }

  // Claude Code on a registration's road: its configuration under one of the road's homes, or its base URL
  // at the gateway.
  fn owns_program(&self, machine: &Machine, args: &[String], env: &BTreeMap<String, String>) -> bool {
    args.first().is_some_and(|program| program == ClaudeCode::PROGRAM) && registered_road(machine, env)
  }

  // A registration takes no level; whether it thinks is the agent's switch, where it names one.
  fn run_effort(
    &self,
    mapped: Option<&str>,
    thinking: Option<bool>,
    level: Option<&str>,
    levels: &[String],
  ) -> Option<String> {
    let _ = (level, levels);

    match thinking {
      None => mapped.map(str::to_owned),
      Some(true) => Some("thinking".to_owned()),
      Some(false) => Some("no-thinking".to_owned()),
    }
  }

  // A run on a registration that carries tools is Claude Code on the gateway, whose events name its session as
  // Claude Code's do.
  fn session_of_event(&self, event: &serde_json::Value) -> Option<String> {
    ClaudeCode.session_of_event(event)
  }

  // Two kinds of run print in the host: Claude Code on the gateway, whose typed events are read as Claude Code's
  // are, and a request with no tool asked directly, which prints its answer as it stands and is shown so.
  fn progress_of(&self, line: &str) -> Option<String> {
    let trimmed = line.trim();

    if trimmed.is_empty() {
      return None;
    }

    let typed = trimmed.starts_with('{')
      && serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .is_some_and(|event| event.get("type").is_some_and(serde_json::Value::is_string));

    if typed { ClaudeCode.progress_of(line) } else { Some(line.to_owned()) }
  }

  // A run on a registration is Claude Code under the registration's own home.
  fn session_records(&self, machine: &Machine, cwd: &Path, session: &str, model: &str) -> Vec<PathBuf> {
    ClaudeCode::records_under(&Self::home(machine, model), cwd, session)
  }

  // Every registration keeps a home of its own, and a session may have run on any of them.
  fn home_records(&self, machine: &Machine, cwd: &Path) -> Vec<PathBuf> {
    let Ok(homes) = std::fs::read_dir(Self::homes(machine)) else {
      return vec![];
    };

    homes
      .flatten()
      .flat_map(|home| ClaudeCode::every_record_under(&home.path(), cwd))
      .collect()
  }

  // Every registration keeps a history of its own, as it keeps its records.
  fn forget_prompts(&self, machine: &Machine, cwd: &Path, session: Option<&str>) -> Result<usize, String> {
    let Ok(homes) = std::fs::read_dir(Self::homes(machine)) else {
      return Ok(0);
    };
    let mut forgotten = 0;
    let mut refused = vec![];

    for home in homes.flatten() {
      match ClaudeCode::forget_prompts_under(&home.path(), cwd, session) {
        Ok(count) => forgotten += count,
        Err(why) => refused.push(why),
      }
    }

    if refused.is_empty() { Ok(forgotten) } else { Err(refused.join("; ")) }
  }
}
