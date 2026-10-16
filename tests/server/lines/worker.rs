use std::collections::BTreeMap;

use agency_server::lines::claude::{COMPACT_WINDOW, Command, Runs, command_env, line_environment};
use agency_server::lines::worker::*;

// The window a job worker's session is held to is the worker line's alone: every line's own variables
// leave it out, so a chat, an editor or a gate compacts against its model's window as before.
#[test]
fn the_compact_window_is_set_for_the_job_worker_alone() {
  let process = BTreeMap::from([("PATH".to_owned(), "/bin".to_owned())]);
  let command = Command {
    args: vec![],
    cwd: "/w".into(),
    env: command_env(&process, None, &Runs::Here, true).unwrap(),
  };

  assert!(!line_environment(&command, "bmo-any-line").contains_key(COMPACT_WINDOW));
  assert_eq!(
    worker_variables(),
    [(COMPACT_WINDOW.to_owned(), RESIDENT_TURN.worker_window_tokens.to_string())]
  );
  // The program takes no window under a hundred thousand tokens, so a smaller figure would not hold.
  assert!(RESIDENT_TURN.worker_window_tokens >= 100_000);
}
