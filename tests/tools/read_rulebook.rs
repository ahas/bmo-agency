//! The rulebook reader: a rulebook the run was handed read whole out of the tree, one it was not refused with
//! the ones it was, and arguments that do not fit refused unread; and its executable answering a call as the
//! tool host hands it one, placed in a tree of its own.

use std::io::Write;
use std::process::{Command, Stdio};

use read_rulebook_tool::{call, read};
use serde_json::{Value, json};
use tool_call::Session;

use crate::binary;

fn reader() -> Session {
  serde_json::from_value(json!({
    "line": "search",
    "capabilities": ["web.rulebooks"],
    "dataRoot": "/nowhere",
    "archive": "",
    "research": { "closesAt": 1, "rulebooks": ["restaurant-search"] },
  }))
  .unwrap()
}

// A tree whose rules folder holds one rulebook, `restaurant-search`.
fn tree() -> tempfile::TempDir {
  let tree = tempfile::tempdir().unwrap();
  let rules = tree.path().join("prompts").join("rules");

  std::fs::create_dir_all(&rules).unwrap();
  std::fs::write(rules.join("restaurant-search.md"), "# Restaurant search\n").unwrap();
  std::fs::write(rules.join("search-kinds.md"), "# Search kinds\n").unwrap();
  tree
}

#[test]
fn a_rulebook_the_run_was_handed_is_read_whole_out_of_the_tree() {
  let tree = tree();
  let answer = read(&reader(), "restaurant-search", tree.path());

  assert!(!answer.is_error);
  assert_eq!(answer.plain(), "RULEBOOK prompts/rules/restaurant-search.md:\n# Restaurant search\n");
}

#[test]
fn a_rulebook_the_run_was_not_handed_is_refused_with_the_ones_it_was_and_bad_arguments_unread() {
  let tree = tree();
  let other = call(&reader(), json!({ "name": "search-kinds" }), tree.path()).unwrap();

  assert!(other.is_error);
  assert_eq!(
    other.plain(),
    "ERROR: search-kinds is not one of this run's rulebooks: restaurant-search"
  );
  assert!(call(&reader(), json!({ "name": "" }), tree.path()).is_err());
  assert!(call(&reader(), json!({}), tree.path()).is_err());

  let unread = read(&reader(), "restaurant-search", &tree.path().join("elsewhere"));

  assert!(unread.is_error && unread.plain().contains("could not be read"), "{}", unread.plain());
}

// The executable started in the tree's placed folder for the tool, handed `request`, and what it wrote.
fn asked(tree: &std::path::Path, request: &Value) -> (bool, Value) {
  let placed = tree.join("tools").join("read_rulebook").join(".placed");

  std::fs::create_dir_all(&placed).unwrap();

  let mut child = Command::new(binary::built("read-rulebook-tool", "read-rulebook-tool"))
    .current_dir(&placed)
    .env_clear()
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .unwrap();

  child.stdin.take().unwrap().write_all(request.to_string().as_bytes()).unwrap();

  let ended = child.wait_with_output().unwrap();

  (ended.status.success(), serde_json::from_slice(&ended.stdout).unwrap_or(Value::Null))
}

#[test]
fn the_executable_answers_a_call_out_of_the_tree_it_is_placed_in() {
  let tree = tree();
  let session = serde_json::to_value(json!({
    "line": "search",
    "capabilities": ["web.rulebooks"],
    "dataRoot": "/nowhere",
    "archive": "",
    "research": { "rulebooks": ["restaurant-search"] },
  }))
  .unwrap();
  let call = |entry: &str, arguments: Value| json!({ "protocol": 1, "kind": "call", "entry": entry, "arguments": arguments, "session": session, "callback": "/run/c.sock" });

  assert_eq!(
    asked(tree.path(), &call("web/read_rulebook", json!({ "name": "restaurant-search" }))),
    (
      true,
      json!({ "content": [{ "type": "text", "text": "RULEBOOK prompts/rules/restaurant-search.md:\n# Restaurant search\n" }], "isError": false })
    )
  );

  let (ended, said) = asked(tree.path(), &call("web/read_rulebook", json!({ "name": "" })));

  assert!(ended);
  assert_eq!(said["isError"], true);
  assert!(
    said["content"][0]["text"]
      .as_str()
      .unwrap()
      .starts_with("ERROR: invalid arguments: read_rulebook")
  );

  let (ended, said) = asked(tree.path(), &call("web/other", json!({})));

  assert!(ended);
  assert_eq!(said["content"][0]["text"], "ERROR: this executable holds no entry web/other");
  assert!(!asked(tree.path(), &json!({ "protocol": 2, "kind": "resources_list", "session": session })).0);
}
