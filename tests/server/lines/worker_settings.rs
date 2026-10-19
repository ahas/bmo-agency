//! The worker settings' guard hook: the job guard the deployment names, run with the hook's input on
//! standard input and answered with its output, and nothing judged where none is named or it fails.

use std::io::Write as _;
use std::process::{Command, Stdio};

use serde_json::Value;

// The command the worker settings' first `PreToolUse` hook runs.
fn guard_hook() -> String {
  let path = concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/agents/worker/settings.json");
  let settings: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

  settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap().to_owned()
}

// The hook run as the program runs it, with the variables named and `input` on standard input: its code
// and what it printed.
fn run(variables: &[(&str, &str)], input: &str) -> (i32, String) {
  let mut child = Command::new("sh")
    .arg("-c")
    .arg(guard_hook())
    .env_clear()
    .env("PATH", std::env::var("PATH").unwrap_or_default())
    .envs(variables.iter().copied())
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .unwrap();

  // A hook that judges nothing ends without reading its input, so the input may meet a pipe already closed.
  if let Err(why) = child.stdin.take().unwrap().write_all(input.as_bytes()) {
    assert_eq!(why.kind(), std::io::ErrorKind::BrokenPipe, "{why}");
  }

  let done = child.wait_with_output().unwrap();

  (done.status.code().unwrap_or(-1), String::from_utf8_lossy(&done.stdout).into_owned())
}

#[test]
fn the_hook_hands_the_call_to_the_guard_the_deployment_names_and_answers_with_its_word() {
  let answered = run(&[("BMO_SOURCE_DIR", "/checkout"), ("BMO_JOB_GUARD", "tr a-z A-Z")], "deny\n");

  assert_eq!(answered, (0, "DENY\n".to_owned()));
}

#[test]
fn with_no_guard_named_no_checkout_or_a_failing_guard_nothing_is_judged() {
  assert_eq!(run(&[("BMO_SOURCE_DIR", "/checkout")], "x"), (0, String::new()));
  assert_eq!(run(&[("BMO_JOB_GUARD", "echo judged")], "x"), (0, String::new()));
  assert_eq!(
    run(&[("BMO_SOURCE_DIR", "/checkout"), ("BMO_JOB_GUARD", "exit 3")], "x"),
    (0, String::new())
  );
}
