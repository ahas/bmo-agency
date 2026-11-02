//! A run a procedure requires opening a job's way to a Claude Code session (docs/live-claude-guard.md,
//! 「Second layer: the guard in agency's code」). Alone in a process of its own, since the declaration and the
//! job's marks it is read against stand for the whole process.

use std::collections::BTreeMap;

use crate::integration::common::alone::alone;
use agency_server::door::guard::{declare_required_run, launch_refusal, prompt_refusal};

// "a declaration opens a job's launch"
#[test]
fn a_declaration_opens_a_jobs_launch() {
  let Some(_run) = alone(module_path!(), "a_declaration_opens_a_jobs_launch") else {
    return;
  };

  // SAFETY: the one test of its process, which starts no thread before the variables are set.
  unsafe {
    std::env::remove_var("BMO_TEST_HOME");
    std::env::set_var("CLAUDECODE", "1");
    std::env::set_var("BMO_TASK_LOCKED", "1");
  }

  let line = ["claude".to_owned(), "-p".to_owned()];

  assert!(launch_refusal("worker", &line, &BTreeMap::new()).is_some());
  assert!(prompt_refusal().is_some());

  declare_required_run("this test");

  assert_eq!(launch_refusal("worker", &line, &BTreeMap::new()), None);
  assert_eq!(prompt_refusal(), None);
}
