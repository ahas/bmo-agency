//! A pinned model is the one its pin names now (docs/repositories.md, 「A pinned model」; docs/companions.md,
//! 「The program on an ended session」): a repository's jobs stand on the registration its pin names, a pin
//! moved to another registration is that one, and a companion's program standing on another model than its
//! order's registration is let go.

use crate::support::rows::{repository, supervisor};
use crate::support::scratch::Scratch;

#[tokio::test]
async fn a_worker_s_model_is_the_pin_s_never_its_last_model() {
  let scratch = Scratch::new().await;

  repository(
    scratch.db(),
    "kakia",
    "/checkouts/kakia",
    &[("pinned_provider", "registered".into()), ("pinned_model", "flash-next".into())],
  )
  .await;

  assert_eq!(
    agency_server::herdr_backend::pinned_worker_model(scratch.db(), "kakia").await.unwrap(),
    Some("flash-next".to_owned())
  );

  scratch
    .run(
      r#"UPDATE "repositories" SET "pinned_model" = 'next-registration' WHERE "name" = 'kakia'"#,
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

  supervisor(
    scratch.db(),
    "kakia-chat",
    &[
      ("provider_order", r#"[{"provider":"registered","model":"flash-next"}]"#.into()),
      ("last_model", "flash-next-coder".into()),
    ],
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
