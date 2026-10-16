use std::collections::BTreeMap;

use agency_server::lines::claude::{COMPACT_SHARE, Command, Runs, command_env, line_environment};
use agency_server::lines::worker::*;
use policy::resident_turn::RESIDENT_TURN;

// The share a job worker's session compacts at inside a turn is the worker line's alone: every line's own
// variables leave it out, so a chat, an editor or a gate compacts where its program does by itself.
#[test]
fn the_compact_share_is_set_for_the_job_worker_alone() {
  let process = BTreeMap::from([("PATH".to_owned(), "/bin".to_owned())]);
  let command = Command {
    args: vec![],
    cwd: "/w".into(),
    env: command_env(&process, None, &Runs::Here, true).unwrap(),
  };

  assert!(!line_environment(&command, "bmo-any-line").contains_key(COMPACT_SHARE));
  // The worker's session is held to its model's window and compacts at the fold ratio of it.
  assert_eq!(RESIDENT_TURN.fold_ratio, 0.85);
  assert_eq!(worker_variables(), [(COMPACT_SHARE.to_owned(), "85.0000".to_owned())]);
}
