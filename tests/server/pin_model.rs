//! A pinned worker's model comes from its pin, never the last model its row ran on (docs/companions.md,
//! 「The program on an ended session」): a pin moved to another registration makes the program stand the
//! registration it names now.

use crate::scratch::Scratch;

#[tokio::test]
async fn a_worker_s_model_is_the_pin_s_never_its_last_model() {
  let scratch = Scratch::new().await;

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "pinned_provider", "pinned_model", "last_model")
         VALUES ('kakia', 'worker', 'registered', 'flash-next', 'flash-next-coder')"#,
      vec![],
    )
    .await;

  assert_eq!(
    agency_server::herdr_backend::pinned_worker_model(scratch.db(), "kakia").await.unwrap(),
    Some("flash-next".to_owned())
  );

  scratch
    .run(
      r#"UPDATE "agents" SET "pinned_model" = 'next-registration', "last_model" = 'flash-next-coder'
         WHERE "name" = 'kakia'"#,
      vec![],
    )
    .await;

  assert_eq!(
    agency_server::herdr_backend::pinned_worker_model(scratch.db(), "kakia").await.unwrap(),
    Some("next-registration".to_owned()),
    "a pin moved to another registration is the model, not the last one"
  );
}

#[tokio::test]
async fn a_companion_program_on_an_old_model_is_let_go_for_its_pin() {
  use std::collections::HashMap;

  use agency_server::Service;
  use agency_server::config::Config;
  use agency_server::lines::place::{Place, Program};
  use agency_server::places::Ground;
  use agent_host::place::ResidentCommand;
  use harness_providers::Machine;

  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let service = Service::new(
    scratch.db().clone(),
    Config::read(|name| HashMap::<String, String>::new().get(name).cloned()).unwrap(),
  );
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), home.path().to_string_lossy().into_owned()),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
    ],
    home.path().join("data"),
  );
  let ground = Ground::new(&service, &machine, None);

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "pinned_provider", "pinned_model", "last_model")
         VALUES ('kakia-chat', 'chat', 'registered', 'flash-next', 'flash-next-coder')"#,
      vec![],
    )
    .await;

  let place = Place::of(&ground.roots.data, "kakia-chat");
  let program = Program {
    identity: "stale".into(),
    command: ResidentCommand {
      args: vec!["claude".into(), "--model".into(), "flash-next-coder@n3".into()],
      cwd: home.path().to_path_buf(),
      env: [].into(),
    },
    restart: None,
    opened: None,
  };

  std::fs::create_dir_all(&place.path).unwrap();
  std::fs::write(place.path.join("program.json"), serde_json::to_vec(&program).unwrap()).unwrap();

  agency_server::places::standby::release_stale_companion(&ground, "kakia-chat").await;

  assert!(place.program().is_none(), "the program on the old model is let go");
}
