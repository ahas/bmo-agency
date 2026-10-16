//! A folder trusted in a login's configuration, by the module and as a work node's setup runs it as
//! `agent-host trust` (docs/work-nodes.md, 「The workplace on a node」).

use std::path::Path;
use std::process::{Command, Output};

use agent_host::trust::{Trust, config_file, trust_folder};
use serde_json::Value;

#[test]
fn a_folder_is_written_trusted_with_every_other_entry_kept_in_its_order() {
  let dir = tempfile::tempdir().unwrap();
  let file = config_file(dir.path());

  std::fs::write(
    &file,
    "{\n  \"zeta\": 1,\n  \"projects\": {\n    \"/a\": { \"x\": 2 }\n  },\n  \"alpha\": true\n}\n",
  )
  .unwrap();

  assert_eq!(trust_folder(&file, "/a"), Trust::Written);
  assert_eq!(trust_folder(&file, "/a"), Trust::Trusted);

  let text = std::fs::read_to_string(&file).unwrap();
  let read: Value = serde_json::from_str(&text).unwrap();

  assert!(text.find("zeta").unwrap() < text.find("alpha").unwrap());
  assert_eq!(read["projects"]["/a"], serde_json::json!({ "x": 2, "hasTrustDialogAccepted": true }));
  assert!(!dir.path().join(".claude.json.lock").exists());
}

#[test]
fn nothing_is_written_where_no_login_lives_or_the_file_is_no_configuration() {
  let dir = tempfile::tempdir().unwrap();
  let file = config_file(dir.path());

  assert_eq!(trust_folder(&config_file(&dir.path().join("none")), "/a"), Trust::NoConfigDir);
  std::fs::write(&file, "[1]").unwrap();
  assert_eq!(trust_folder(&file, "/a"), Trust::Unreadable);
}

fn trust(file: &Path, folder: &str) -> Output {
  Command::new(crate::binary::built("agent-host", "agent-host"))
    .arg("trust")
    .arg(file)
    .arg(folder)
    .output()
    .unwrap()
}

#[test]
fn a_checkout_is_trusted_once_in_the_logins_configuration_and_a_login_that_is_not_there_is_refused() {
  let root = tempfile::tempdir().unwrap();
  let config = root.path().join("login/.claude.json");

  std::fs::create_dir_all(config.parent().unwrap()).unwrap();

  let written = trust(&config, "/work/checkout");

  assert!(written.status.success());
  assert_eq!(
    String::from_utf8_lossy(&written.stdout).trim(),
    format!("/work/checkout: written in {}", config.display())
  );

  let again = trust(&config, "/work/checkout");

  assert!(again.status.success());
  assert!(String::from_utf8_lossy(&again.stdout).contains("/work/checkout: trusted in "));

  let refused = trust(&root.path().join("gone/.claude.json"), "/work/checkout");

  assert!(!refused.status.success());
  assert!(String::from_utf8_lossy(&refused.stdout).contains("no-config-dir"));
}
