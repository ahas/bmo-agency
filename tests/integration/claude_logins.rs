//! The one judgement every Claude run is offered a login by, on a scratch store of the test's own and the
//! windows agency keeps in it (docs/cache-fallback.md, 「The Claude Code road is a list of accounts」): a
//! login whose window is kept full is closed while the reading stands and not once it is stale, a run none of whose
//! logins answers goes out on none, a full window rests the login it was met on and no other, the rest that
//! closes the last open login is published once, and a standing program left on a closed login is taken
//! down.

use crate::common;

use agency_server::control::claude_accounts::{Account, Judging, open_accounts, walk_for};
use agency_server::job_run::claude::failure::Failure;
use agency_server::job_run::claude::walk::{Attempt, Walking, on_accounts};
use agency_server::keepalive::logins::retire_closed;
use agency_server::places::Ground;
use agency_server::rest::DeploymentResting;
use cache_siren::{Slot, Window};
use common::Scratch;
use harness_providers::Machine;
use serde_json::json;

const HOUR: i64 = 3_600;

// The program a standing place runs, as its record names it.
const PROGRAM: &str = "claude";

fn now() -> i64 {
  chrono::Utc::now().timestamp()
}

/// A store, a data root and the logins' homes of the test's own, and agency's ground on them, which keeps
/// the logins' windows in that store.
struct Bench {
  scratch: Scratch,
  homes: tempfile::TempDir,
  data: tempfile::TempDir,
  ground: Ground,
}

impl Bench {
  // Two logins, `main` first in the order and in use, `default` after it, each in a folder of its own.
  async fn new() -> Self {
    let scratch = Scratch::new().await;
    let homes = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let service = common::service(&scratch, &[]);
    let machine =
      Machine::with_env([("HOME".to_owned(), data.path().to_string_lossy().into_owned())], data.path());
    let ground = Ground::new(&service, &machine, None);

    for (id, place) in [("main", 1_i64), ("default", 2)] {
      let home = homes.path().join(id);

      std::fs::create_dir_all(&home).unwrap();
      scratch
        .run(
          r#"INSERT INTO "claude_accounts" ("id", "name", "place", "enabled", "credential_kind", "credential_path", "added_at")
             VALUES ($1, $1, $2, 1, 'directory', $3, 1)
             ON CONFLICT ("id") DO UPDATE SET "place" = $2, "credential_path" = $3"#,
          vec![id.into(), place.into(), home.to_string_lossy().into_owned().into()],
        )
        .await;
    }

    scratch
      .run(
        r#"INSERT INTO "settings" ("key", "value") VALUES ('claude_account_in_use', 'main')
           ON CONFLICT ("key") DO UPDATE SET "value" = 'main'"#,
        vec![],
      )
      .await;

    Self {
      scratch,
      homes,
      data,
      ground,
    }
  }

  // One login's weekly window kept as read: the share used, its reset and when it was seen.
  async fn read(&self, account: &str, used: f64, resets_at: i64, seen_at: i64) {
    let week = Window {
      name: "seven_day".into(),
      utilization: used,
      resets_at: Some(resets_at),
      duration_minutes: Some(10_080),
      limit_name: None,
    };

    self
      .ground
      .windows
      .replace(
        Slot {
          mode: "claude-code",
          account,
        },
        &[week],
        seen_at,
      )
      .await
      .unwrap();
  }

  fn judging(&self) -> Judging<'_> {
    Judging::of(&self.ground)
  }

  fn walking(&self) -> Walking<'_> {
    Walking {
      ground: &self.ground,
    }
  }

  async fn setting(&self, key: &str) -> Option<String> {
    store::one(
      self.scratch.db(),
      r#"SELECT "value" FROM "settings" WHERE "key" = $1"#,
      vec![key.into()],
    )
    .await
    .unwrap()
    .map(|row| row.try_get("", "value").unwrap())
  }

  async fn walk(&self) -> Vec<String> {
    walk_for(self.judging(), "bmo")
      .await
      .unwrap()
      .into_iter()
      .map(|account| account.id)
      .collect()
  }
}

// A turn that meets a full window on the logins named, and answers on any other; every login it was
// handed is kept.
struct Turn {
  full: Vec<&'static str>,
  tried: Vec<Option<String>>,
}

impl Attempt<String> for Turn {
  async fn attempt(&mut self, account: Option<Account>) -> Result<String, Failure> {
    let id = account.map(|account| account.id);

    self.tried.push(id.clone());

    match id {
      Some(id) if self.full.contains(&id.as_str()) => Err(Failure::Limit {
        resets_at: Some(now() + HOUR),
        account: None,
        silenced: false,
      }),
      Some(id) => Ok(id),
      None => Ok("the machine's own".to_owned()),
    }
  }
}

// A login whose window is kept full is never offered, whatever became of the rest written for it, and one whose
// window has reset since closes nothing.
#[tokio::test]
async fn a_login_whose_window_is_read_full_is_never_offered_whatever_became_of_its_rest() {
  let bench = Bench::new().await;

  bench.read("main", 1.0, now() + 4 * 24 * HOUR, now()).await;
  bench.read("default", 0.19, now() + 6 * 24 * HOUR, now()).await;

  assert_eq!(bench.walk().await, ["default"]);

  bench.read("main", 1.0, now() - 60, now()).await;

  assert_eq!(bench.walk().await, ["main", "default"]);
}

