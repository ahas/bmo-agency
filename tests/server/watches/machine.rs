//! This machine's room for the launcher: the node service's reasons and agency's job-count limits over the
//! workers at work here, the launcher's notice of a held launch kept, and the room's return told once
//! nothing holds a launch.

use agency_server::events::Bus;
use agency_server::peers::node::{NodeRow, NodeService, PlaceHolding, Room as NodeRoom, Standing};
use agency_server::peers::{Asking, PeerFault};
use agency_server::watches::machine::{
  AtWork, LaunchHeld, MachineRoomReturned, at_work, keep_worker_job, launch_held, room, tell_launch_held,
  watch_held,
};
use agent_session::JobMark;
use serde_json::json;

use crate::scratch::Scratch;

/// A node service whose room holds what the test names, whose standing has the worker `w1` running a
/// browser, and which cannot be asked at all where it is `down`.
struct Machine {
  held: Vec<String>,
  down: bool,
}

impl Machine {
  fn holding(held: &[&str]) -> Self {
    Self {
      held: held.iter().map(|reason| (*reason).to_owned()).collect(),
      down: false,
    }
  }
}

fn unreachable() -> PeerFault {
  PeerFault::Unreachable {
    peer: "node service",
    detail: "refused".into(),
  }
}

impl NodeService for Machine {
  fn machine_room(&self) -> Asking<'_, NodeRoom> {
    let answer = if self.down {
      Err(unreachable())
    } else {
      Ok(NodeRoom {
        held: self.held.clone(),
      })
    };

    Box::pin(async move { answer })
  }

  fn machine_standing(&self) -> Asking<'_, Standing> {
    let answer = if self.down {
      Err(unreachable())
    } else {
      Ok(Standing {
        browsers: 3,
        places: vec![
          PlaceHolding {
            place: "w1".into(),
            megabytes: 800.0,
            browsers: 2,
          },
          PlaceHolding {
            place: "w2".into(),
            megabytes: 300.0,
            browsers: 0,
          },
          PlaceHolding {
            place: "chat".into(),
            megabytes: 200.0,
            browsers: 1,
          },
        ],
      })
    };

    Box::pin(async move { answer })
  }

  fn link_up<'a>(&'a self, _node: &'a str) -> Asking<'a, bool> {
    Box::pin(async { Ok(false) })
  }

  fn node_rows(&self) -> Asking<'_, Vec<NodeRow>> {
    Box::pin(async { Ok(vec![]) })
  }

  fn ssh_login<'a>(&'a self, _node: &'a str) -> Asking<'a, Option<Vec<String>>> {
    Box::pin(async { Ok(None) })
  }
}

// Workers w1 and w2 at work here, w3 idle, a worker at work on a node, and a chat agent with a run open.
async fn workers(scratch: &Scratch) {
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "node") VALUES
         ('w1', 'worker', NULL), ('w2', 'worker', NULL), ('w3', 'worker', NULL), ('far', 'worker', 'n1'),
         ('chat', 'chat', NULL)"#,
      vec![],
    )
    .await;

  for (id, agent, ended) in [
    ("e1", "w1", None),
    ("e2", "w2", None),
    ("e3", "w3", Some(5)),
    ("e4", "far", None),
    ("e5", "chat", None),
  ] {
    scratch
      .run(
        r#"INSERT INTO "agent_executions" ("id", "body") VALUES ($1, $2)"#,
        vec![id.into(), json!({ "id": id, "agent": agent, "endedAt": ended }).to_string().into()],
      )
      .await;
  }
}

#[tokio::test]
async fn the_jobs_at_work_are_the_workers_here_with_a_run_open_each_with_the_job_its_turn_named() {
  let scratch = Scratch::new().await;
  let db = scratch.db();

  workers(&scratch).await;
  keep_worker_job(
    db,
    "w1",
    Some(JobMark {
      job: 41,
      item: Some(1802),
    }),
  )
  .await;
  keep_worker_job(
    db,
    "w2",
    Some(JobMark {
      job: 42,
      item: None,
    }),
  )
  .await;
  keep_worker_job(db, "w2", None).await;

  assert_eq!(
    at_work(db).await.unwrap(),
    vec![
      AtWork {
        worker: "w1".into(),
        job: Some(JobMark {
          job: 41,
          item: Some(1802)
        })
      },
      AtWork {
        worker: "w2".into(),
        job: None
      },
    ]
  );
}

#[tokio::test]
async fn the_room_holds_on_the_node_service_s_reasons_and_then_on_agency_s_job_counts() {
  let scratch = Scratch::new().await;
  let db = scratch.db();

  workers(&scratch).await;

  let roomy = room(db, &Machine::holding(&[])).await.unwrap();

  assert_eq!((roomy.jobs, roomy.browser_jobs, roomy.browsers), (2, 1, 3));
  assert!(roomy.held.is_empty(), "two jobs and one with a browser are under the defaults");

  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES
         ('machine_limit:launch_jobs', '2'), ('machine_limit:launch_browser_jobs', 'none')"#,
      vec![],
    )
    .await;

  let held = room(db, &Machine::holding(&["disk free 12GB < 20GB"])).await.unwrap();

  assert_eq!(held.held, vec!["disk free 12GB < 20GB".to_owned(), "jobs 2 >= 2".to_owned()]);
  assert_eq!(
    json!(held),
    json!({ "held": ["disk free 12GB < 20GB", "jobs 2 >= 2"], "jobs": 2, "browserJobs": 1, "browsers": 3 })
  );

  let unread = room(
    db,
    &Machine {
      held: vec!["load 80 >= 24 threads x 2".into()],
      down: true,
    },
  )
  .await
  .unwrap();

  assert_eq!(
    unread.held,
    vec!["jobs 2 >= 2".to_owned()],
    "a node service not asked holds nothing of its part"
  );
  assert_eq!((unread.browser_jobs, unread.browsers), (0, 0));
}

#[tokio::test]
async fn a_held_launch_is_let_go_and_its_return_told_once_nothing_holds_and_a_withdrawal_tells_nothing() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let bus = Bus::default();
  let mut returned = bus.subscribe::<MachineRoomReturned>();
  let notice = LaunchHeld {
    at: 1_000,
    held: vec!["memory pressure 6% >= 5%".into()],
  };

  assert!(
    !watch_held(db, &Machine::holding(&[]), &bus).await.unwrap(),
    "no notice, nothing to tell"
  );

  tell_launch_held(db, &notice).await.unwrap();
  assert_eq!(launch_held(db).await.unwrap(), Some(notice.clone()));
  assert!(!watch_held(db, &Machine::holding(&["memory pressure 6% >= 5%"]), &bus).await.unwrap());
  assert_eq!(launch_held(db).await.unwrap(), Some(notice.clone()), "still held, the notice stands");
  assert!(returned.ready().is_none());

  assert!(watch_held(db, &Machine::holding(&[]), &bus).await.unwrap());
  assert_eq!(
    returned.ready(),
    Some(MachineRoomReturned {
      at: 1_000,
      held: vec!["memory pressure 6% >= 5%".into()]
    })
  );
  assert_eq!(launch_held(db).await.unwrap(), None);

  tell_launch_held(db, &notice).await.unwrap();
  tell_launch_held(
    db,
    &LaunchHeld {
      at: 2_000,
      held: vec![],
    },
  )
  .await
  .unwrap();
  assert_eq!(launch_held(db).await.unwrap(), None);
  assert!(!watch_held(db, &Machine::holding(&[]), &bus).await.unwrap());
  assert!(returned.ready().is_none(), "a withdrawn notice is let go without a word");
}
