//! DeepSeek: present while the road's key file is private and holds a key; its two models and the words a
//! run of one may go out with; and the pane program that is its own.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;

use harness_providers::deepseek::*;
use harness_providers::{Machine, Provider, standing_provider};

#[test]
fn the_key_file_answers_only_while_it_is_private_and_holds_a_key() {
  let dir = tempfile::tempdir().unwrap();
  let machine = Machine::with_env([], dir.path());
  let key = DeepSeek::key_file(&machine);

  assert!(DeepSeek.missing(&machine).is_some());

  std::fs::create_dir_all(key.parent().unwrap()).unwrap();
  std::fs::write(&key, "secret\n").unwrap();
  std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
  assert!(DeepSeek.missing(&machine).is_some());

  std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
  assert_eq!(DeepSeek.missing(&machine), None);

  std::fs::write(&key, "").unwrap();
  assert!(DeepSeek.missing(&machine).is_some());
}

#[tokio::test]
async fn the_catalogue_is_the_two_models_its_policy_declares() {
  let dir = tempfile::tempdir().unwrap();
  let machine = Machine::with_env([], dir.path());
  let catalogue = DeepSeek.catalogue(&machine, None).await.unwrap();
  let ids: Vec<&str> = catalogue["models"]
    .as_array()
    .unwrap()
    .iter()
    .filter_map(|one| one["id"].as_str())
    .collect();

  assert_eq!(ids, ["deepseek-flash", "deepseek-v4-pro"]);

  let offered = DeepSeek.offered(&machine, None, 0.0).await.unwrap();

  assert_eq!(offered.models.len(), 2);
  assert_eq!(
    offered.models[0].efforts,
    [
      None,
      Some("none".into()),
      Some("low".into()),
      Some("medium".into()),
      Some("high".into()),
      Some("max".into())
    ]
  );
}

#[test]
fn a_run_without_thinking_names_its_own_off_word_and_a_level_stands_where_the_model_takes_it() {
  let levels = vec!["low".to_owned(), "medium".to_owned(), "high".to_owned()];

  assert_eq!(
    DeepSeek.run_effort(Some("high"), Some(false), Some("low"), &levels),
    Some("none".into())
  );
  assert_eq!(
    DeepSeek.run_effort(Some("high"), Some(true), Some("medium"), &levels),
    Some("medium".into())
  );
  assert_eq!(DeepSeek.run_effort(Some("max"), Some(true), Some("xhigh"), &levels), Some("max".into()));
  assert_eq!(DeepSeek.run_effort(Some("low"), None, None, &levels), Some("low".into()));
}

#[test]
fn bmo_tui_standing_a_dsh_session_on_its_own_route_is_this_providers_and_claude_code_is_not() {
  let dir = tempfile::tempdir().unwrap();
  let machine = Machine::with_env([], dir.path());
  let owner = |args: &[&str]| {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();

    standing_provider(&machine, &args, &BTreeMap::new()).map(|provider| provider.name())
  };
  let pane = |choice: &str| {
    owner(&["/deploy/bin/bmo-tui", "dsh", "--program", "dsh", "--place", "/place", "--model", choice])
  };

  assert_eq!(pane(r#"["deepseek","deepseek-flash"]"#), Some("deepseek"));
  assert_eq!(pane(r#"["openrouter","vendor/model"]"#), Some("openrouter"));
  // Claude Code under any home stays Claude Code's: no program of this provider's is Claude Code.
  let home = dir.path().join("claude-roads").join("deepseek").join("deepseek-flash");
  let env = BTreeMap::from([("CLAUDE_CONFIG_DIR".to_owned(), home.to_string_lossy().into_owned())]);

  assert_eq!(
    standing_provider(&machine, &["claude".to_owned()], &env).map(|provider| provider.name()),
    Some("claude-code")
  );
}
