//! The handoff a job's session leaves (docs/sessions.md, 「The handoff a job's session leaves」; docs/api.md,
//! 「assignments and jobs」): what a session's answer gives, the run log standing in for a session that cannot
//! be asked, a prompt opened on a handoff with the item's title kept first; and a stopped job's answer to the
//! stop words noted on its item as its handoff, which the item's next job opens on, on another agent's launch
//! as much as the same one's.

use std::sync::Arc;
use std::time::Duration;

use agency_server::control::{Front, router};
use agency_server::door::execution::cancelled;
use agency_server::job_handoff::{Handoff, Source, ahead, answered, from_log};
use agency_server::lines::place::Place;
use agency_server::now_ms;
use agency_server::run_log::lines::Line;
use agency_server::run_log::{RunLog, Target};
use agency_server::turn_pickup::{InFlight, at_start, keep};
use policy::job_handoff::JOB_HANDOFF;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::grounds::seeded_with;
use crate::support::rows::repository;
use crate::support::scratch::Scratch;

const TOKEN: &str = "secret";

#[test]
fn a_session_s_answer_is_its_handoff_unless_it_answered_nothing_or_none() {
  assert_eq!(answered(None), None);
  assert_eq!(answered(Some("  \n")), None);
  assert_eq!(answered(Some(" None. ")), None);
  assert_eq!(
    answered(Some("\nThe parser is fixed; its tests pass.\n")),
    Some(Handoff {
      text: "The parser is fixed; its tests pass.".to_owned(),
      source: Source::Session,
    })
  );
}

#[test]
fn a_prompt_opened_on_a_handoff_keeps_the_item_s_title_first() {
  let opened = ahead("The parser is fixed.", "# An item\n\n## Instruction\n\nDo the work.");

  assert!(opened.starts_with("# An item\n\n## The last session's handoff\n"), "{opened}");
  assert!(
    opened.find("The parser is fixed.").unwrap() < opened.find("## Instruction").unwrap(),
    "{opened}"
  );
  assert!(opened.ends_with("## Instruction\n\nDo the work."), "{opened}");

  // A prompt that opens on anything but a title is opened on the handoff whole.
  let continued = ahead("The parser is fixed.", "This job already ran on deepseek.\n\n# An item");

  assert!(continued.starts_with("## The last session's handoff\n"), "{continued}");
  assert!(continued.ends_with("This job already ran on deepseek.\n\n# An item"), "{continued}");
}

#[tokio::test]
async fn a_session_that_cannot_be_asked_leaves_its_run_log_s_last_lines_oldest_first() {
  let scratch = Scratch::new().await;
  let mut log = RunLog::new(Target {
    agent: "helper".to_owned(),
    run: "job-7".to_owned(),
    chat_id: None,
    job_id: Some(7),
  });

  for said in ["Read docs/area.md.", "Edited src/area.rs.", "The area tests fail on a missing field."] {
    log.write(scratch.db(), Line::note(said.to_owned())).await;
  }

  let handoff = from_log(scratch.db(), 7, &JOB_HANDOFF.why.road_stopped).await;

  assert_eq!(handoff.source, Source::Log);
  assert!(handoff.text.contains(&JOB_HANDOFF.why.road_stopped), "{}", handoff.text);

  let read = handoff.text.find("Read docs/area.md.").expect("the first line");
  let edited = handoff.text.find("Edited src/area.rs.").expect("the second line");
  let failing = handoff.text.find("The area tests fail on a missing field.").expect("the last line");

  assert!(read < edited && edited < failing, "{}", handoff.text);
}

// The control front on the ground, on a loopback port of its own.
async fn front(ground: Arc<agency_server::places::Ground>) -> u16 {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let port = listener.local_addr().unwrap().port();
  let app = router(Front::new(TOKEN.to_owned(), ground).unwrap());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  port
}

