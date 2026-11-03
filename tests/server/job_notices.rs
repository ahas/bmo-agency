//! What a job's orderer and the owner are told of the job (docs/agent-control.md, 「An item's change is told to
//! its orderer」): a job's wait as it begins, a job's end told to the orderer of a derived item up the chain,
//! and a failed job told to the owner as well where its orderer is another room.

use agency_server::job_end::delivered_with;
use agency_server::job_notices::{failed, owner_hears_failure, question_due, tell_waiting};
use agency_server::queue::orderer::Orderer;
use policy::job_notices::JOB_NOTICES;
use serde_json::{Value, json};

use crate::support::peers::{item, stand};

fn member(item: i64, outcome: Option<&str>, room: Option<i64>) -> Value {
  json!({
    "id": 77,
    "itemId": item,
    "leaseId": 0,
    "workerName": "w1",
    "roomId": room,
    "state": "ended",
    "outcome": outcome,
    "report": "what the run left",
  })
}

#[tokio::test]
async fn a_job_s_wait_is_told_to_its_orderer_in_the_room_up_the_chain() {
  let (peers, vars) = stand().await;

  {
    let mut held = peers.lock().unwrap();

    held.items.insert(40, item(40, "done", json!({ "scope": "-400" })));
    held.items.insert(41, item(41, "leased", json!({ "raisedBy": 40 })));
    held.items.insert(42, item(42, "leased", json!({})));
  }

  tell_waiting(&vars, 41, "w1", "no provider of the order has quota").await;
  tell_waiting(&vars, 42, "w1", "it waits for a slot of the worker role").await;

  let told = peers.lock().unwrap().instructed();

  assert_eq!(told.len(), 2, "{told:?}");
  assert_eq!(
    (&told[0]["kind"], &told[0]["events"], &told[0]["room"], &told[0]["sort"]),
    (&json!("job-waiting:41"), &json!(["job-waiting:#41"]), &json!(-400), &json!("job"))
  );
  assert!(
    told[0]["said"]["order"]
      .as_str()
      .unwrap()
      .contains("no provider of the order has quota")
  );
  assert_eq!(
    told[1]["room"],
    Value::Null,
    "an item naming no room and no parent: the owner's private room"
  );
}

#[tokio::test]
async fn a_job_s_end_on_a_derived_item_is_told_to_the_orderer_of_its_parent_s_parent() {
  let (peers, vars) = stand().await;

  {
    let mut held = peers.lock().unwrap();

    held
      .items
      .insert(50, item(50, "done", json!({ "scope": "-500", "requesterName": "agent-a" })));
    held.items.insert(51, item(51, "done", json!({ "labels": { "raised-by": "50" } })));
    held.items.insert(52, item(52, "leased", json!({ "raisedBy": 51 })));
  }

  delivered_with(&vars, &member(52, None, None)).await;

  let held = peers.lock().unwrap();
  let told = held.instructed();

  assert_eq!(told.len(), 1, "{told:?}");
  assert_eq!(
    (&told[0]["kind"], &told[0]["room"], &told[0]["agentName"]),
    (&json!("job-ended:52"), &json!(-500), &json!("agent-a"))
  );
  assert!(held.noticed().is_empty(), "an end that is no failure is no owner notice");
}

#[tokio::test]
async fn a_failed_job_in_another_room_is_told_to_the_owner_as_well_and_not_in_his_own() {
  let (peers, vars) = stand().await;

  peers.lock().unwrap().items.insert(60, item(60, "leased", json!({ "scope": "-600" })));
  peers.lock().unwrap().items.insert(61, item(61, "leased", json!({})));

  delivered_with(&vars, &member(60, Some("failed"), None)).await;
  delivered_with(&vars, &member(61, Some("blocked"), None)).await;

  let held = peers.lock().unwrap();
  let notices = held.noticed();

  assert_eq!(held.instructed().len(), 2, "each end told to its orderer");
  assert_eq!(notices.len(), 1, "the owner's own room hears it once: {notices:?}");
  assert_eq!(
    (&notices[0]["code"], &notices[0]["itemId"], &notices[0]["room"], &notices[0]["outcome"]),
    (&json!(JOB_NOTICES.failed_code), &json!(60), &json!(-600), &json!("failed"))
  );
}

#[test]
fn a_failure_is_the_owner_s_only_for_failed_or_blocked_in_another_room() {
  let room = Orderer {
    room: Some(-1),
    requester: None,
  };

  assert!(owner_hears_failure(&member(1, Some("failed"), None), &room));
  assert!(owner_hears_failure(&member(1, Some("blocked"), None), &room));
  assert!(!owner_hears_failure(&member(1, Some("stopped"), None), &room));
  assert!(!owner_hears_failure(&member(1, None, None), &room));
  assert!(!owner_hears_failure(&member(1, Some("failed"), None), &Orderer::default()));
  assert_eq!(failed(&member(1, Some("failed"), None), Some(-1))["jobId"], 77);
}

#[test]
fn a_question_past_its_due_time_is_an_owner_notice_carrying_the_question() {
  let said = question_due(&json!({
    "id": 9, "itemId": 3, "text": "which one?", "choices": ["a", "b"], "askerName": "harness", "dueAt": 5,
  }));

  assert_eq!(said["code"], JOB_NOTICES.question_due_code.as_str());
  assert_eq!(
    (&said["questionId"], &said["itemId"], &said["text"]),
    (&json!(9), &json!(3), &json!("which one?"))
  );
  assert_eq!(said["choices"], json!(["a", "b"]));
}
