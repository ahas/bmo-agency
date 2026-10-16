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

  assert_eq!(folders, ["procedures", "read_rulebook"]);

  for folder in &folders {
    let manifest = read(&tools().join(folder).join("tool.toml"));
    let crate_manifest = read(&tools().join(folder).join("Cargo.toml"));
    let package = text(&crate_manifest, &["package", "name"]);
    let binary = text(&crate_manifest["bin"][0], &["name"]);

    assert_eq!(text(&manifest, &["tool", "name"]), folder);
    assert_eq!(manifest["tool"]["protocol"].as_integer(), Some(i64::from(tool_call::PROTOCOL)));
    assert_eq!(text(&manifest, &["tool", "language"]), "rust");
    assert_eq!(words(&manifest["tool"]["run"]), [binary]);
    assert!(manifest["tool"]["timeout"].as_integer().is_some_and(|seconds| seconds > 0));
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
fn the_rulebook_readers_entry_is_declared_as_its_tool_answers_it() {
  let folder = tools().join("read_rulebook");
  let manifest = read(&folder.join("tool.toml"));
  let entries = manifest["entry"].as_array().unwrap();
  let entry = &entries[0];

  assert_eq!(entries.len(), 1);
  assert_eq!(text(entry, &["namespace"]), read_rulebook_tool::NAMESPACE);
  assert_eq!(text(entry, &["name"]), read_rulebook_tool::NAME);
  assert_eq!(text(entry, &["title"]), read_rulebook_tool::TITLE);
  assert_eq!(text(entry, &["description"]), read_rulebook_tool::DESCRIPTION);
  assert_eq!(words(&entry["capabilities"]), read_rulebook_tool::CAPABILITIES);
  assert_eq!(entry["hints"]["read_only"].as_bool(), Some(true));

  let schema: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(folder.join(text(entry, &["input"]))).unwrap()).unwrap();

  assert_eq!(schema["type"], "object");
  assert_eq!(schema["required"], serde_json::json!(["name"]));
  assert_eq!(schema["properties"]["name"]["type"], "string");
  assert_eq!(schema["properties"]["name"]["minLength"], 1);
}
