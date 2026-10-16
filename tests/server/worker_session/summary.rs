//! The session summariser: nothing carried over until a caller registers one, and the note it answers
//! carried whole, an empty one read as none.

use agency_server::hooks::Hooks;
use agency_server::worker_session::summary::{Closed, Kind, Summariser, handover};

fn closed(own_note: Option<&str>) -> Closed {
  Closed {
    kind: Kind::Worker,
    mode: "claude-code".into(),
    session_id: Some("s-1".into()),
    cwd: "/work".into(),
    account: None,
    transcript: None,
    own_note: own_note.map(str::to_owned),
    chat_id: None,
  }
}

#[tokio::test]
async fn with_no_summariser_registered_nothing_is_carried_over() {
  let hooks = Hooks::default();

  assert_eq!(handover(&hooks, closed(None)).await, None);
  assert_eq!(handover(&hooks, closed(Some("the worker's own note"))).await, None);
}

#[tokio::test]
async fn the_summarisers_note_is_carried_and_an_empty_one_is_none() {
  let hooks = Hooks::default();

  hooks.register::<Summariser, _, _>(|closed: Closed| async move {
    match closed.own_note {
      Some(note) => Some(format!("{:?}: {note}", closed.kind)),
      None => Some("   ".into()),
    }
  });

  assert_eq!(
    handover(&hooks, closed(Some("left off at step 3"))).await.as_deref(),
    Some("Worker: left off at step 3")
  );
  assert_eq!(handover(&hooks, closed(None)).await, None);
}

#[test]
fn what_closed_travels_in_the_shape_the_hook_names() {
  assert_eq!(
    serde_json::to_value(Closed {
      kind: Kind::Switch,
      chat_id: Some(-7),
      ..closed(None)
    })
    .unwrap(),
    serde_json::json!({
      "kind": "switch", "mode": "claude-code", "sessionId": "s-1", "cwd": "/work", "account": null,
      "transcript": null, "ownNote": null, "chatId": -7,
    })
  );
}
