//! Who may put words into a pane's Claude Code session through this door (docs/live-claude-guard.md,
//! 「Second layer: the guard in agency's code」). The door is agency's own and runs in its service,
//! which carries neither of a job's two marks; a test reaches only a stand-in herdr under its own home
//! or the temporary directory; and a job's own process is refused, since the opening a job makes for
//! itself is read by the TypeScript guard (the agent package's `live-claude-guard.ts`) alone, but for a run
//! a procedure requires, which its verb declares first. Claude Code switched off closes every turn on a
//! Claude Code program the door would put in, and none on a program standing on a registration's road.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use harness_providers::Machine;
use harness_providers::registered::registered_road;
use policy::door_words::DOOR_WORDS;
use sea_orm::DatabaseConnection;

use crate::control::harness::provider_enabled;

const TEST_HOME: &str = "BMO_TEST_HOME";
const SESSION_MARK: &str = "CLAUDECODE";
const JOB_RUNNER_MARKER: &str = "BMO_TASK_LOCKED";

// A run a procedure requires, said by the verb that makes it before it makes it.
static DECLARED: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// A run a procedure requires — the fold a prefix change asks for after a deployment — said by the verb
/// that makes it before it makes it, which lets it through from inside a job as the TypeScript guard lets a
/// declared run through. The files that may say so are held by a test (docs/live-claude-guard.md).
pub fn declare_required_run(purpose: &str) {
  let _ = DECLARED.set(purpose.to_owned());
}

fn declared() -> bool {
  DECLARED.get().is_some()
}

fn set(name: &str) -> Option<String> {
  std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn within(path: &Path, dir: &Path) -> bool {
  let absolute = |path: &Path| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());

  absolute(path).starts_with(absolute(dir))
}

/// Why this process may not reach the herdr at `socket`, or nothing.
pub fn socket_refusal(socket: &Path) -> Option<String> {
  let home = PathBuf::from(set(TEST_HOME)?);

  (!within(socket, &home) && !within(socket, &std::env::temp_dir())).then(|| {
    format!(
      "a test was about to reach the herdr at {}; a test reaches only a stand-in under its own home or the temporary directory (docs/live-claude-guard.md)",
      socket.display()
    )
  })
}

/// Why this process may not put words into a pane's Claude Code session, or nothing.
pub fn prompt_refusal() -> Option<String> {
  if set(TEST_HOME).is_some()
    || set(SESSION_MARK).is_none()
    || set(JOB_RUNNER_MARKER).is_none()
    || declared()
  {
    return None;
  }

  Some(
    "refused: this process runs inside a job's Claude Code session, and agency's server's resident door puts no job's words into a pane (docs/live-claude-guard.md)"
      .to_owned(),
  )
}

// The lines whose program, Claude Code, opens a session that writes a whole prefix to the cache.
const SESSION_LINES: [&str; 5] = ["worker", "chat", "search", "browse", "pane"];

// The programs a launch is judged by: Claude Code itself, and herdr, which stands one in a pane.
const GUARDED: [&str; 2] = ["claude", "herdr"];

fn basename(path: &str) -> &str {
  path.rsplit('/').next().unwrap_or(path)
}

// Where a program is found on a search path, as a shell finds it: the first executable file of that name.
fn which(program: &str, path: &str) -> Option<PathBuf> {
  use std::os::unix::fs::PermissionsExt;

  if program.contains('/') {
    return Some(PathBuf::from(program));
  }

  path
    .split(':')
    .filter(|dir| !dir.is_empty())
    .map(|dir| Path::new(dir).join(program))
    .find(|found| {
      std::fs::metadata(found).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    })
}

/// Why this process may not start this command on a line, or nothing: a test starts only a stand-in under
/// its own home or the temporary directory, never the machine's own program, and a job's own process opens
/// no Claude Code session on a line that keeps one, since the opening a job makes for itself is read by the
/// TypeScript guard alone.
pub fn launch_refusal(line: &str, args: &[String], env: &BTreeMap<String, String>) -> Option<String> {
  let program = args.iter().find(|part| GUARDED.contains(&basename(part)))?;

  if let Some(home) = set(TEST_HOME) {
    let path = env.get("PATH").cloned().or_else(|| std::env::var("PATH").ok()).unwrap_or_default();
    let found = which(program, &path)?;

    if within(&found, Path::new(&home)) || within(&found, &std::env::temp_dir()) {
      return None;
    }

    return Some(format!(
      "a test was about to start {}, the machine's own {}; a test starts only the stand-in in its own home or a fake of its own (docs/live-claude-guard.md)",
      found.display(),
      basename(program)
    ));
  }

  if basename(program) != GUARDED[0]
    || !SESSION_LINES.contains(&line)
    || set(SESSION_MARK).is_none()
    || set(JOB_RUNNER_MARKER).is_none()
    || declared()
  {
    return None;
  }

  Some(format!(
    "refused: this process runs inside a job's Claude Code session, and agency's server starts no Claude Code session on the {line} line for a job (docs/live-claude-guard.md)"
  ))
}

/// Why a turn on a program of this environment may not go in now, or nothing.
pub async fn turn_refusal(
  db: &DatabaseConnection,
  machine: &Machine,
  work_key: &str,
  env: &BTreeMap<String, String>,
) -> Option<String> {
  if !registered_road(machine, env) && !provider_enabled(db, "claude-code").await.unwrap_or(false) {
    return Some(format!(
      "{work_key}: selected claude-code route did not answer: {}",
      DOOR_WORDS.provider_off
    ));
  }

  prompt_refusal()
}
