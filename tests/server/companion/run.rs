//! One run of a companion's turn: the lines its record leaves are labelled with the provider it went out on.

use std::time::Instant;

use agency_server::Service;
use agency_server::companion::run::Watched;
use agency_server::config::Config;
use agency_server::places::Ground;
use agency_server::run_log::{RunLog, Target};
use harness_providers::Machine;
use sea_orm::DatabaseConnection;
use serde_json::json;

// A companion turn may stand any provider's program, so the lines its record leaves are labelled with the
// provider the turn went out on rather than one fixed program's.
#[tokio::test]
async fn the_lines_a_turn_leaves_are_labelled_with_its_own_provider() {
  let service = Service::new(DatabaseConnection::Disconnected, Config::read(|_| None).unwrap());
  let machine = Machine::with_env([("HOME".to_owned(), "/home/tester".to_owned())], "/data");
  let ground = Ground::new(&service, &machine, None);
  let mut log = RunLog::new(Target {
    agent: "sample-agent".to_owned(),
    run: "chat:1:1".to_owned(),
    chat_id: Some(1),
    job_id: None,
  });
  let mut watched = Watched {
    ground: &ground,
    log: &mut log,
    source: "codex",
    run: "chat:1:1".to_owned(),
    guard: None,
    tally: None,
    started: Instant::now(),
    last_tally: Instant::now(),
    cut: None,
  };
  let rollout = json!({ "type": "response_item", "payload": { "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "done" }] } });
  let written = watched.feed(&[rollout.to_string()]).await;

  assert_eq!(
    written.iter().map(|line| line.source.as_deref()).collect::<Vec<_>>(),
    vec![Some("codex")]
  );
}
