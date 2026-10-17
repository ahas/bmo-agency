//! A turn's context as the turn route reads it from a body: every part optional, the room's block whole,
//! the road and the persona trimmed and none where blank, and a body that does not read refused.

use agency_server::control::session_ops::context_read;
use agent_session::{RoomBlock, RunParts, TurnContext};
use serde_json::json;

#[test]
fn a_turns_context_reads_every_part_it_names_and_none_of_what_it_does_not() {
  assert_eq!(context_read(&json!({ "session": "a", "prompt": "p" })).unwrap(), TurnContext::default());
  assert_eq!(
    context_read(&json!({
      "room": { "chatId": -5, "text": "the room's block", "mark": "[a]" },
      "via": " console ",
      "basePersona": "  ",
      "privileged": true,
      "answers": [7, 8],
      "began": 1_000,
    }))
    .unwrap(),
    TurnContext {
      room: Some(RoomBlock {
        chat_id: -5,
        text: "the room's block".into(),
        mark: "[a]".into()
      }),
      via: Some("console".into()),
      base_persona: None,
      privileged: true,
      answers: vec![7, 8],
      began: Some(1_000),
      run: None,
    }
  );
  assert!(context_read(&json!({ "room": { "text": "no room" } })).is_err());

  let parts = context_read(&json!({
    "run": "bmo-digest:digest:companion:-7:1",
    "parent": null,
    "agent": "companion",
    "chatId": -7,
    "step": 1,
    "steps": 2,
    "workdir": "",
    "desk": "desk-1",
    "registration": null,
  }))
  .unwrap()
  .run;

  assert_eq!(
    parts,
    Some(RunParts {
      name: "bmo-digest:digest:companion:-7:1".into(),
      parent: None,
      agent: Some("companion".into()),
      chat_id: Some(-7),
      step: Some(1),
      steps: Some(2),
      workdir: None,
      desk: Some("desk-1".into()),
      registration: None,
    })
  );
  assert_eq!(context_read(&json!({ "agent": "companion" })).unwrap().run, None);
  assert!(context_read(&json!({ "answers": "7" })).is_err());
}
