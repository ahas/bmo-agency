//! A job's review (docs/job-review.md): an answered turn puts the job under review rather than ending it, its
//! checkout and lease kept; the orderer's moves refused where they do not fit — no turn, a job not under review,
//! the owner asked before the ceiling; the checks passed one at a time, a job of one stage ending once its last
//! check passes, a job of several going on to its next stage; and a rejection counted, its round moved on and its
//! rework turned out; and the seat a job holds kept through its review and across a restart, so its next stage
//! runs on its agent before a job waiting for that agent starts.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::work;
use agency_server::control::{Front, router};
use agency_server::now_ms;
use agency_server::order_run::dispatch::{self, Asked, Wanted};
use agency_server::places::Ground;
use agency_server::turn_marks::{self, Mark};
use harness_providers::Machine;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::support::grounds::{deepseek_balance, seeded};
use crate::support::rows::repository;
use crate::support::scratch::Scratch;

const TOKEN: &str = "secret";

fn ground(scratch: &Scratch, home: &std::path::Path) -> Arc<Ground> {
  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), home.to_string_lossy().into_owned()),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
    ],
    home.join("data"),
  );

  Arc::new(Ground::new(&service, &machine, None))
}

async fn front(ground: Arc<Ground>) -> u16 {
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

// A repository's checkout with one running job on an item in `stages` stages, its assignment running.
async fn running(scratch: &Scratch, worker: &str, job: i64, item: i64, stages: &[&str]) {
  let now = now_ms();

  repository(scratch.db(), worker, &format!("/checkouts/{worker}"), &[]).await;
  scratch
    .run(
      r#"INSERT INTO "assignments" ("id", "item_id", "holder_name", "worker_name", "state", "created_at",
           "changed_at")
         VALUES ($1, $2, 'harness', $3, 'running', $4, $4)"#,
      vec![job.into(), item.into(), worker.into(), now.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
           "state", "started_at", "prompt", "review_stages", "review_stage", "review_turn")
         VALUES ($1, $2, 0, 'harness', $1, $3, 'running', $4, 'work', $5, 1, 'work')"#,
      vec![
        job.into(),
        item.into(),
        worker.into(),
        now.into(),
        serde_json::to_string(stages).unwrap().into(),
      ],
    )
    .await;
}

async fn read(port: u16, job: i64) -> Value {
  call(port, reqwest::Method::GET, &format!("/jobs/{job}"), None).await.1
}

async fn review(port: u16, agent: &str, job: i64, action: &str, text: Option<&str>) -> (u16, Value) {
  call(
    port,
    reqwest::Method::POST,
    "/job-reviews",
    Some(json!({ "agentName": agent, "jobId": job, "action": action, "text": text })),
  )
  .await
}

// The seat of the agent a job's turn was let in on, standing let in as a job's slot request does.
async fn seated(scratch: &Scratch, job: i64) {
  scratch
    .run(
      r#"INSERT INTO "role_slot_requests" ("token", "request_key", "owner", "source", "role", "path", "provider",
           "model", "effort", "capacity", "state", "enqueued_at", "class", "seat")
         VALUES ($1, $1, $2, 'job', 'coder', '/checkouts/', 'deepseek', 'deepseek-flash', '', 1, 'active', 0,
           'foreground', 'coder-05')"#,
      vec![format!("seat-{job}").into(), format!("job-{job}").into()],
    )
    .await;
}

async fn seat_state(scratch: &Scratch, job: i64) -> i64 {
  scratch
    .count(
      r#"SELECT COUNT(*) FROM "role_slot_requests" WHERE "owner" = $1 AND "state" = 'active'"#,
      vec![format!("job-{job}").into()],
    )
    .await
}

