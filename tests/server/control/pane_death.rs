//! A head pane's death: the loop reports its exit code and last stderr line, and the worker's running job
//! ends failed with that in the report (docs/agent-control.md, 「A pane that dies」).

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::work::pane_died;
use agency_server::places::Ground;
use harness_providers::Machine;

use crate::support::scratch::Scratch;

#[tokio::test]
async fn a_head_loop_reports_a_death_bounds_three_fast_deaths_and_keeps_exit_three_deliberate() {
  let home = tempfile::tempdir().unwrap();
  let scratch = Scratch::new().await;
  let variables = HashMap::<String, String>::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let ground = Ground::new(
    &service,
    &Machine::with_env(
      [("HOME".to_owned(), home.path().to_string_lossy().into_owned())],
      home.path().join("data"),
    ),
    None,
  );
  let command = ground.head_command("worker", home.path());
  let script = &command[2];

  assert!(script.contains("pane-deaths"), "{script}");
  assert!(script.contains("stderr.log"), "{script}");
  assert!(script.contains("deaths"), "{script}");
  assert!(script.contains("\"$deaths\" -ge 3"), "{script}");
  assert!(script.contains("3) exit 0"), "{script}");
}

#[tokio::test]
async fn the_head_loop_starts_pane_head_with_the_agents_name() {
  let home = tempfile::tempdir().unwrap();
  let scratch = Scratch::new().await;
  let variables = HashMap::<String, String>::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let ground = Ground::new(
    &service,
    &Machine::with_env(
      [("HOME".to_owned(), home.path().to_string_lossy().into_owned())],
      home.path().join("data"),
    ),
    None,
  );
  let argument = home.path().join("pane-head-argument");
  let head = home.path().join("bin").join("pane-head");

  std::fs::create_dir_all(head.parent().unwrap()).unwrap();
  std::fs::write(&head, format!("#!/bin/sh\nprintf '%s\\n' \"$1\" > '{}'\nexit 3\n", argument.display()))
    .unwrap();
  std::fs::set_permissions(&head, std::fs::Permissions::from_mode(0o755)).unwrap();

  let command = ground.head_command("worker", home.path());
  let status = std::process::Command::new(&command[0]).args(&command[1..]).status().unwrap();

  assert!(status.success(), "the loop ends 0 on the one exit it obeys");
  assert_eq!(std::fs::read_to_string(&argument).unwrap().trim(), "worker");
}

#[tokio::test]
async fn a_pane_death_ends_a_running_job_failed_with_its_code_and_last_line() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let variables = HashMap::<String, String>::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let ground = Ground::new(
    &service,
    &Machine::with_env(
      [("HOME".to_owned(), home.path().to_string_lossy().into_owned())],
      home.path().join("data"),
    ),
    None,
  )
  .with_var("BMO_TEST_GROUND", Some("1"));

  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state", "token_count", "started_at", "prompt")
         VALUES (70, 700, 7, 'harness', 1, 'worker-a', 'running', 0, $1, '# item')"#,
      vec![agency_server::now_ms().into()],
    )
    .await;

  let ended = pane_died(&ground, "worker-a", 1, "boom: the program stopped").await.unwrap();
  assert_eq!(ended, vec![70]);

  let row = store::one(
    scratch.db(),
    r#"SELECT "state", "outcome", "report" FROM "jobs" WHERE "id" = 70"#,
    vec![],
  )
  .await
  .unwrap()
  .unwrap();
  assert_eq!(
    (
      row.try_get::<String>("", "state").unwrap(),
      row.try_get::<String>("", "outcome").unwrap(),
    ),
    ("ended".to_owned(), "failed".to_owned())
  );
  let report: String = row.try_get("", "report").unwrap();
  assert!(report.contains("exit code 1"), "{report}");
  assert!(report.contains("boom: the program stopped"), "{report}");
}
