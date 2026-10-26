//! Whether a shell command reaches another unit's store (docs/agents.md, 「No agent reaches another unit's
//! store」): runs a database client, names a database address, reaches a loopback port a unit answers on, or
//! reads the token files under the data root's `env/`. The words are read the way the shell splits them, its
//! substitutions and `-c` scripts among them; the addresses, ports and paths anywhere in the text, a heredoc's
//! body included, since a script handed to an interpreter reaches them as well.

use std::path::Path;
use std::sync::LazyLock;

use policy::shell_guard::SHELL_GUARD;
use regex::Regex;

use super::shell::{self, program_of, shell_script};
use crate::lines::token::TOKEN_VARIABLE;

// A loopback host and the port after it, by a colon or a space.
static LOOPBACK: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(?i)(?:127\.0\.0\.1|localhost|0\.0\.0\.0|\[?::1\]?)(?::|\s+)(\d{2,5})\b")
    .expect("the loopback pattern")
});

// The data root's token folder, however the command names the data root.
static TOKEN_DIR: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(?:\$\{?BMO_DATA_DIR\}?|\.local/share/bmo/data)/env(?:/|\b)")
    .expect("the token folder pattern")
});

/// Why the command reaches another unit's store, or nothing.
pub fn store_reach(command: &str, data_root: &Path) -> Option<String> {
  let lowered = command.to_lowercase();

  if let Some(scheme) = SHELL_GUARD.schemes.iter().find(|scheme| lowered.contains(scheme.as_str())) {
    return Some(format!("it names a {scheme} address"));
  }

  if let Some(program) = client_run(command, 0) {
    return Some(format!("it runs {program}"));
  }

  for found in LOOPBACK.captures_iter(command) {
    if let Some(port) = found.get(1).and_then(|port| port.as_str().parse::<u16>().ok())
      && SHELL_GUARD.guards_port(port)
    {
      return Some(format!("it reaches {}", found.get(0).map_or("", |all| all.as_str())));
    }
  }

  if command.contains(TOKEN_VARIABLE) {
    return Some("it handles the session's token".to_owned());
  }

  let own_tokens = format!("{}/env/", data_root.to_string_lossy().trim_end_matches('/'));

  if command.contains(&own_tokens) || TOKEN_DIR.is_match(command) {
    return Some("it reads the deployment's token files".to_owned());
  }

  None
}

// The database client a command runs, in its simple commands, its substitutions and the scripts its shells are
// handed, or none.
fn client_run(command: &str, depth: usize) -> Option<String> {
  if depth > 4 {
    return None;
  }

  let split = shell::split(command);

  for simple in &split.commands {
    let Some(run) = program_of(&simple.words) else { continue };

    if SHELL_GUARD.programs.contains(&run.program) {
      return Some(run.program);
    }

    if let Some(script) = shell_script(&run)
      && let Some(found) = client_run(&script, depth + 1)
    {
      return Some(found);
    }
  }

  split.inner.iter().find_map(|inner| client_run(&inner.text, depth + 1))
}

/// The refusal a command reaching another unit's store is answered with.
pub fn store_refusal(reason: &str) -> String {
  format!("refused: {reason}. {}", SHELL_GUARD.road)
}
