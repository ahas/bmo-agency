//! The job service's events told to an item's orderer and the owner (docs/agent-control.md, 「An item's change
//! is told to its orderer」): an item closed by somebody other than its orderer, a run's question asked and
//! withdrawn once a reply settles it, a question past its due time told to the owner, each once; and a pass
//! with no cursor, or one past the window, telling nothing of what it passed.

use agency_server::queue::settings;
use agency_server::watches::job_events::{CURSOR_KEY, QUESTION_TOLD_PREFIX, pass};
use policy::job_notices::JOB_NOTICES;
use serde_json::{Value, json};

use crate::support::peers::{item, stand};
use crate::support::rows::supervisor;
use crate::support::scratch::Scratch;

fn event(id: i64, kind: &str, data: Value) -> Value {
  json!({ "id": id, "kind": kind, "at": id, "data": data })
}

fn closed(id: i64, item_id: i64, by: Option<&str>) -> Value {
  event(
    id,
    "item.changed",
    item(item_id, "done", json!({ "previousState": "leased", "changedBy": by, "raisedBy": 70 })),
  )
}

#[tokio::test]
async fn a_pass_with_no_cursor_tells_nothing_it_passed_and_keeps_the_last_number() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;

  peers.lock().unwrap().events = vec![closed(3, 71, Some("someone")), closed(4, 72, Some("someone"))];

  pass(db, &vars).await.unwrap();

  assert!(peers.lock().unwrap().instructed().is_empty());
  assert_eq!(settings::get(db, CURSOR_KEY).await.unwrap().as_deref(), Some("4"));

  let asked = peers
    .lock()
    .unwrap()
    .calls
    .iter()
    .find(|call| call.path == "/events")
    .unwrap()
    .query
    .clone();

  assert!(asked.contains("kinds=item.changed%2Cquestion.created"), "{asked}");
  assert!(!asked.contains("after="), "no cursor reads from the oldest: {asked}");

  // An empty window starts the cursor at nothing, so what comes next is told.
  let scratch = Scratch::new().await;
  let (_, empty) = stand().await;

  pass(scratch.db(), &empty).await.unwrap();
  assert_eq!(settings::get(scratch.db(), CURSOR_KEY).await.unwrap().as_deref(), Some("0"));
}

#[tokio::test]
async fn an_item_closed_by_somebody_else_is_told_once_to_its_orderer_and_its_own_close_is_not() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;

  supervisor(db, "chat", &[]).await;
  settings::set(db, CURSOR_KEY, "10").await.unwrap();

  {
    let mut held = peers.lock().unwrap();

    held
      .items
      .insert(70, item(70, "done", json!({ "scope": "-700", "requesterName": "agent-a" })));
    held.events = vec![
      closed(11, 71, Some("someone")),
      closed(12, 72, Some("agent-a")),
      closed(13, 73, Some("chat")),
      event(
        14,
        "item.changed",
        item(74, "held", json!({ "previousState": "open", "changedBy": "x" })),
      ),
      event(15, "item.changed", item(75, "done", json!({ "previousState": null, "changedBy": "x" }))),
    ];
  }

  pass(db, &vars).await.unwrap();
  pass(db, &vars).await.unwrap();

  let told = peers.lock().unwrap().instructed();

  assert_eq!(told.len(), 1, "only the close by somebody else, once: {told:?}");
  assert_eq!(
    (&told[0]["kind"], &told[0]["events"], &told[0]["room"], &told[0]["sort"]),
    (&json!("item-closed:71"), &json!(["item-closed:#71"]), &json!(-700), &json!("job"))
  );
  assert!(told[0]["said"]["order"].as_str().unwrap().contains("someone"));
  assert_eq!(settings::get(db, CURSOR_KEY).await.unwrap().as_deref(), Some("15"));
}

#[tokio::test]
async fn a_question_is_told_to_its_orderer_withdrawn_once_settled_and_told_to_the_owner_when_due() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;

  settings::set(db, CURSOR_KEY, "20").await.unwrap();

  {
    let mut held = peers.lock().unwrap();

    held.items.insert(80, item(80, "done", json!({ "scope": "-800" })));
    held.items.insert(81, item(81, "leased", json!({ "labels": { "raised-by": "80" } })));
    held.events = vec![
      event(
        21,
        "question.created",
        json!({ "id": 5, "itemId": 81, "text": "which one?", "choices": ["a", "b"] }),
      ),
      event(
        22,
        "question.created",
        json!({ "id": 6, "scope": "-900", "text": "on its own", "choices": [] }),
      ),
      event(
        22,
        "question.created",
        json!({ "id": 7, "itemId": 81, "text": "put by its asker", "choices": ["a", "b"], "toldByAsker": true }),
      ),
      event(
        23,
        "question.due",
        json!({ "id": 6, "scope": "-900", "text": "on its own", "choices": [] }),
      ),
      event(
        24,
        "reply.created",
        json!({ "id": 1, "questionId": 5, "settles": false, "text": "a word" }),
      ),
    ];
  }

  pass(db, &vars).await.unwrap();

  {
    let held = peers.lock().unwrap();
    let told = held.instructed();

    assert_eq!(told.len(), 2, "{told:?}");
    assert_eq!(
      (&told[0]["kind"], &told[0]["events"], &told[0]["room"]),
      (&json!("question-awaits-answer:5"), &json!(["question-awaits-answer:#5"]), &json!(-800)),
      "the question's item's orderer, up the chain"
    );
    assert!(told[0]["said"]["order"].as_str().unwrap().contains("which one?"));
    assert_eq!(told[1]["room"], json!(-900), "a question on its own: the room its scope names");

    let notices = held.noticed();

    assert_eq!(notices.len(), 1);
    assert_eq!(
      (&notices[0]["code"], &notices[0]["questionId"]),
      (&json!(JOB_NOTICES.question_due_code), &json!(6))
    );
    assert!(held.withdrawn().is_empty(), "a word that settles nothing withdraws nothing");
  }

  assert_eq!(
    settings::get(db, &format!("{QUESTION_TOLD_PREFIX}5")).await.unwrap().as_deref(),
    Some("1")
  );

  peers.lock().unwrap().events.push(event(
    25,
    "reply.created",
    json!({ "id": 2, "questionId": 5, "settles": true, "choice": "a" }),
  ));
  pass(db, &vars).await.unwrap();
  assert_eq!(settings::get(db, &format!("{QUESTION_TOLD_PREFIX}5")).await.unwrap(), None);

  let held = peers.lock().unwrap();

  assert_eq!(held.withdrawn(), ["1"], "the settled question's instruction withdrawn");
  assert_eq!(held.instructed().len(), 2, "nothing told twice");
  assert_eq!(held.noticed().len(), 1, "nothing told twice");
}

#[tokio::test]
async fn a_cursor_past_the_window_reads_again_from_the_oldest_telling_nothing_it_passed() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;

  settings::set(db, CURSOR_KEY, "5").await.unwrap();

  {
    let mut held = peers.lock().unwrap();

    held.window_from = 30;
    held.events = vec![closed(30, 71, Some("someone")), closed(31, 72, Some("someone"))];
  }

  pass(db, &vars).await.unwrap();

  assert!(peers.lock().unwrap().instructed().is_empty());
  assert_eq!(settings::get(db, CURSOR_KEY).await.unwrap().as_deref(), Some("31"));
}