// Waits until the job stands in `state`, however its turn comes to it.
async fn until_state(port: u16, job: i64, state: &str) -> Value {
  for _ in 0..200 {
    let member = read(port, job).await;

    if member["state"] == state {
      return member;
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
  }

  panic!("job {job} never stood {state}: {}", read(port, job).await);
}

fn carrying(agent: &str, run: &str) {
  turn_marks::carrying(
    agent,
    run,
    Mark {
      chat_id: -5,
      privileged: false,
    },
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answered_turn_puts_the_job_under_review_and_keeps_its_checkout() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path());
  let port = front(ground.clone()).await;

  running(&scratch, "worker-r", 21, 61, &[]).await;
  seated(&scratch, 21).await;
  agency_server::job_review::reported(&ground, 21, "Done: the guard is mended and its test passes.", None)
    .await;

  // Its agent answers for it until it ends: the seat its turn was let in on stays the job's under review.
  assert_eq!(seat_state(&scratch, 21).await, 1);

  let member = read(port, 21).await;

  assert_eq!(member["state"], "reviewing", "{member}");
  assert_eq!(member["report"], "Done: the guard is mended and its test passes.");
  assert_eq!(
    (&member["review"]["check"], &member["review"]["round"], &member["review"]["rejections"]),
    (&json!("promises"), &json!(1), &json!(0))
  );

  // The job keeps its checkout while it is under review.
  let (_, view) = call(port, reqwest::Method::GET, "/repositories/worker-r", None).await;

  assert!(!view["checkouts"][0]["job"].is_null(), "{view}");
  assert_eq!(call(port, reqwest::Method::GET, "/assignments/21", None).await.1["state"], "running");

  let (_, listed) = call(port, reqwest::Method::GET, "/jobs?running=true", None).await;

  assert_eq!(listed["items"][0]["id"], 21, "{listed}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_checks_pass_one_at_a_time_and_a_job_of_one_stage_ends_after_its_last() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path());
  let port = front(ground.clone()).await;
  let agent = "reviewing-chat-agent";

  running(&scratch, "worker-s", 22, 62, &[]).await;
  agency_server::job_review::reported(&ground, 22, "All of it is done.", None).await;

  // No turn, no move.
  let (status, refused) = review(port, agent, 22, "pass", None).await;

  assert_eq!((status, refused["code"].as_str()), (409, Some("no-turn")), "{refused}");
  carrying(agent, "run-r1");

  // A move outside the four, and one that needs words and names none, are refused.
  let (status, refused) = review(port, agent, 22, "approve", None).await;

  assert_eq!((status, refused["code"].as_str()), (400, Some("bad-request")), "{refused}");

  let (status, refused) = review(port, agent, 22, "reject", None).await;

  assert_eq!((status, refused["code"].as_str()), (400, Some("bad-request")), "{refused}");

  // The owner is not asked before the work was sent back as often as the ceiling.
  let (status, refused) = review(port, agent, 22, "escalate", Some("Is this his call?")).await;

  assert_eq!((status, refused["code"].as_str()), (409, Some("escalation-early")), "{refused}");

  // The first check passed brings the second; the second passed ends a job of one stage with its report.
  let (status, member) = review(port, agent, 22, "pass", Some("Every promise is kept.")).await;

  assert_eq!(status, 200, "{member}");
  assert_eq!(
    (&member["state"], &member["review"]["check"]),
    (&json!("reviewing"), &json!("decisions"))
  );

  let (status, member) = review(port, agent, 22, "pass", None).await;

  assert_eq!(status, 200, "{member}");
  assert_eq!((&member["state"], &member["report"]), (&json!("ended"), &json!("All of it is done.")));
  assert_eq!(call(port, reqwest::Method::GET, "/assignments/22", None).await.1["state"], "released");

  // A job no longer under review takes no move.
  let (status, refused) = review(port, agent, 22, "pass", None).await;

  assert_eq!((status, refused["code"].as_str()), (409, Some("not-reviewing")), "{refused}");
  turn_marks::ended(agent, "run-r1");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rejection_is_counted_and_a_stage_passed_hands_the_next_one_over() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path());
  let port = front(ground.clone()).await;
  let agent = "staging-chat-agent";

  carrying(agent, "run-s1");

  // Sent back: counted, the round moves on, and the rework goes out as a turn of the job.
  running(&scratch, "worker-t", 23, 63, &[]).await;
  agency_server::job_review::reported(&ground, 23, "Done, but the docs are for later.", None).await;

  let (status, member) =
    review(port, agent, 23, "reject", Some("The docs were part of the brief: write them now.")).await;

  assert_eq!(status, 200, "{member}");
  assert_eq!((&member["review"]["rejections"], &member["review"]["round"]), (&json!(1), &json!(2)));

  // This ground stands no places, so the rework's turn fails as it goes out; that it went out is what is read.
  until_state(port, 23, "stopped").await;

  // Two stages: both checks of the first passed hand the second over as the next turn.
  running(&scratch, "worker-u", 24, 64, &["Change the screen", "Check it with the design skill"]).await;
  agency_server::job_review::reported(&ground, 24, "The screen is changed.", None).await;
  review(port, agent, 24, "pass", None).await;

  let (status, member) = review(port, agent, 24, "pass", None).await;

  assert_eq!(status, 200, "{member}");
  assert_eq!((&member["review"]["stage"], &member["review"]["round"]), (&json!(2), &json!(1)));
  assert_eq!(
    member["review"]["stages"],
    json!(["Change the screen", "Check it with the design skill"])
  );
  turn_marks::ended(agent, "run-s1");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_owner_s_word_holds_the_review_gives_the_checkout_up_past_its_wait_and_comes_back_as_a_check() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path());
  let port = front(ground.clone()).await;
  let agent = "escalating-chat-agent";
  let vars = std::collections::BTreeMap::new();

  carrying(agent, "run-e1");
  running(&scratch, "worker-v", 25, 65, &[]).await;
  seated(&scratch, 25).await;
  agency_server::job_review::reported(&ground, 25, "Done, but one matter is the owner's.", None).await;

  // Past the ceiling the work may be put to the owner: refused here only because this ground names no job
  // service to ask the question in, never as too early.
  scratch.run(r#"UPDATE "jobs" SET "rejections" = 10 WHERE "id" = 25"#, vec![]).await;

  let (status, refused) = review(port, agent, 25, "escalate", Some("Is the colour his call?")).await;

  assert_eq!((status, refused["code"].as_str()), (502, Some("upstream-failed")), "{refused}");

  // While the owner's question waits, nothing of the review moves.
  scratch.run(r#"UPDATE "jobs" SET "review_question" = 77 WHERE "id" = 25"#, vec![]).await;

  let (status, refused) = review(port, agent, 25, "pass", None).await;

  assert_eq!((status, refused["code"].as_str()), (409, Some("awaits-owner")), "{refused}");

  // Past its due time the job gives its checkout up, its item still its own.
  agency_server::job_review::question_due(scratch.db(), &vars, 77).await;

  let member = read(port, 25).await;

  assert_eq!(
    (&member["state"], &member["review"]["parked"]),
    (&json!("reviewing"), &json!(true)),
    "{member}"
  );
  assert_eq!(call(port, reqwest::Method::GET, "/assignments/25", None).await.1["state"], "released");

  // Paused so, it answers for its agent no longer: the seat is given back for a job waiting on that agent.
  assert_eq!(seat_state(&scratch, 25).await, 1);
  agency_server::control::work::paused_seats_given_back(&ground).await;
  assert_eq!(seat_state(&scratch, 25).await, 0);

  // The owner's answer comes back as the check due, and the review moves again.
  agency_server::job_review::ruled(
    scratch.db(),
    &vars,
    &json!({ "questionId": 77, "choice": "Send the work back", "text": "Settle the colour yourself." }),
  )
  .await;

  assert_eq!(read(port, 25).await["review"]["question"], Value::Null);

  // Sent back, the job takes its checkout again before its turn goes out.
  let (status, member) =
    review(port, agent, 25, "reject", Some("The owner says: settle the colour yourself.")).await;

  assert_eq!(status, 200, "{member}");

  let member = until_state(port, 25, "stopped").await;

  assert_eq!(member["review"]["parked"], false, "{member}");
  turn_marks::ended(agent, "run-e1");
}

// One turn's ask of a job's seat, as the job's turn asks it: under the job's owner, at its checkout, naming the
// one agent its item names.
fn turn_ask(job: i64, turn: i64, agent: &str) -> Asked<'_> {
  Asked {
    request_key: format!("job-{job}-{turn}-1"),
    owner: format!("{}{job}", dispatch::JOB_OWNER),
    wanted: Wanted {
      role: "worker",
      path: Some("/checkouts/seated"),
      fixed: None,
      agent: Some(agent),
    },
    ..Asked::default()
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_s_next_stage_runs_on_its_agent_before_a_job_waiting_for_that_agent_starts() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let db = &ground.db;

  deepseek_balance(&ground, 5.0).await;

  let agents = agency_server::role_agents::of_role(db, "worker").await.unwrap();
  let agent = agents[0].name.as_str();
  let active = async |token: &str| dispatch::request(db, token).await.unwrap().unwrap().state == "active";

  // The first job's first stage is let in on the agent its item names.
  running(&scratch, "worker-first", 31, 71, &["Implement", "Verify"]).await;

  let first = work::job_seat(&ground, 31, &turn_ask(31, 1, agent)).await.unwrap().unwrap();

  assert_eq!(first.seat.as_deref(), Some(agent));

  // A second job naming the same agent waits for it.
  let second =
    dispatch::enqueue(db, &ground.events, &turn_ask(32, 1, agent), dispatch::JOB, async |_, _| Ok(()))
      .await
      .unwrap();

  assert!(dispatch::acquire(&ground, &second).await.unwrap().is_none());

  // The first stage's turn runs, its run seen standing by the line's pass, then ends with its report: the pass
  // reads that run gone, and agency restarts while the job stands under review.
  let mut standing = std::collections::HashSet::new();

  standing.insert("job-31".to_owned());

  for seen in [standing, std::collections::HashSet::new()] {
    let leases = dispatch::bound_leases(db).await.unwrap();

    dispatch::release_ended(db, &ground.events, &leases, &seen).await.unwrap();
  }

  agency_server::job_review::reported(&ground, 31, "Stage 1 is implemented.", None).await;
  dispatch::release_every_session(db, &ground.events).await.unwrap();
  work::paused_seats_given_back(&ground).await;

  assert!(active(&first.token).await, "the first job keeps its seat under review");

  let asked_again = dispatch::request(db, &second.token).await.unwrap().unwrap();

  assert!(
    dispatch::acquire(&ground, &asked_again).await.unwrap().is_none(),
    "the waiting job is not let in on a seat a reviewing job holds"
  );

  // The first job's next stage goes out on the seat it holds, asking nothing, while the second still waits.
  let next = work::job_seat(&ground, 31, &turn_ask(31, 2, agent)).await.unwrap().unwrap();

  assert_eq!(next.token, first.token, "the next stage runs on the seat its first stage held");
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "role_slot_requests" WHERE "owner" = 'job-31'"#, vec![])
      .await,
    1,
    "the next stage asks for no slot of its own"
  );
  assert_eq!(dispatch::request(db, &second.token).await.unwrap().unwrap().state, "waiting");

  // Only the first job's end gives the agent to the job that waited for it.
  agency_server::job_end::slot_given_back(db, &ground.events, 31, "ended").await;

  let asked_again = dispatch::request(db, &second.token).await.unwrap().unwrap();
  let let_in = dispatch::acquire(&ground, &asked_again).await.unwrap().unwrap();

  assert_eq!(let_in.seat.as_deref(), Some(agent));
}
