//! The room grants: what stands in the turn's room listed, a grant made and taken back on a message the turn
//! answers and read back from the record before it is answered, the account service's refusal answered in its own
//! words, and every call that names no room turn, no message of the turn or no holder refused before the service
//! is asked (docs/tools.md, 「The room grants」).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use room_grant_tool::{Action, GrantInput, act};
use serde_json::{Value, json};
use tool_call::room_gate::{AccountService, RoomTurn};

const ROOM: i64 = -7;
const OWNER_MESSAGE: i64 = 11;
const MEMBER_MESSAGE: i64 = 12;
const HOLDER: i64 = 300;
const REFUSAL: &str = "the sender of message 12 in room -7 may not grant on bmo_room:-7";

#[derive(Default)]
struct Record {
  rows: Vec<Value>,
  changes: Vec<(String, Value)>,
  // Whether a grant is answered made and never kept, as a service whose record lost it.
  forgets: bool,
}

type Shared = Arc<Mutex<Record>>;

// The subject key a grant's holder stands under: the room's members, or the user a sender is linked to.
fn subject(to: &Value) -> String {
  if to["members"] == true {
    return format!("bmo_member:{ROOM}");
  }

  match to["sender"].as_i64() {
    Some(HOLDER) => "user:3".to_owned(),
    Some(other) => format!("user:{other}"),
    None => "user:none".to_owned(),
  }
}

// The granter is the sender of the message: the owner's message alone may grant.
fn granter_refused(body: &Value) -> Option<(StatusCode, Json<Value>)> {
  (body["message"]["id"].as_i64() != Some(OWNER_MESSAGE))
    .then(|| (StatusCode::FORBIDDEN, Json(json!({ "code": "forbidden", "error": REFUSAL }))))
}

async fn standing(
  State(record): State<Shared>,
  Query(query): Query<BTreeMap<String, String>>,
) -> Json<Value> {
  assert_eq!(query.get("room").map(String::as_str), Some("-7"));

  Json(json!({ "grants": record.lock().unwrap().rows }))
}

async fn grant(State(record): State<Shared>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
  let mut record = record.lock().unwrap();

  record.changes.push(("POST".to_owned(), body.clone()));

  if let Some(refused) = granter_refused(&body) {
    return refused;
  }

  let relation = body["relation"].as_str().unwrap().to_owned();
  let key = subject(&body["to"]);

  record.rows.retain(|row| !(row["relation"] == relation && row["subjectKey"] == key));
  if !record.forgets {
    record.rows.push(json!({
      "relation": relation,
      "subjectKey": key,
      "expiresAt": body.get("expiresAt"),
      "words": body.get("words"),
    }));
  }

  (
    StatusCode::CREATED,
    Json(json!({ "grant": format!("bmo_room:{ROOM}#{relation}"), "subject": key })),
  )
}

async fn revoke(State(record): State<Shared>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
  let mut record = record.lock().unwrap();

  record.changes.push(("DELETE".to_owned(), body.clone()));

  if let Some(refused) = granter_refused(&body) {
    return refused;
  }

  let relation = body["relation"].as_str().unwrap().to_owned();
  let key = subject(&body["to"]);
  let before = record.rows.len();

  record.rows.retain(|row| !(row["relation"] == relation && row["subjectKey"] == key));

  let revoked = before - record.rows.len();

  (
    StatusCode::OK,
    Json(json!({ "grant": format!("bmo_room:{ROOM}#{relation}"), "revoked": revoked })),
  )
}

async fn user_of(Path(sender): Path<i64>) -> (StatusCode, Json<Value>) {
  match sender {
    HOLDER => (StatusCode::OK, Json(json!({ "user": "3" }))),
    _ => (StatusCode::NOT_FOUND, Json(json!({ "code": "not-found", "error": "no user" }))),
  }
}

async fn senders_of(Path(user): Path<String>) -> Json<Value> {
  Json(json!({ "senders": if user == "3" { vec![HOLDER] } else { vec![] } }))
}

async fn accounts(forgets: bool) -> (AccountService, Shared) {
  let record = Shared::default();

  record.lock().unwrap().forgets = forgets;

  let app = Router::new()
    .route("/grants", get(standing).post(grant).delete(revoke))
    .route("/senders/{sender}/user", get(user_of))
    .route("/users/{user}/senders", get(senders_of))
    .with_state(record.clone());
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  (
    AccountService {
      address,
      token: "web-token".into(),
    },
    record,
  )
}

