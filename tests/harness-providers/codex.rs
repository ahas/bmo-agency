//! Codex: its headline windows, its catalogue as the executable on the PATH prints it, and its failures.

use harness_providers::codex::*;
use harness_providers::{Headline, Machine, Provider, Window};

#[test]
fn the_headline_is_the_standard_windows_by_length_then_by_slot() {
  let window = |name, duration_minutes| Window {
    name,
    duration_minutes,
  };
  let standard = [
    window("codex:default:primary", Some(300)),
    window("codex:codex:secondary", None),
    window("codex:codex:primary", Some(300)),
  ];
  let legacy = [window("codex:default:secondary", Some(10_080))];

  assert_eq!(
    Codex.headline(&standard),
    Headline {
      session: Some(2),
      week: Some(1)
    }
  );
  assert_eq!(
    Codex.headline(&legacy),
    Headline {
      session: None,
      week: Some(0)
    }
  );
  assert_eq!(Codex.headline(&[]), Headline::default());
}

/// keeps visible chooser fields and leaves hidden compatibility slugs out
#[test]
fn keeps_visible_chooser_fields_and_leaves_hidden_compatibility_slugs_out() {
  let read = read_models(
    r#"{"models":[
      {"slug":"gpt-6-astra","display_name":"GPT-6-Astra","description":"Most capable","context_window":1050000,
       "default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"other":1}],"visibility":"list"},
      {"slug":"old-alias","visibility":"hide"},{"slug":""},{"display_name":"no slug"},{"slug":"bare"}]}"#,
  )
  .unwrap();

  assert_eq!(
    read,
    [
      Model {
        id: "gpt-6-astra".into(),
        name: "GPT-6-Astra".into(),
        description: "Most capable".into(),
        context_length: Some(1_050_000),
        default_reasoning_effort: "low".into(),
        supported_reasoning_efforts: vec!["low".into(), "medium".into()],
      },
      Model {
        id: "bare".into(),
        name: "bare".into(),
        description: String::new(),
        context_length: None,
        default_reasoning_effort: "medium".into(),
        supported_reasoning_efforts: vec![],
      },
    ]
  );
}

/// refuses an output with no catalogue rather than showing an empty lie
#[test]
fn refuses_an_output_with_no_catalogue_rather_than_showing_an_empty_lie() {
  assert_eq!(read_models("{}").unwrap_err(), "codex models: the CLI returned no model list");
}

#[test]
fn a_failure_names_the_last_error_reported_then_the_last_word_said() {
  assert_eq!(
    failure(
      "{\"type\":\"error\",\"message\":\"one\"}\n{\"type\":\"turn.failed\",\"error\":{\"message\":\"two\"}}",
      "x",
      Some(1)
    ),
    "two"
  );
  assert_eq!(failure("", "first\nlast\n\n", Some(1)), "last");
  assert_eq!(failure("", "", Some(3)), "exit 3");
}

#[tokio::test]
async fn the_catalogue_is_the_one_the_executable_on_the_path_prints() {
  use std::os::unix::fs::PermissionsExt;

  let dir = tempfile::tempdir().unwrap();
  let program = dir.path().join("codex");

  std::fs::write(&program, "#!/bin/sh\n[ \"$*\" = \"debug models --bundled\" ] || exit 2\necho '{\"models\":[{\"slug\":\"gpt-5.6-sol\"}]}'\n").unwrap();
  std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();

  let machine = Machine::with_env([("PATH".to_owned(), dir.path().display().to_string())], dir.path());

  assert_eq!(Codex.missing(&machine), None);
  assert_eq!(models(&machine).await.unwrap()[0].id, "gpt-5.6-sol");
  assert_eq!(
    Codex.missing(&Machine::with_env([], dir.path())).as_deref(),
    Some("The Codex executable is missing.")
  );
}
