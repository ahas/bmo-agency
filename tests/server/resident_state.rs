//! An agent's own session on a program that carries its own turns (docs/api.md, 「sessions」; docs/sessions.md,
//! 「Session compaction」): readable from its start though its agent holds no session id, on the provider and
//! model its last answer went out on where its row pins none, at its resident's context.

use std::collections::HashMap;
use std::sync::Arc;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::herdr_backend::HerdrSessions;
use agency_server::lines::place::Place;
use agency_server::places::{Ground, Places};
use agency_server::seeds::deployment;
use agent_session::{Context, SessionId, Sessions};
use harness_providers::Machine;
use serde_json::json;

use crate::scratch::Scratch;

#[tokio::test]
async fn an_agent_s_session_on_an_inbox_resident_reads_its_provider_and_its_resident_s_context() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), home.path().to_string_lossy().into_owned()),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
    ],
    home.path().join("data"),
  );

  store::seed::settle(scratch.db(), &deployment(&machine)).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "session_scope", "turn_shape", "last_mode", "last_model")
         VALUES ('agent-a', 'chat', 'agent', 'window', 'deepseek', 'deepseek-flash')"#,
      vec![],
    )
    .await;

  let place = Place::of(&home.path().join("data"), "agent-a");
  let lines = [
    json!({ "kind": "start", "id": "t", "name": "chat:-5:1", "at": 1 }),
    json!({ "kind": "answer", "id": "t", "text": "hi", "context": { "used": 300_000, "size": 1_000_000 } }),
    json!({ "kind": "end", "id": "t" }),
  ];

  std::fs::create_dir_all(&place.path).unwrap();
  std::fs::write(
    place.path.join("loop.jsonl"),
    lines.iter().map(|line| format!("{line}\n")).collect::<String>(),
  )
  .unwrap();

  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let sessions = HerdrSessions::new(Places::new(Ground::new(&service, &machine, None), Arc::default()));
  let state = sessions.read_state(&SessionId("agent-a".into()), None).await.unwrap();

  assert_eq!((state.provider.as_str(), state.model.as_str()), ("deepseek", "deepseek-flash"));
  assert_eq!(
    state.context,
    Some(Context {
      carried: 300_000,
      window: 1_000_000
    })
  );
}
