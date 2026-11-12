//! Notices owed across a cut receiver and a stopped sender (docs/agent-control.md, 「An item's change is told to
//! its orderer」): the job service's events read while the receiver cuts every call are owed, the cursor moves
//! past them, and each is told once the receiver answers again; a sender stopped in the middle of a telling
//! leaves the notice owed, the next start tells it, a standing one again at every start under its kind, until
//! its condition ends and the copy told last is withdrawn.

use std::collections::BTreeMap;

use agency_server::outbox::{Keep, Notice, owe, owed, restarted};
use agency_server::queue::settings;
use agency_server::watches::job_events::{self, CURSOR_KEY};
use agency_server::watches::notices_owed::pass;
use serde_json::{Value, json};

use crate::support::cut::{Front, Mode};
use crate::support::peers::{Peers, item, stand};
use crate::support::scratch::Scratch;

// The stand-in as the job service, and a front before it as the receiver.
async fn cut_receiver() -> (Peers, Front, BTreeMap<String, String>) {
  let (peers, mut vars) = stand().await;
  let front = Front::before(&vars["BMO_RECEIVER_URL"]).await;

  vars.insert("BMO_RECEIVER_URL".to_owned(), front.address.clone());

  (peers, front, vars)
}

fn event(id: i64, kind: &str, data: Value) -> Value {
  json!({ "id": id, "kind": kind, "at": id, "data": data })
}

fn kinds(peers: &Peers) -> Vec<Value> {
  peers.lock().unwrap().instructed().iter().map(|told| told["kind"].clone()).collect()
}

async fn failed_tries(scratch: &Scratch) -> i64 {
  scratch.count(r#"SELECT COUNT(*) FROM "notices" WHERE "tries" > 0"#, vec![]).await
}

#[tokio::test]
async fn events_read_while_the_receiver_cuts_every_call_are_owed_and_told_once_it_answers_again() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, front, vars) = cut_receiver().await;

  settings::set(db, CURSOR_KEY, "60").await.unwrap();

  {
    let mut held = peers.lock().unwrap();

    held.items.insert(82, item(82, "leased", json!({ "scope": "-820" })));
    held.events = vec![
      event(
        61,
        "question.created",
        json!({ "id": 9, "itemId": 82, "text": "which?", "choices": [] }),
      ),
      event(
        62,
        "item.changed",
        item(
          84,
          "done",
          json!({ "previousState": "leased", "changedBy": "someone", "scope": "-840" }),
        ),
      ),
    ];
  }

  front.set(Mode::Cut);
  job_events::pass(db, &vars).await.unwrap();

  assert_eq!(
    settings::get(db, CURSOR_KEY).await.unwrap().as_deref(),
    Some("62"),
    "the cursor moved past both"
  );
  assert!(front.accepted() >= 2, "each notice was tried and its connection cut");
  assert!(kinds(&peers).is_empty(), "nothing reached the receiver");
  assert_eq!(failed_tries(&scratch).await, 2, "both owed, each with its failed try");

  // The receiver is back; the rows wait out their wait, then agency's start makes them due at once.
  front.set(Mode::Pass);
  assert_eq!(pass(db, &vars).await.unwrap(), 0, "no row is tried before its wait is out");

  restarted(db).await.unwrap();
  assert_eq!(pass(db, &vars).await.unwrap(), 2);
  let mut told = kinds(&peers);

  told.sort_by_key(Value::to_string);
  assert_eq!(told, [json!("item-closed:84"), json!("question-awaits-answer:9")], "each told once");

  restarted(db).await.unwrap();
  assert_eq!(pass(db, &vars).await.unwrap(), 0, "a notice told once is not told again");
  assert!(owed(db, "item-closed:").await.unwrap().is_empty());
  assert!(
    owed(db, "question-awaits-answer:9").await.unwrap()[""].told_at.is_some(),
    "the question kept, told"
  );
}

fn check(item: i64) -> Value {
  json!({
    "room": -110,
    "said": { "order": "the check due" },
    "sort": "job",
    "kind": format!("job-review:{item}"),
    "summary": "the check due",
    "events": [format!("job-review:#{item}")],
  })
}

#[tokio::test]
async fn a_sender_stopped_mid_telling_leaves_the_check_owed_and_each_start_tells_it_until_it_is_no_longer_due()
 {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, front, vars) = cut_receiver().await;

  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
           "started_at", "prompt", "review_check")
         VALUES (7, 11, 1, 'runner', 1, 'w1', 'reviewing', 1000, 'the work', 'promises')"#,
      vec![],
    )
    .await;

  // The first telling hangs on the receiver, and the process telling it stops in the middle.
  front.set(Mode::Hold);

  let telling = {
    let (db, vars) = (db.clone(), vars.clone());

    tokio::spawn(async move { owe(&db, &vars, Notice::instruction(check(11), Keep::Standing)).await })
  };

  front.heard().await;
  telling.abort();
  assert!(telling.await.unwrap_err().is_cancelled());

  let stood = owed(db, "job-review:11").await.unwrap().remove("").unwrap();

  assert_eq!(stood.told_at, None, "the stopped telling left the check owed, untold");
  front.set(Mode::Pass);
  assert_eq!(
    pass(db, &vars).await.unwrap(),
    0,
    "the stopped sender's hold on the row stands until it lapses"
  );

  // The next process starts: the check is told; and each start after tells it again under the same kind.
  restarted(db).await.unwrap();
  assert_eq!(pass(db, &vars).await.unwrap(), 1);
  restarted(db).await.unwrap();
  assert_eq!(pass(db, &vars).await.unwrap(), 1);
  assert_eq!(kinds(&peers), [json!("job-review:11"), json!("job-review:11")]);

  // The orderer moves and the job's turn runs: the check is no longer due, let go and its last copy withdrawn.
  scratch
    .run(r#"UPDATE "jobs" SET "state" = 'running', "review_check" = NULL WHERE "id" = 7"#, vec![])
    .await;
  pass(db, &vars).await.unwrap();
  restarted(db).await.unwrap();
  assert_eq!(pass(db, &vars).await.unwrap(), 0, "a check no longer due is told no more");
  assert!(owed(db, "job-review:").await.unwrap().is_empty());
  assert_eq!(peers.lock().unwrap().withdrawn(), ["2"], "the copy told last withdrawn");
}
