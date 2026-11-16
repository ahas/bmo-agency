//! What a command on a node that failed is said as (docs/person-nodes.md, 「The workplace on a PC」).

use agency_server::verbs::work_node_setup::{NO_CODE, ShellRun};

#[test]
fn a_failed_command_says_its_code_and_its_errors_or_its_output_where_its_errors_hold_nothing() {
  let run = |code: i32, stdout: &str, stderr: &str| ShellRun {
    code,
    stdout: stdout.to_owned(),
    stderr: stderr.to_owned(),
  };

  assert_eq!(run(2, "out", "err\n").said(), "it ended with code 2: err");
  // WSL's launcher, failing before any command runs, writes why on its output alone.
  assert_eq!(
    run(-1073741819, "Catastrophic failure\n", "").said(),
    "it ended with code -1073741819: Catastrophic failure"
  );
  assert_eq!(
    run(1, " ", "").said(),
    "it ended with code 1: nothing was written on its output or its errors"
  );
  // A command that came to no code of its own is its errors alone.
  assert_eq!(run(NO_CODE, "", "the hub: no agent attached").said(), "the hub: no agent attached");
}
