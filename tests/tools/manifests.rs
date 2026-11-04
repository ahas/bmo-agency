//! Each tool's manifest held to the tool it declares: its name its folder's, the protocol agency's tools speak,
//! its executable the crate's own binary, and what it declares what the executable answers.

use std::path::{Path, PathBuf};

use toml::Value;

fn tools() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("tools")
}

fn read(path: &Path) -> Value {
  toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn text<'a>(value: &'a Value, path: &[&str]) -> &'a str {
  path.iter().fold(value, |at, key| &at[*key]).as_str().unwrap()
}

fn words(value: &Value) -> Vec<&str> {
  value.as_array().unwrap().iter().map(|word| word.as_str().unwrap()).collect()
}

#[test]
fn every_folder_under_tools_is_a_tool_named_after_it_whose_executable_is_its_crates_binary() {
  let mut folders: Vec<String> = std::fs::read_dir(tools())
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();

  folders.sort();

  assert_eq!(folders, ["delegate", "procedures", "relay", "worker_ask"]);

  for folder in &folders {
    let manifest = read(&tools().join(folder).join("tool.toml"));
    let crate_manifest = read(&tools().join(folder).join("Cargo.toml"));
    let package = text(&crate_manifest, &["package", "name"]);
    let binary = text(&crate_manifest["bin"][0], &["name"]);

    assert_eq!(text(&manifest, &["tool", "name"]), folder);
    assert_eq!(manifest["tool"]["protocol"].as_integer(), Some(i64::from(tool_call::PROTOCOL)));
    assert_eq!(text(&manifest, &["tool", "language"]), "rust");
    assert_eq!(words(&manifest["tool"]["run"]), [binary]);
    // A call is waited on until the executable answers: no manifest names a timeout or an unbounded entry.
    assert!(manifest["tool"].get("timeout").is_none(), "{folder}");

    for entry in manifest.get("entry").and_then(Value::as_array).into_iter().flatten() {
      assert!(entry.get("timeout").is_none(), "{folder}");
      assert!(entry.get("unbounded").is_none(), "{folder}");
    }
    assert!(text(&manifest, &["tool", "build"]).contains(&format!("-p {package} ")), "{folder}");
    assert!(
      text(&manifest, &["tool", "build"]).contains(&format!("/release/{binary}\"")),
      "{folder}"
    );
  }
}

#[test]
fn the_procedures_declare_their_resources_and_no_entry() {
  let manifest = read(&tools().join("procedures").join("tool.toml"));

  assert!(manifest.get("entry").is_none());
  assert_eq!(text(&manifest, &["resources", "namespace"]), "bmo");
  assert_eq!(words(&manifest["resources"]["capabilities"]), ["room.tools"]);
  assert!(procedures_tool::PROCEDURE_PREFIX.starts_with("bmo://"));
}

#[test]
fn the_worker_ask_entry_is_declared_as_its_tool_answers_it() {
  let folder = tools().join("worker_ask");
  let manifest = read(&folder.join("tool.toml"));
  let entry = &manifest["entry"].as_array().unwrap()[0];

  assert_eq!(text(entry, &["namespace"]), worker_ask_tool::NAMESPACE);
  assert_eq!(text(entry, &["name"]), worker_ask_tool::NAME);
  assert_eq!(text(entry, &["title"]), worker_ask_tool::TITLE);
  assert_eq!(text(entry, &["description"]), worker_ask_tool::DESCRIPTION);
  assert_eq!(words(&entry["capabilities"]), worker_ask_tool::CAPABILITIES);

  let schema: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(folder.join(text(entry, &["input"]))).unwrap()).unwrap();

  assert_eq!(schema["required"], serde_json::json!(["words"]));
  assert!(schema["properties"].get("timeBoundSeconds").is_none(), "the ask takes no time bound");
}

#[test]
fn the_delegate_entry_is_declared_as_its_tool_answers_it() {
  let folder = tools().join("delegate");
  let manifest = read(&folder.join("tool.toml"));
  let entry = &manifest["entry"].as_array().unwrap()[0];

  assert_eq!(text(entry, &["namespace"]), delegate_tool::NAMESPACE);
  assert_eq!(text(entry, &["name"]), delegate_tool::NAME);
  assert_eq!(text(entry, &["title"]), delegate_tool::TITLE);
  assert_eq!(text(entry, &["description"]), delegate_tool::DESCRIPTION);
  assert_eq!(words(&entry["capabilities"]), delegate_tool::CAPABILITIES);

  let schema: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(folder.join(text(entry, &["input"]))).unwrap()).unwrap();

  assert_eq!(schema["required"], serde_json::json!(["title", "instruction"]));
  assert_eq!(
    schema["properties"].as_object().unwrap().keys().collect::<Vec<_>>(),
    ["title", "instruction", "role", "folder"]
  );
}

#[test]
fn the_delegations_entry_is_declared_as_its_tool_answers_it() {
  let folder = tools().join("delegate");
  let manifest = read(&folder.join("tool.toml"));
  let entry = &manifest["entry"].as_array().unwrap()[1];

  assert_eq!(text(entry, &["namespace"]), delegate_tool::NAMESPACE);
  assert_eq!(text(entry, &["name"]), delegate_tool::LIST_NAME);
  assert_eq!(text(entry, &["title"]), delegate_tool::LIST_TITLE);
  assert_eq!(text(entry, &["description"]), delegate_tool::LIST_DESCRIPTION);
  assert_eq!(words(&entry["capabilities"]), delegate_tool::CAPABILITIES);

  let schema: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(folder.join(text(entry, &["input"]))).unwrap()).unwrap();

  assert!(schema.get("required").is_none());
  assert_eq!(schema["properties"].as_object().unwrap().keys().collect::<Vec<_>>(), ["ended"]);
}

#[test]
fn the_relay_entries_are_declared_as_its_tool_answers_them() {
  let folder = tools().join("relay");
  let manifest = read(&folder.join("tool.toml"));
  let entries = manifest["entry"].as_array().unwrap();

  assert_eq!(entries.len(), 2);

  let relay = &entries[0];

  assert_eq!(text(relay, &["namespace"]), relay_tool::NAMESPACE);
  assert_eq!(text(relay, &["name"]), relay_tool::NAME);
  assert_eq!(text(relay, &["title"]), relay_tool::TITLE);
  assert_eq!(text(relay, &["description"]), relay_tool::DESCRIPTION);
  assert_eq!(words(&relay["capabilities"]), relay_tool::CAPABILITIES);

  let schema: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(folder.join(text(relay, &["input"]))).unwrap()).unwrap();

  assert_eq!(schema["required"], serde_json::json!(["to", "text"]));
  assert!(schema["properties"].get("from").is_none(), "the sender is never typed");

  let chat = &entries[1];

  assert_eq!(text(chat, &["namespace"]), relay_tool::NAMESPACE);
  assert_eq!(text(chat, &["name"]), relay_tool::CHAT_NAME);
  assert_eq!(text(chat, &["title"]), relay_tool::CHAT_TITLE);
  assert_eq!(text(chat, &["description"]), relay_tool::CHAT_DESCRIPTION);
  assert_eq!(words(&chat["capabilities"]), relay_tool::CAPABILITIES);

  let schema: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(folder.join(text(chat, &["input"]))).unwrap()).unwrap();

  assert_eq!(schema["required"], serde_json::json!(["to", "text", "room"]));
}
