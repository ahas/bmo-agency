//! Notices owed across a cut receiver and a stopped sender (docs/agent-control.md, 「An item's change is told to
//! its orderer」): the job service's events read while the receiver cuts every call are owed, the cursor moves
//! past them, and each is told once the receiver answers again; a sender stopped in the middle of a telling
//! leaves the notice owed, the next start tells it, a standing one again at every start under its kind, until
//! its condition ends and the copy told last is withdrawn; and a job's end or a review's check whose row a stop cut
//! off owed by the sender once the grace is past.

use std::collections::BTreeMap;

use agency_server::outbox::{Keep, Notice, owe, owed, restarted};
use agency_server::queue::settings;
use agency_server::watches::job_events::{self, CURSOR_KEY};
use agency_server::watches::notices_owed::pass;
use policy::job_notices::JOB_NOTICES;
use policy::job_review::JOB_REVIEW;
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

// Room -120 is item 12's, -130 item 13's: the orderers of the jobs below.
async fn stand_items(peers: &Peers) {
  let mut held = peers.lock().unwrap();

  held.items.insert(12, item(12, "leased", json!({ "scope": "-120" })));
  held.items.insert(13, item(13, "leased", json!({ "scope": "-130" })));
}

#[tokio::test]
async fn a_job_s_end_whose_notice_a_stop_cut_off_is_owed_by_the_sender_once_and_a_fresh_one_is_left_to_its_path()
 {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;
  let now = agency_server::now_ms();

  stand_items(&peers).await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
           "outcome", "report", "started_at", "ended_at", "prompt")
         VALUES (8, 12, 1, 'runner', 1, 'w1', 'failed', 'failed', 'it broke', 1000, 2000, 'the work'),
                (9, 13, 2, 'runner', 2, 'w1', 'stopped', 'stopped', 'stopped', 1000, $1, 'the work')"#,
      vec![now.into()],
    )
    .await;

  pass(db, &vars).await.unwrap();

  let told = peers.lock().unwrap().instructed();

  assert_eq!(told.len(), 1, "only the end past its grace: {told:?}");
  assert_eq!((&told[0]["kind"], &told[0]["room"]), (&json!("job-ended:12"), &json!(-120)));
  assert_eq!(peers.lock().unwrap().noticed()[0]["code"], json!(JOB_NOTICES.failed_code));
  assert_eq!(
    scratch.count(r#"SELECT COUNT(*) FROM "jobs" WHERE "end_owed""#, vec![]).await,
    1,
    "the end owed is marked so"
  );

  pass(db, &vars).await.unwrap();
  assert_eq!(peers.lock().unwrap().instructed().len(), 1, "an end owed is not owed twice");
}

fn told(heard: &str, words: &str, at: i64) -> String {
  json!({ "heard": heard, "words": words, "at": at }).to_string()
}

#[tokio::test]
async fn a_check_due_whose_row_a_stop_cut_off_is_owed_by_the_sender_with_the_words_it_came_due_with() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;

  stand_items(&peers).await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
           "started_at", "prompt", "review_check", "review_words", "review_told")
         VALUES (7, 12, 1, 'runner', 1, 'w1', 'reviewing', 1000, 'the work', 'promises', 'the report', $1),
                (6, 13, 2, 'runner', 2, 'w1', 'reviewing', 1000, 'the work', 'promises', 'the report', $2)"#,
      vec![
        told("answer", "It was done so.", 1000).into(),
        told("report", "the report", agency_server::now_ms()).into(),
      ],
    )
    .await;

  pass(db, &vars).await.unwrap();

  let told = peers.lock().unwrap().instructed();

  assert_eq!(told.len(), 1, "only the check past its grace: {told:?}");
  assert_eq!((&told[0]["kind"], &told[0]["room"]), (&json!("job-review:12"), &json!(-120)));

  let order = told[0]["said"]["order"].as_str().unwrap();

  assert!(
    order.contains(&JOB_REVIEW.answer_heading) && order.contains("It was done so."),
    "{order}"
  );
  assert!(owed(db, "job-review:12").await.unwrap()[""].told_at.is_some(), "kept standing");

  pass(db, &vars).await.unwrap();
  assert_eq!(
    peers.lock().unwrap().instructed().len(),
    1,
    "a check owed is not owed again by the sender"
  );
}
