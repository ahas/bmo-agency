//! The herdr backend's session ids: each names its agent and its scope, and reads back as both.

use agency_server::herdr_backend::{id_of, scope_of};
use agent_session::{Scope, SessionId};

#[test]
fn a_session_id_names_its_agent_and_scope_and_reads_back() {
  for scope in [Scope::Agent, Scope::Worker, Scope::Room("-1001".into())] {
    let id = id_of("companion-a", &scope);

    assert_eq!(scope_of(&id), ("companion-a".to_owned(), scope));
  }

  assert_eq!(id_of("worker-a", &Scope::Worker), SessionId("worker-a@worker".into()));
  assert_eq!(id_of("companion-a", &Scope::Room("7".into())), SessionId("companion-a@7".into()));
}

#[test]
fn a_road_sums_what_each_model_reported_and_none_where_none_reported() {
  use agency_server::door::record::ModelSpend;
  use agency_server::herdr_backend::RoadWalked;

  let spend = |model: &str, input: i64| ModelSpend {
    mode: "claude-code".into(),
    model: model.into(),
    tokens: input + 1,
    input_tokens: input,
    output_tokens: 1,
    cache_read_tokens: 10,
    cache_write_tokens: 0,
    cost: 0.0,
  };
  let started = std::time::Instant::now();
  let answered = RoadWalked::of(
    -5,
    "run-1",
    "claude-code",
    "model-a",
    true,
    started,
    &[spend("a", 3), spend("b", 4)],
    Some(0.5),
  );

  assert_eq!(
    (
      answered.stage.as_str(),
      answered.outcome,
      answered.input_tokens,
      answered.output_tokens,
      answered.cache_read_tokens
    ),
    ("chat", "answered", Some(7), Some(2), Some(20))
  );

  let failed = RoadWalked::of(-5, "run-1", "claude-code", "model-a", false, started, &[], None);

  assert_eq!((failed.outcome, failed.input_tokens, failed.context_ratio), ("error", None, None));
}

#[tokio::test]
async fn an_agents_session_across_rooms_is_kept_on_its_row_and_let_go() {
  use agency_server::herdr_backend::{agent_session, keep_agent_session};

  let scratch = crate::scratch::Scratch::new().await;

  scratch
    .run(r#"INSERT INTO "agents" ("name", "kind") VALUES ('chat-agent', 'chat')"#, vec![])
    .await;

  assert_eq!(agent_session(scratch.db(), "chat-agent").await.unwrap(), None);

  keep_agent_session(scratch.db(), "chat-agent", Some(("s-1", -5))).await.unwrap();

  assert_eq!(agent_session(scratch.db(), "chat-agent").await.unwrap(), Some("s-1".into()));

  keep_agent_session(scratch.db(), "chat-agent", None).await.unwrap();

  assert_eq!(agent_session(scratch.db(), "chat-agent").await.unwrap(), None);
  assert_eq!(
    scratch
      .count(r#"SELECT "chat_id" FROM "agents" WHERE "name" = 'chat-agent'"#, vec![])
      .await,
    -5,
    "the room its last turn answered stays"
  );
}
