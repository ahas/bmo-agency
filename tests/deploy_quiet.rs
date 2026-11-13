//! The declared pre-flip quiet, `deploy/quiet`, run over a stand-in receiver (docs/sessions.md, 「A restart
//! hands the session over」): the receiver's 0 handed on, its 1 left with the lines that name no chat turn,
//! and any other code it ends with handed on as it is, with what it printed.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// How long the script, which runs one stand-in that ends at once, is given before it fails the test.
const BOUND: Duration = Duration::from_secs(10);

// The script run with a home whose deployed receiver prints `lines` and ends with `code`: the script's code
// and what it printed.
fn quiet_over(lines: &[&str], code: i32) -> (i32, String) {
  let home = tempfile::tempdir().unwrap();
  let bin = home.path().join(".local/share/bmo-receiver/deploy/bin");
  std::fs::create_dir_all(&bin).unwrap();
  let receiver = bin.join("bmo-receiver");
  let printed: String = lines.iter().map(|line| format!("echo '{line}'\n")).collect();
  std::fs::write(&receiver, format!("#!/bin/sh\n{printed}exit {code}\n")).unwrap();
  std::fs::set_permissions(&receiver, std::fs::Permissions::from_mode(0o755)).unwrap();

  run(home.path())
}

fn run(home: &Path) -> (i32, String) {
  let script = concat!(env!("CARGO_MANIFEST_DIR"), "/deploy/quiet");
  let mut child = Command::new("sh")
    .arg(script)
    .env_clear()
    .env("PATH", std::env::var("PATH").unwrap_or_default())
    .env("HOME", home)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .spawn()
    .unwrap();
  let deadline = Instant::now() + BOUND;

  while child.try_wait().unwrap().is_none() {
    if Instant::now() > deadline {
      child.kill().unwrap();
      child.wait().unwrap();
      panic!("the quiet did not end");
    }
    std::thread::sleep(Duration::from_millis(5));
  }

  let done = child.wait_with_output().unwrap();

  (done.status.code().unwrap_or(-1), String::from_utf8_lossy(&done.stdout).into_owned())
}

#[test]
fn work_under_way_stands_only_where_a_line_names_something_other_than_a_chat_turn() {
  assert_eq!(quiet_over(&["room:chat:7"], 1), (0, String::new()));
  assert_eq!(quiet_over(&["room:chat:7", "room:job:9"], 1), (1, "room:job:9\n".to_owned()));
}

#[test]
fn a_receiver_that_could_not_tell_is_handed_on_with_its_own_code_and_all_it_printed() {
  assert_eq!(
    quiet_over(&["room:chat:7", "store unreadable"], 2),
    (2, "room:chat:7\nstore unreadable\n".to_owned())
  );
  assert_eq!(quiet_over(&[], 3), (3, String::new()));
}
