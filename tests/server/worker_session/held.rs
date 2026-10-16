//! The session a worker holds: warm under its provider's lifetime, and set aside only where the worker's own
//! session turns to another model.

use std::path::PathBuf;

use agency_server::lines::worker::WorkerRun;
use agency_server::worker_session::held::*;

const CODEX: &str = "codex";

fn kept(mode: &str, model: &str, at: i64) -> Held {
  Held {
    mode: mode.to_owned(),
    model: model.to_owned(),
    at,
    ..Held::default()
  }
}

fn run_on(mode: &'static str, model: &str) -> SessionRun {
  SessionRun {
    mode,
    line: WorkerRun {
      model: model.to_owned(),
      effort: "high".to_owned(),
      tier: None,
      thinking: true,
      reasoning: None,
      system_prompt: None,
      rules: PathBuf::new(),
      mcp_config: PathBuf::new(),
      settings: PathBuf::new(),
      plugin_dir: PathBuf::new(),
      agents_json: String::new(),
      tools_denied: String::new(),
      cwd: "/work".to_owned(),
    },
    prompt: String::new(),
    resume: None,
    account: None,
    placement: None,
    thread: None,
  }
}

#[test]
fn a_session_is_warm_under_its_providers_lifetime_and_an_hour_where_none_is_declared() {
  let hour = 60 * 60_000;

  assert_eq!(cache_lifetime_ms(MODE, "model-a"), hour);
  assert_eq!(cache_lifetime_ms("no-such-provider", "model-a"), hour);
  assert!(warm(&kept(MODE, "model-a", 0), hour - 60_000));
  assert!(!warm(&kept(MODE, "model-a", 0), hour));
}

#[test]
fn only_the_workers_own_session_turned_to_another_model_is_set_aside() {
  assert!(turns_from(&kept(MODE, "model-a", 0), &run_on(MODE, "model-b")));
  assert!(!turns_from(&kept(MODE, "model-a", 0), &run_on(MODE, "model-a")));
  assert!(!turns_from(&kept(CODEX, "model-a", 0), &run_on(MODE, "model-b")));
  assert!(!turns_from(&kept(MODE, "model-a", 0), &run_on(CODEX, "model-b")));
}