async fn call(port: u16, method: reqwest::Method, path: &str, body: Option<Value>) -> (u16, Value) {
  let mut request = reqwest::Client::new()
    .request(method, format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(TOKEN);

  if let Some(body) = body {
    request = request.json(&body);
  }

  let answer = request.send().await.unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_job_s_answer_is_noted_as_its_handoff_and_the_item_s_next_job_opens_on_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (peers, vars) = crate::support::peers::stand().await;
  let ground = Arc::new(seeded_with(scratch.db(), home.path(), &vars).await);
  let port = front(ground.clone()).await;
  let place = Place::of(&ground.roots.data, "worker-01");
  let handoff = "Done: the parser reads the new field. Left: its tests in tests/area.rs, uncommitted.";

  peers.lock().unwrap().leases.insert(1);
  peers
    .lock()
    .unwrap()
    .items
    .insert(42, crate::support::peers::item(42, "open", json!({})));
  repository(scratch.db(), "helper", &home.path().join("helper").to_string_lossy(), &[]).await;
  std::fs::create_dir_all(home.path().join("helper")).unwrap();

  // A running job of item 42 whose turn goes on in its program, picked up as agency's start picks it up.
  let now = now_ms();

  scratch
    .run(
      r#"INSERT INTO "assignments" ("id", "item_id", "holder_name", "worker_name", "state", "created_at",
           "changed_at")
         VALUES (8, 42, 'harness', 'helper', 'running', $1, $1)"#,
      vec![now.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
           "run_name", "state", "started_at", "prompt")
         VALUES (8, 42, 1, 'harness', 8, 'helper', 'job-8', 'running', $1, '# item 42')"#,
      vec![now.into()],
    )
    .await;
  keep(
    scratch.db(),
    &InFlight {
      session: "worker-01@worker".into(),
      turn: 1,
      run: "job-8".into(),
      key: None,
      agent: "worker-01".into(),
      chat_id: None,
      ledger: None,
      job: Some(8),
      started_at: now - 1_000,
    },
  )
  .await;

  let append = |line: Value| {
    use std::io::Write as _;

    std::fs::create_dir_all(&place.path).unwrap();
    let mut file = std::fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open(place.path.join("loop.jsonl"))
      .unwrap();

    writeln!(file, "{line}").unwrap();
  };

  append(
    json!({ "kind": "start", "id": "j", "name": "job-8", "title": "a turn", "at": 1, "typed": false }),
  );
  at_start(&ground).await.unwrap();

  // The program answers the stop words with its handoff once the stop reaches it, and ends.
  let db = scratch.db().clone();
  let program = tokio::spawn({
    let file = place.path.join("loop.jsonl");

    async move {
      for _ in 0..200 {
        if cancelled(&db, &["job-8"]).await.unwrap() {
          use std::io::Write as _;

          let mut file = std::fs::OpenOptions::new().append(true).open(&file).unwrap();

          writeln!(file, "{}", json!({ "kind": "answer", "id": "j", "text": handoff })).unwrap();
          writeln!(file, "{}", json!({ "kind": "end", "id": "j" })).unwrap();
          return true;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
      }

      false
    }
  });
  let (status, stopped) = call(
    port,
    reqwest::Method::PATCH,
    "/jobs/8",
    Some(json!({ "state": "stopped", "report": "Stopped for the bench." })),
  )
  .await;

  assert!(program.await.unwrap(), "the stop reached the job's turn");
  assert_eq!((status, stopped["state"].as_str()), (200, Some("stopped")), "{stopped}");

  // Its answer stands on the item as a note of the handoff's own kind.
  let notes = peers.lock().unwrap().notes.get(&42).cloned().unwrap_or_default();

  assert_eq!(notes.len(), 1, "{notes:?}");
  assert_eq!(notes[0]["text"], handoff);
  assert_eq!(notes[0]["labels"], json!({ "kind": "handoff", "job": "8", "source": "session" }));

  // The item's next job, launched afresh, opens on it: the title first, the handoff ahead of the rest, and the
  // handoff no note of its own below.
  let (status, assignment) = call(
    port,
    reqwest::Method::POST,
    "/assignments",
    Some(json!({ "itemId": 42, "holderName": "harness", "assigneeName": "helper",
                 "item": { "title": "item 42", "brief": "Carry on.", "notes": [] } })),
  )
  .await;

  assert_eq!(status, 201, "{assignment}");

  let (status, created) = call(
    port,
    reqwest::Method::POST,
    "/jobs",
    Some(json!({
      "assignmentId": assignment["id"],
      "itemId": 42,
      "leaseId": 1,
      "holderName": "harness",
      "item": { "title": "item 42", "brief": "Carry on.", "notes": notes, "labels": { "agent": "worker-01" } },
    })),
  )
  .await;

  assert_eq!(status, 201, "{created}");

  let (_, prompt) =
    call(port, reqwest::Method::GET, &format!("/jobs/{}/prompt", created["id"]), None).await;
  let prompt = prompt["text"].as_str().unwrap_or_default();

  assert!(prompt.starts_with("# item 42\n\n## The last session's handoff\n"), "{prompt}");
  assert!(prompt.find(handoff).unwrap() < prompt.find("Carry on.").unwrap(), "{prompt}");
  assert_eq!(prompt.matches(handoff).count(), 1, "{prompt}");
  assert!(!prompt.contains("## Notes"), "{prompt}");

  let _ = call(
    port,
    reqwest::Method::PATCH,
    &format!("/jobs/{}", created["id"]),
    Some(json!({ "state": "stopped", "report": "The test is over." })),
  )
  .await;
}
