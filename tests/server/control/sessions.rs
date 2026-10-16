use agency_server::control::sessions::*;
use serde_json::{Value, json};

#[test]
fn a_body_names_a_session_by_its_agent_and_a_room_or_none() {
  let read = |body: Value| Asked::read(&body).map(|asked| (asked.agent, asked.chat_id, asked.no_room));

  assert_eq!(read(json!({ "agent": "companion" })), Some(("companion".to_owned(), None, None)));
  assert_eq!(
    read(json!({ "agent": "room-companion", "chatId": -100 })),
    Some(("room-companion".to_owned(), Some(-100), None))
  );
  assert_eq!(
    read(json!({ "agent": "room-companion", "chatId": 1.5 })),
    Some(("room-companion".to_owned(), None, Some("1.5".to_owned())))
  );
  assert!(read(json!({ "agent": "room-companion", "chatId": "room" })).is_none());
  assert!(read(json!({ "chatId": null })).is_none());
  assert!(read(json!(["companion"])).is_none());
}

#[test]
fn a_worker_runs_where_agency_says_so_or_where_nobody_could_say() {
  assert!(Running(Some(vec!["companion".to_owned()])).holds("companion"));
  assert!(!Running(Some(vec![])).holds("companion"));
  assert!(Running::unread().holds("companion"));
}

#[test]
fn who_asked_is_the_body_s_word_trimmed_or_the_console() {
  assert_eq!(asked_by(&json!({ "by": " qa-admin " })), "qa-admin");
  assert_eq!(asked_by(&json!({ "by": "  " })), "console");
  assert_eq!(asked_by(&json!({})), "console");
}