// A full reading closes its login for as long as its reader says it stands, and no longer.
#[tokio::test]
async fn a_full_reading_past_its_fresh_span_closes_nothing() {
  let bench = Bench::new().await;
  let fresh = bench.ground.windows.fresh("claude-code").as_secs() as i64;

  bench.read("main", 1.0, now() + 4 * 24 * HOUR, now() - fresh + 60).await;
  bench.read("default", 1.0, now() + HOUR, now() - fresh - 60).await;

  assert_eq!(bench.walk().await, ["default"]);
}

// With every login closed the run goes out on none, not on the closed ones and not on the machine's own
// login, and ends as the limit the first of them reopens from, naming no login, so nothing after the walk
// writes a rest under a login the run was not on.
#[tokio::test]
async fn a_run_none_of_whose_logins_answers_goes_out_on_none() {
  let bench = Bench::new().await;
  let reopens = now() + 2 * HOUR;

  bench.read("main", 1.0, now() + 4 * 24 * HOUR, now()).await;
  bench
    .scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('rest_until:default', $1)"#,
      vec![reopens.to_string().into()],
    )
    .await;

  assert!(open_accounts(bench.judging()).await.unwrap().is_empty());

  let mut turn = Turn {
    full: vec![],
    tried: vec![],
  };
  let walked = on_accounts(&bench.walking(), "bmo", None, &mut turn).await;

  assert_eq!(
    walked,
    Err(Failure::Limit {
      resets_at: Some(reopens),
      account: None,
      silenced: false,
    })
  );
  assert!(turn.tried.is_empty());
  assert_eq!(bench.setting("rest_until:main").await, None);
}

// A full window met on one login rests that login, under its own name, and the next carries the run; where
// every login meets one, each rests under its own name and the failure names the last.
#[tokio::test]
async fn a_full_window_rests_the_login_it_was_met_on_and_no_other() {
  let bench = Bench::new().await;
  let mut turn = Turn {
    full: vec!["main"],
    tried: vec![],
  };

  assert_eq!(on_accounts(&bench.walking(), "bmo", None, &mut turn).await, Ok("default".to_owned()));
  assert_eq!(turn.tried, [Some("main".to_owned()), Some("default".to_owned())]);
  assert!(bench.setting("rest_until:main").await.is_some());
  assert_eq!(bench.setting("rest_until:default").await, None);
  assert_eq!(bench.setting("claude_account_in_use").await.as_deref(), Some("default"));

  bench
    .scratch
    .run(r#"DELETE FROM "settings" WHERE "key" LIKE 'rest_until%'"#, vec![])
    .await;

  let mut turn = Turn {
    full: vec!["main", "default"],
    tried: vec![],
  };
  let walked = on_accounts(&bench.walking(), "bmo", None, &mut turn).await;

  assert!(matches!(walked, Err(Failure::Limit { account: Some(ref last), .. }) if last == "main"));
  assert!(bench.setting("rest_until:main").await.is_some());
  assert!(bench.setting("rest_until:default").await.is_some());
}

// The walk's rest that closes the last open login is published once as `deployment-resting`, and a run
// after it, every login closed, publishes nothing again.
#[tokio::test]
async fn the_rest_that_closes_the_last_login_is_published_once() {
  let bench = Bench::new().await;
  let mut resting = bench.ground.events.subscribe::<DeploymentResting>();
  let both_full = || Turn {
    full: vec!["main", "default"],
    tried: vec![],
  };

  assert!(matches!(
    on_accounts(&bench.walking(), "bmo", None, &mut both_full()).await,
    Err(Failure::Limit { .. })
  ));

  let published = resting.ready().expect("the rest was published");

  assert_eq!(published.kind, "limit");
  assert_eq!(resting.ready(), None);
  assert!(matches!(
    on_accounts(&bench.walking(), "bmo", None, &mut both_full()).await,
    Err(Failure::Limit { account: None, .. })
  ));
  assert_eq!(resting.ready(), None);
}

// The loop in a pane starts the program its place holds again on the login written into it, so a program
// left on a closed login is taken down, while one with a turn in it and one on an open login stand.
#[tokio::test]
async fn a_standing_program_on_a_closed_login_is_taken_down_where_no_turn_is_in_it() {
  let bench = Bench::new().await;
  let data = bench.data.path();
  let homes = bench.homes.path();

  bench.read("main", 1.0, now() + 4 * 24 * HOUR, now()).await;

  let place = |agent: &str, login: &str, turn_open: bool| {
    let path = data.join("agent-places").join(agent);
    let env = json!({ "CLAUDE_CONFIG_DIR": homes.join(login) });
    let command = json!({ "args": [PROGRAM, "--resume", "s"], "cwd": "/tmp", "env": env });

    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
      path.join("program.json"),
      json!({ "identity": agent, "command": command, "restart": command }).to_string(),
    )
    .unwrap();

    if turn_open {
      std::fs::write(
        path.join("run.json"),
        json!({ "name": "r", "title": "t", "person": false, "startedAt": 1, "endedAt": null }).to_string(),
      )
      .unwrap();
    }

    path.join("program.json")
  };
  let closed = place("chat", "main", false);
  let working = place("worker", "main", true);
  let open = place("gate", "default", false);

  retire_closed(bench.judging(), data, chrono::Utc::now().timestamp_millis())
    .await
    .unwrap();

  assert!(!closed.exists());
  assert!(working.exists());
  assert!(open.exists());
}
