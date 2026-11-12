//! Which jobs stand in the machine's turn (docs/role-agents.md, 「When an agent is busy」): a job testing or
//! waiting to test leaves its agent free, so the agent's next item is offered and let in beside it, and the
//! first job goes on with its seat once its test ends.

use agency_server::order_run::dispatch::{self, Asked, Wanted};
use agency_server::watches::machine_turn::{Reading, mark};
use agency_server::watches::queue_idle::{Open, awaiting_launch, held};

use crate::support::grounds::{deepseek_balance, seeded};
use crate::support::rows::repository;
use crate::support::scratch::Scratch;

fn ask<'a>(job: i64, path: &'a str, agent: &'a str) -> Asked<'a> {
  Asked {
    request_key: format!("job-{job}-1-1"),
    owner: format!("{}{job}", dispatch::JOB_OWNER),
    wanted: Wanted {
      role: "worker",
      path: Some(path),
      fixed: None,
      agent: Some(agent),
    },
    place_key: Some(path),
    ..Asked::default()
  }
}

fn item_for(id: i64, assignee: &str, agent: &str) -> Open {
  Open {
    id,
    title: format!("item {id}"),
    assignee: Some(assignee.to_owned()),
    agent: Some(agent.to_owned()),
    queue: "todo".to_owned(),
    ..Open::default()
  }
}

#[test]
fn the_testbed_names_a_checkout_by_its_root_and_a_handed_over_commit_frees_only_a_job_under_review() {
  let printed = r#"{
    "queues": { "items": [
      { "name": "wide", "width": 24,
        "holder": { "scope": null, "checkout": "/checkouts/first/", "command": "cargo test", "pid": 1,
                    "askedAt": 1, "heldAt": 2, "kept": false, "front": false, "next": false },
        "waiting": [
          { "scope": null, "checkout": "/checkouts/third/crates/store", "command": "cargo check", "pid": 2,
            "askedAt": 3, "heldAt": null, "kept": false, "front": true, "next": false } ] },
      { "name": "narrow", "width": 1, "holder": null, "waiting": [] } ], "more": false },
    "closeLine": { "holder": null,
                   "waiting": [ { "scope": null, "checkout": "/checkouts/fourth", "askedAt": 4, "heldAt": null,
                                  "pid": 3 } ] }
  }"#;
  let reading = Reading {
    checkouts: Reading::testing_in(printed).unwrap(),
    awaiting_live: [71].into(),
  };

  assert!(reading.holds("running", Some("/checkouts/first"), 1), "a lane's holder");
  assert!(
    reading.holds("running", Some("/checkouts/third/"), 2),
    "a waiter in a folder under the checkout"
  );
  assert!(reading.holds("running", Some("/checkouts/fourth"), 3), "a waiter in the close line");
  assert!(!reading.holds("running", Some("/checkouts/fir"), 4), "a name that only begins the same");
  assert!(
    !reading.holds("running", Some("/checkouts/second"), 5),
    "a checkout the testbed does not name"
  );
  assert!(reading.holds("reviewing", None, 71), "a reviewed job whose commit waits to go live");
  assert!(!reading.holds("running", None, 71), "a job back at work while its commit still waits");
  assert!(Reading::testing_in("not json").is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_whose_only_job_waits_in_the_testbed_s_line_is_offered_its_next_item_and_let_in_on_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let db = &ground.db;

  deepseek_balance(&ground, 5.0).await;
  repository(db, "first", "/checkouts/first", &[]).await;
  repository(db, "second", "/checkouts/second", &[]).await;

  let agents = agency_server::role_agents::of_role(db, "worker").await.unwrap();
  let agent = agents[0].name.as_str();
  let state = async |token: &str| dispatch::request(db, token).await.unwrap().unwrap().state;
  let offered = async || {
    let standing = held(db, vec![item_for(52, "second", agent)]).await.unwrap();

    awaiting_launch(&standing).iter().map(|item| item.id).collect::<Vec<_>>()
  };

  // The agent's only job runs in its checkout, let in on the agent.
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
           "state", "started_at", "prompt", "item_agent")
         VALUES (41, 51, 0, 'harness', 0, 'first', 'running', 0, 'work', $1)"#,
      vec![agent.into()],
    )
    .await;

  let first = agency_server::control::work::job_seat(&ground, 41, &ask(41, "/checkouts/first", agent))
    .await
    .unwrap()
    .unwrap();

  assert_eq!(first.seat.as_deref(), Some(agent));

  // While it writes, the agent is busy: its next item is not offered, and a job asking for it waits.
  assert!(offered().await.is_empty(), "a busy agent's next item is not offered");

  let second = dispatch::enqueue(
    db,
    &ground.events,
    &ask(42, "/checkouts/second", agent),
    dispatch::JOB,
    async |_, _| Ok(()),
  )
  .await
  .unwrap();

  assert!(dispatch::acquire(&ground, &second).await.unwrap().is_none());

  // Its checkout waits in the testbed's line: the agent is free, its next item offered and let in beside it.
  let waiting_to_test = Reading {
    checkouts: ["/checkouts/first".to_owned()].into(),
    ..Reading::default()
  };

  assert_eq!(mark(db, &waiting_to_test).await.unwrap(), vec![41]);
  assert_eq!(offered().await, vec![52], "the free agent's next item is offered");

  let asked_again = dispatch::request(db, &second.token).await.unwrap().unwrap();
  let let_in = dispatch::acquire(&ground, &asked_again).await.unwrap().unwrap();

  assert_eq!(let_in.seat.as_deref(), Some(agent), "the second job is let in on the same agent");
  assert_eq!(state(&first.token).await, "active", "the first job keeps its seat while it tests");

  // The job of the item offered stands running in its own checkout.
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
           "state", "started_at", "prompt", "item_agent")
         VALUES (42, 52, 0, 'harness', 0, 'second', 'running', 0, 'work', $1)"#,
      vec![agent.into()],
    )
    .await;

  // With both jobs in the testbed's line the agent is free; once the first one's test ends it goes on on its
  // seat, and that job alone makes the agent busy again.
  let busy = async || {
    agency_server::role_agents::busy(db, "worker")
      .await
      .unwrap()
      .contains(&agent.to_owned())
  };
  let both = Reading {
    checkouts: ["/checkouts/first".to_owned(), "/checkouts/second".to_owned()].into(),
    ..Reading::default()
  };

  assert_eq!(mark(db, &both).await.unwrap(), vec![42]);
  assert!(!busy().await, "an agent whose every job tests is free");

  let first_tested = Reading {
    checkouts: ["/checkouts/second".to_owned()].into(),
    ..Reading::default()
  };

  assert_eq!(mark(db, &first_tested).await.unwrap(), vec![41]);
  assert_eq!(state(&first.token).await, "active", "the first job goes on on its seat");
  assert!(busy().await, "an agent whose job is back at work is busy");
}
