use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agency_server::lines::place::*;
use agent_host::place::ResidentCommand;
use harness_providers::Machine;
use serde_json::{Map, Value, json};

#[test]
fn a_program_whose_settings_came_to_call_other_hooks_is_another_program() {
  let dir = tempfile::tempdir().unwrap();
  let settings = dir.path().join("settings.json");
  let command = ResidentCommand {
    args: vec!["claude".into(), "--settings".into(), settings.to_string_lossy().into_owned()],
    cwd: dir.path().to_path_buf(),
    env: BTreeMap::new(),
  };

  std::fs::write(&settings, r#"{"statusLine":{"command":"old"}}"#).unwrap();

  let before = command_digest(&command);

  assert_eq!(command_digest(&command), before);
  std::fs::write(&settings, r#"{"statusLine":{"command":"new"}}"#).unwrap();
  assert_ne!(command_digest(&command), before);
}

#[test]
fn a_guard_the_place_s_verb_stands_is_the_program_s_and_another_hook_is_not() {
  let dir = tempfile::tempdir().unwrap();
  let settings = dir.path().join("settings.json");
  let command = ResidentCommand {
    args: vec!["claude".into(), "--settings".into(), settings.to_string_lossy().into_owned()],
    cwd: dir.path().to_path_buf(),
    env: BTreeMap::new(),
  };
  let hook = |command: &str| {
    json!({ "statusLine": { "command": "s" }, "hooks": { "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "command": command }] }] } })
      .to_string()
  };

  std::fs::write(&settings, r#"{"statusLine":{"command":"s"}}"#).unwrap();

  let bare = command_digest(&command);

  std::fs::write(&settings, hook("bmo job guard")).unwrap();
  assert_eq!(command_digest(&command), bare);
  std::fs::write(&settings, hook(&format!("bin/bmo-agency {STATUS_VERB} shell --place p"))).unwrap();
  assert_ne!(command_digest(&command), bare);
}

#[test]
fn a_command_opening_its_session_is_compared_by_the_form_that_resumes_it() {
  let machine = Machine::with_env([("HOME".to_owned(), "/fixture/home".to_owned())], "/data");
  let place = Path::new("/data/agent-places/worker");
  let env = BTreeMap::from([
    ("BMO_RUN_NAME".to_owned(), "bmo-standby-worker".to_owned()),
    ("BMO_TURN_ROOM".to_owned(), "5".to_owned()),
    ("PATH".to_owned(), "/bin".to_owned()),
  ]);
  let opening = ["claude".to_owned(), "--session-id".to_owned(), "s1".to_owned()];
  let program = resident_of(&machine, place, "/w", &opening, &env, "s1");
  let resumed = resident_of(
    &machine,
    place,
    "/w",
    &["claude".to_owned(), "--resume".to_owned(), "s1".to_owned()],
    &env,
    "s1",
  );

  assert_eq!(program.identity, resumed.identity);
  assert_eq!(program.restart.as_ref().unwrap().args, ["claude", "--resume", "s1"]);
  assert_eq!(
    program.command.env.get(PLACE_VARIABLE).map(String::as_str),
    Some("/data/agent-places/worker")
  );
  assert!(!program.command.env.contains_key("BMO_RUN_NAME"));
  assert!(!program.command.env.contains_key("BMO_TURN_ROOM"));
  assert_eq!(program.opened, Some(Some(PathBuf::from("/fixture/home/.claude/projects/-w/s1.jsonl"))));
  assert_eq!(resumed.opened, Some(None));
}

#[test]
fn a_places_settings_keep_the_lines_own_hooks_and_add_the_status_line_and_the_four_hooks() {
  let dir = tempfile::tempdir().unwrap();
  let base = dir.path().join("settings.json");

  std::fs::write(
    &base,
    r#"{"env":{"A":"1"},"hooks":{"Stop":[{"hooks":[{"type":"command","command":"own"}]}]}}"#,
  )
  .unwrap();

  let place = dir.path().join("place");
  let path = write_place_settings(
    &place,
    read_settings(&base),
    Path::new("/deploy"),
    Holds {
      tasks: true,
      wording: false,
      compose_typed: false,
      shell_guard: false,
    },
  )
  .unwrap();
  let settings: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
  let stop = settings["hooks"]["Stop"].as_array().unwrap();

  assert_eq!(settings["env"]["A"], "1");
  assert_eq!(stop[0]["hooks"][0]["command"], "own");
  assert_eq!(
    stop[1]["hooks"][0]["command"],
    format!(
      "'/deploy/bin/bmo-agency' 'agent-status' 'stop' '--place' '{}' '--hold-tasks'",
      place.display()
    )
  );
  assert!(settings["statusLine"]["command"].as_str().unwrap().contains("'status'"));
  assert_eq!(settings["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
  assert_eq!(
    settings["hooks"]["StopFailure"][0]["hooks"][0]["command"],
    format!("'/deploy/bin/bmo-agency' 'agent-status' 'failure' '--place' '{}'", place.display())
  );
  assert!(settings["hooks"].get("PreToolUse").is_none());
}

#[test]
fn a_place_whose_line_guards_its_shell_reads_every_bash_call_first() {
  let dir = tempfile::tempdir().unwrap();
  let place = dir.path().join("place");
  let path = write_place_settings(
    &place,
    Map::new(),
    Path::new("/deploy"),
    Holds {
      shell_guard: true,
      ..Holds::default()
    },
  )
  .unwrap();
  let settings: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
  let guard = &settings["hooks"]["PreToolUse"][0];

  assert_eq!(guard["matcher"], "Bash");
  assert_eq!(
    guard["hooks"][0]["command"],
    format!("'/deploy/bin/bmo-agency' 'agent-status' 'shell' '--place' '{}'", place.display())
  );
}
