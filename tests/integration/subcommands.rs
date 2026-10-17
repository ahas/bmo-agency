//! The verbs as a shell reaches them: each subcommand of `bmo-agency` the deployment, the hooks and the panes
//! call, hidden ones among them, read by the binary's own command line, and the status line's shell guard
//! answered through it.

use std::io::Write as _;
use std::process::{Command, Output, Stdio};

use crate::binary;

/// Every subcommand path the deployment, an agent's settings, a hook or a pane runs.
const SUBCOMMANDS: &[&[&str]] = &[
  &["agent"],
  &["node", "claude-token"],
  &["node", "pane"],
  &["node", "work"],
  &["dev-server", "start"],
  &["dev-server", "restart"],
  &["dev-server", "stop"],
  &["dev-server", "list"],
  &["dev-server", "pane"],
  &["worker", "fold"],
  &["model"],
  &["reload"],
  &["companion", "follow"],
  &["coworker", "pool"],
  &["coworker", "hook"],
  &["claude", "batch"],
  &["live-claude"],
  &["prompts"],
  &["dev-pane"],
  &["node-pane"],
  &["work-node"],
];

// The binary run on `args` in a home and a data root of the test's own, `input` on its standard input.
fn run(args: &[&str], input: &str) -> Output {
  let home = tempfile::tempdir().unwrap();
  let mut child = Command::new(binary::built("agency-server", "bmo-agency"))
    .args(args)
    .current_dir(home.path())
    .env("HOME", home.path())
    .env("BMO_DATA_DIR", home.path().join("data"))
    .env_remove("BMO_ARCHIVE_URL")
    .env_remove("CLAUDECODE")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();

  child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
  child.wait_with_output().unwrap()
}

#[test]
fn every_subcommand_a_caller_runs_is_read_by_the_binary() {
  let mut unread = Vec::new();

  for path in SUBCOMMANDS {
    let mut args = path.to_vec();

    args.push("--help");

    let output = run(&args, "");
    let said = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() || !said.contains("Usage:") {
      unread.push(format!("{}: {}", path.join(" "), String::from_utf8_lossy(&output.stderr)));
    }
  }

  assert!(unread.is_empty(), "subcommands the binary does not read: {unread:#?}");
}

#[test]
fn the_status_lines_shell_guard_refuses_a_launch_of_claude_code_and_lets_another_command_through() {
  let place = tempfile::tempdir().unwrap();
  let at = place.path().to_string_lossy().into_owned();
  let shell = ["agent-status", "shell", "--place", at.as_str()];
  let refused = run(&shell, r#"{"tool_name":"Bash","tool_input":{"command":"claude -p hello"}}"#);
  let let_through = run(&shell, r#"{"tool_name":"Bash","tool_input":{"command":"ls -la ."}}"#);

  assert!(refused.status.success());
  assert!(String::from_utf8_lossy(&refused.stdout).contains("deny"), "{refused:?}");
  assert!(let_through.status.success());
  assert_eq!(String::from_utf8_lossy(&let_through.stdout).trim(), "");
}

#[test]
fn a_subcommand_nobody_runs_is_refused() {
  let output = run(&["no-such-verb"], "");

  assert!(!output.status.success());
}