fn turn(messages: &[i64], privileged: bool) -> RoomTurn {
  RoomTurn {
    chat: ROOM,
    messages: messages.to_vec(),
    privileged,
  }
}

fn asked(action: Action) -> GrantInput {
  GrantInput {
    action,
    relation: None,
    sender: None,
    members: None,
    words: None,
    until: None,
    message: None,
  }
}

fn to_holder(action: Action, relation: &str) -> GrantInput {
  GrantInput {
    relation: Some(relation.into()),
    sender: Some(HOLDER),
    ..asked(action)
  }
}

fn read(answer: &tool_call::Answer) -> Value {
  assert!(!answer.is_error, "{}", answer.plain());

  serde_json::from_str(&answer.plain()).unwrap()
}

#[tokio::test]
async fn a_grant_is_made_on_the_owners_message_and_answered_with_the_row_read_back() {
  let (service, record) = accounts(false).await;
  let input = GrantInput {
    words: Some("  may ask how the agent works  ".into()),
    until: Some("2030-01-02T03:04:05Z".into()),
    ..to_holder(Action::Grant, "system")
  };
  let answered = read(&act(&service, Some(&turn(&[OWNER_MESSAGE], false)), input).await);

  assert_eq!(
    record.lock().unwrap().changes,
    [(
      "POST".to_owned(),
      json!({
        "room": ROOM,
        "relation": "system",
        "to": { "sender": HOLDER, "members": false },
        "words": "may ask how the agent works",
        "expiresAt": 1_893_553_445_000_i64,
        "message": { "room": ROOM, "id": OWNER_MESSAGE },
      })
    )]
  );
  assert_eq!(answered["room"], ROOM);
  assert_eq!(answered["granted"]["subject"], "user:3");
  assert_eq!(answered["standing"]["relation"], "system");
  assert_eq!(answered["standing"]["subjectKey"], "user:3");
  assert_eq!(answered["standing"]["words"], "may ask how the agent works");
  assert_eq!(
    answered["standing"]["senders"],
    json!([HOLDER]),
    "the holder is named by their sender ids"
  );

  // The list reads the same record.
  let listed = read(&act(&service, Some(&turn(&[], false)), asked(Action::List)).await);

  assert_eq!(listed["grants"], json!([answered["standing"]]));
}

#[tokio::test]
async fn the_rooms_members_are_granted_and_taken_back_and_the_revocation_is_read_back() {
  let (service, record) = accounts(false).await;
  let owners = turn(&[OWNER_MESSAGE], true);
  let to_members = |action| GrantInput {
    relation: Some("system".into()),
    members: Some(true),
    ..asked(action)
  };

  read(&act(&service, Some(&owners), to_members(Action::Grant)).await);

  let taken = read(&act(&service, Some(&owners), to_members(Action::Revoke)).await);

  assert_eq!(taken["revoked"]["revoked"], 1);
  assert_eq!(taken["grants"], json!([]));
  assert_eq!(
    record.lock().unwrap().changes[1],
    (
      "DELETE".to_owned(),
      json!({
        "room": ROOM,
        "relation": "system",
        "to": { "members": true },
        "message": { "room": ROOM, "id": OWNER_MESSAGE },
      })
    )
  );

  // Taking back what does not stand takes nothing back, and says so.
  let nothing = act(&service, Some(&owners), to_members(Action::Revoke)).await;

  assert!(nothing.is_error);
  assert!(nothing.plain().contains("nothing was taken back"), "{}", nothing.plain());
}

#[tokio::test]
async fn a_grant_on_a_message_whose_sender_may_not_grant_is_refused_in_the_account_services_words() {
  let (service, record) = accounts(false).await;
  let input = GrantInput {
    message: Some(MEMBER_MESSAGE),
    ..to_holder(Action::Grant, "system")
  };
  let refused = act(&service, Some(&turn(&[OWNER_MESSAGE, MEMBER_MESSAGE], false)), input).await;

  assert!(refused.is_error);
  assert!(refused.plain().contains("(403 forbidden)"), "{}", refused.plain());
  assert!(
    refused.plain().contains(REFUSAL),
    "the refusal as the service gave it: {}",
    refused.plain()
  );
  assert!(record.lock().unwrap().rows.is_empty());

  let revoking = GrantInput {
    message: Some(MEMBER_MESSAGE),
    ..to_holder(Action::Revoke, "system")
  };
  let refused = act(&service, Some(&turn(&[MEMBER_MESSAGE], false)), revoking).await;

  assert!(refused.plain().contains(REFUSAL), "{}", refused.plain());
}

