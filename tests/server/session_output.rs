//! What a session's job turn has written so far (docs/api.md, 「Sessions」, the output read): the text
//! blocks its answers wrote in the record, in order; read while the turn goes and after it was stopped,
//! begun again by an attempt that goes out again and by the session's next turn; nothing found for a
//! session that carried no job turn, and refused for a scope whose turns keep none.

use std::collections::HashMap;
use std::sync::Arc;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::herdr_backend::HerdrSessions;
use agency_server::job_run::claude::turn::assistant_texts;
use agency_server::places::{Ground, Places};
use agency_server::seeds::deployment;
use agency_server::session_output::Outputs;
use agent_session::{Apart, FaultKind, Output, Scope, SessionId, Sessions, Start};
use harness_providers::Machine;
use serde_json::json;

use crate::scratch::Scratch;

fn said(texts: &[&str]) -> String {
  json!({
    "type": "assistant",
    "message": { "content": texts.iter().map(|text| json!({ "type": "text", "text": text })).collect::<Vec<_>>() },
  })
  .to_string()
}

#[test]
fn what_a_turn_wrote_is_every_text_block_its_answers_wrote_in_order() {
  let lines = [
    said(&["TITLE: the angle"]),
    json!({ "type": "assistant", "message": { "content": [{ "type": "tool_use", "name": "Bash" }] } })
      .to_string(),
    json!({ "type": "user", "message": { "content": [{ "type": "text", "text": "a tool's result" }] } })
      .to_string(),
    "not a record line".to_owned(),
    said(&["first finding", "second finding"]),
  ];

  assert_eq!(assistant_texts(&lines), ["TITLE: the angle", "first finding", "second finding"]);
}

fn session() -> SessionId {
  SessionId("branch-a@apart:1".into())
}

#[test]
fn a_running_turns_words_are_read_as_they_are_written_and_stand_once_it_is_stopped() {
  let outputs = Outputs::default();

  assert_eq!(outputs.read(&session()), None, "a session that carried no job turn has nothing to read");

  outputs.begun(&session(), "bmo-research-7");
  outputs.wrote(&session(), &["TITLE: the angle".into()]);
  outputs.wrote(&session(), &["first finding".into()]);

  assert_eq!(
    outputs.read(&session()),
    Some(Output {
      run: "bmo-research-7".into(),
      running: true,
      text: "TITLE: the angle\n\nfirst finding".into(),
    })
  );

  outputs.ended(&session());

  assert_eq!(
    outputs.read(&session()),
    Some(Output {
      run: "bmo-research-7".into(),
      running: false,
      text: "TITLE: the angle\n\nfirst finding".into(),
    }),
    "a turn stopped from outside leaves what it wrote"
  );

  outputs.forget(&session());

  assert_eq!(outputs.read(&session()), None, "an ended session takes its words with it");
}

#[test]
fn an_attempt_that_goes_out_again_and_the_next_turn_begin_the_words_again() {
  let outputs = Outputs::default();

  outputs.begun(&session(), "run-1");
  outputs.wrote(&session(), &["on the first login".into()]);
  outputs.attempted(&session());
  outputs.wrote(&session(), &["on the next one".into()]);

  assert_eq!(outputs.read(&session()).unwrap().text, "on the next one");

  outputs.ended(&session());
  outputs.begun(&session(), "run-2");

  assert_eq!(
    outputs.read(&session()),
    Some(Output {
      run: "run-2".into(),
      running: true,
      text: String::new(),
    })
  );
  outputs.wrote(&SessionId("branch-b@apart:2".into()), &["elsewhere".into()]);
  assert_eq!(outputs.read(&session()).unwrap().text, "", "words are their own session's alone");
}

#[tokio::test]
async fn a_session_that_carried_no_job_turn_is_nothing_found_and_an_editors_keeps_none() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let at = |path: std::path::PathBuf| path.to_string_lossy().into_owned();
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), at(home.path().to_path_buf())),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
    ],
    home.path().join("data"),
  );

  store::seed::settle(scratch.db(), &deployment(&machine)).await.unwrap();

  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let sessions = HerdrSessions::new(Places::new(Ground::new(&service, &machine, None), Arc::default()));
  let apart = sessions
    .start(Start {
      agent: "branch-a".into(),
      scope: Scope::Apart(Apart {
        provider: "claude-code".into(),
        model: "claude-sonnet-5".into(),
        cwd: "/tmp/run".into(),
        todo: None,
        role: None,
      }),
    })
    .await
    .unwrap();

  assert_eq!(sessions.output(&apart.id).await.unwrap_err().kind, FaultKind::Unknown);
  assert_eq!(
    sessions.output(&SessionId("branch-a@apart:99".into())).await.unwrap_err().kind,
    FaultKind::Unknown,
    "a session never started names nothing"
  );
  assert_eq!(
    sessions.output(&SessionId("digest@editor".into())).await.unwrap_err().kind,
    FaultKind::Refused,
    "an editor's runs keep nothing here to read"
  );
}
