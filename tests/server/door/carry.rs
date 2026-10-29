//! A session's record carried into the home the turn's program reads it under (docs/sessions.md, 「Session
//! replacement」): a road that moved continues the session it had, its transcript and the folder beside it
//! copied whole; nothing is copied where the home already holds the record or where no home holds it.

use std::path::{Path, PathBuf};

use agency_server::door::turn::carry_record;
use harness_providers::claude_code::ClaudeCode;

fn home(data: &Path, road: &str, member: &str) -> PathBuf {
  data.join("claude-roads").join(road).join(member)
}

// The record a session leaves under a home, with the folder of its name beside it.
fn planted(records: &Path, cwd: &Path, session: &str) -> PathBuf {
  let folder = ClaudeCode::project_folder(records, cwd);

  std::fs::create_dir_all(folder.join(session)).unwrap();

  let record = folder.join(format!("{session}.jsonl"));

  std::fs::write(&record, "{\"type\":\"user\"}\n").unwrap();
  std::fs::write(folder.join(session).join("child.txt"), "a child's transcript").unwrap();

  record
}

#[test]
fn a_record_standing_in_another_roads_home_is_carried_into_the_one_the_turn_reads() {
  let data = tempfile::tempdir().unwrap();
  let cwd = tempfile::tempdir().unwrap();
  let machine = tempfile::tempdir().unwrap();
  let source = home(data.path(), "deepseek", "deepseek-flash");
  let target = home(data.path(), "registered", "flash-next");
  let record = planted(&source, cwd.path(), "s-1");

  assert!(carry_record(data.path(), machine.path(), &target, cwd.path(), "s-1"));

  let carried = ClaudeCode::project_folder(&target, cwd.path()).join("s-1.jsonl");

  assert_eq!(std::fs::read_to_string(&carried).unwrap(), std::fs::read_to_string(&record).unwrap());
  assert_eq!(
    std::fs::read_to_string(ClaudeCode::project_folder(&target, cwd.path()).join("s-1/child.txt")).unwrap(),
    "a child's transcript",
    "the folder beside the transcript comes with it"
  );
  assert!(
    !carry_record(data.path(), machine.path(), &target, cwd.path(), "s-1"),
    "nothing to carry again"
  );
}

#[test]
fn nothing_is_carried_for_a_session_no_home_holds() {
  let data = tempfile::tempdir().unwrap();
  let cwd = tempfile::tempdir().unwrap();
  let machine = tempfile::tempdir().unwrap();
  let source = home(data.path(), "deepseek", "deepseek-flash");
  let target = home(data.path(), "registered", "flash-next");

  planted(&source, cwd.path(), "s-1");

  assert!(!carry_record(data.path(), machine.path(), &target, cwd.path(), "s-2"));
  assert!(!ClaudeCode::project_folder(&target, cwd.path()).join("s-2.jsonl").exists());
}