#[tokio::test]
async fn a_grant_the_record_does_not_carry_afterwards_is_answered_as_not_in_force() {
  let (service, _) = accounts(true).await;
  let answered = act(&service, Some(&turn(&[OWNER_MESSAGE], true)), to_holder(Action::Grant, "ask")).await;

  assert!(answered.is_error);
  assert!(answered.plain().contains("so it is not in force"), "{}", answered.plain());
}

#[tokio::test]
async fn the_message_is_one_the_turn_answers_and_the_owners_where_the_turn_names_several() {
  let (service, record) = accounts(false).await;

  // A message the turn does not answer is never handed to the service, however it was found.
  let named_elsewhere = GrantInput {
    message: Some(1),
    ..to_holder(Action::Grant, "system")
  };
  let refused = act(&service, Some(&turn(&[OWNER_MESSAGE], true)), named_elsewhere).await;

  assert!(
    refused.plain().contains("message 1 is not one this turn answers (11)"),
    "{}",
    refused.plain()
  );

  // Several messages not all the owner's: the owner's one must be named.
  let refused = act(
    &service,
    Some(&turn(&[OWNER_MESSAGE, MEMBER_MESSAGE], false)),
    to_holder(Action::Grant, "system"),
  )
  .await;

  assert!(refused.plain().contains("name the owner's one as message"), "{}", refused.plain());

  // No message at all: nothing to grant on.
  let refused = act(&service, Some(&turn(&[], true)), to_holder(Action::Grant, "system")).await;

  assert!(refused.plain().contains("this turn answers no message"), "{}", refused.plain());
  assert!(record.lock().unwrap().changes.is_empty(), "the service was never asked");

  // Every message the owner's: the newest is the one granted on.
  let owners = turn(&[9, OWNER_MESSAGE, 10], true);

  read(&act(&service, Some(&owners), to_holder(Action::Grant, "system")).await);
  assert_eq!(
    record.lock().unwrap().changes[0].1["message"],
    json!({ "room": ROOM, "id": OWNER_MESSAGE })
  );
}

#[tokio::test]
async fn a_call_outside_a_room_turn_or_naming_no_holder_or_no_words_for_a_rule_is_refused_unasked() {
  let (service, record) = accounts(false).await;
  let owners = turn(&[OWNER_MESSAGE], true);
  let refusals = [
    (None, asked(Action::List), "stands in no room's turn"),
    (Some(&owners), asked(Action::Grant), "name the relation"),
    (
      Some(&owners),
      GrantInput {
        sender: None,
        ..to_holder(Action::Grant, "system")
      },
      "name the holder",
    ),
    (
      Some(&owners),
      GrantInput {
        members: Some(true),
        ..to_holder(Action::Grant, "system")
      },
      "name the holder once",
    ),
    (
      Some(&owners),
      to_holder(Action::Grant, "rule"),
      "a standing rule carries the owner's words",
    ),
    (
      Some(&owners),
      GrantInput {
        until: Some("next week".into()),
        ..to_holder(Action::Grant, "system")
      },
      "until does not read as an RFC 3339 time",
    ),
    (
      Some(&owners),
      GrantInput {
        words: Some("no longer".into()),
        ..to_holder(Action::Revoke, "system")
      },
      "a revocation takes no words",
    ),
  ];

  for (turn, input, said) in refusals {
    let refused = act(&service, turn, input).await;

    assert!(refused.is_error && refused.plain().contains(said), "{said}: {}", refused.plain());
  }
  assert!(record.lock().unwrap().changes.is_empty(), "the service was never asked");

  // A standing rule with the owner's words goes to the service as he gave them.
  let rule = GrantInput {
    words: Some("may ask about the system, in this room".into()),
    ..to_holder(Action::Grant, "rule")
  };

  assert_eq!(read(&act(&service, Some(&owners), rule).await)["standing"]["relation"], "rule");
}

#[tokio::test]
async fn an_account_service_that_cannot_be_asked_is_said_and_nothing_is_claimed() {
  let unnamed = AccountService::default();
  let refused = act(&unnamed, Some(&turn(&[OWNER_MESSAGE], true)), asked(Action::List)).await;

  assert!(refused.plain().contains("no account service is named"), "{}", refused.plain());

  let gone = AccountService {
    address: "http://[::1]:9".into(),
    token: String::new(),
  };
  let refused = act(&gone, Some(&turn(&[OWNER_MESSAGE], true)), to_holder(Action::Grant, "system")).await;

  assert!(refused.is_error);
  assert!(refused.plain().contains("was not reached"), "{}", refused.plain());
}
