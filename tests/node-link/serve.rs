//! The far end on the node: what it watches, sends and writes.

use std::fs;

use node_link::frame::{Identity, Pull};
use node_link::serve::*;

#[test]
fn a_tree_names_every_file_under_it() {
  let dir = tempfile::tempdir().expect("dir");

  fs::create_dir_all(dir.path().join("a/b")).expect("tree");
  fs::write(dir.path().join("a/b/one.jsonl"), "x").expect("one");
  fs::write(dir.path().join("two.jsonl"), "y").expect("two");

  let mut tree = files_of(&Pull {
    remote: dir.path().to_string_lossy().into(),
    files: vec![],
    tree: true,
    append: true,
    alive: false,
  });
  let flat = files_of(&Pull {
    remote: dir.path().to_string_lossy().into(),
    files: vec![],
    tree: false,
    append: true,
    alive: false,
  });

  tree.sort();
  assert_eq!(tree, vec!["a/b/one.jsonl".to_owned(), "two.jsonl".to_owned()]);
  assert_eq!(flat, vec!["two.jsonl".to_owned()]);
}

#[test]
fn only_a_live_process_of_the_record_is_told_alive() {
  let dir = tempfile::tempdir().expect("dir");
  let own = std::process::id() as i32;
  let birth = process_birth(own).expect("own birth");

  fs::write(
    dir.path().join("running.json"),
    serde_json::json!({ "pid": own, "birth": birth, "loop": { "pid": own, "birth": "0" } }).to_string(),
  )
  .expect("record");

  assert_eq!(alive_in(dir.path()), vec![Identity { pid: own, birth }]);
}
