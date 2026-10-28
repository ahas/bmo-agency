//! A job carried across its roads (docs/cache-fallback.md, 「One road resolution」; docs/api.md, 「assignments
//! and jobs」): a turn whose road stopped answering carried again on the next road, the same job, its prompt
//! opened with the line saying it continues the work and its report saying where it ran; at most once on
//! each provider; and a failure of the work itself moving nothing.

use std::collections::{HashMap, VecDeque};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::harness::{OrderEntry, set_provider_order};
use agency_server::control::work::{Carried, Carrier, carry_job, continued, road_stopped};
use agency_server::places::Ground;
use agency_server::road::Road;
use agency_server::seeds::deployment;
use agent_session::{EngineError, Failure, Outcome, TurnEnd};
use harness_providers::Machine;
use sea_orm::DatabaseConnection;

use crate::scratch::Scratch;

fn ground_on(db: DatabaseConnection, home: &std::path::Path) -> Ground {
  let variables: HashMap<String, String> = HashMap::new();
  let service = Service::new(db, Config::read(|name| variables.get(name).cloned()).unwrap());
  let env = vec![
    ("HOME".to_owned(), home.to_string_lossy().into_owned()),
    ("PATH".to_owned(), "/nowhere".to_owned()),
  ];

  Ground::new(&service, &Machine::with_env(env, home.join("data")), None)
}

fn limit() -> TurnEnd {
  TurnEnd {
    failure: Some(Failure::Limit {
      resets_at: None,
      account: None,
      silenced: false,
    }),
    ..TurnEnd::failed("job-7", "the login's window is full", None)
  }
}

/// A stand-in for the worker's session: each turn it is handed kept, each answered by the next end it holds.
struct Ends {
  ends: VecDeque<TurnEnd>,
  turns: Vec<(Road, String)>,
}

impl Carrier for Ends {
  async fn carry(&mut self, taken: &Road, prompt: &str) -> Result<TurnEnd, String> {
    self.turns.push((taken.clone(), prompt.to_owned()));

    self.ends.pop_front().ok_or_else(|| "no end is left".to_owned())
  }
}

// A store whose order is DeepSeek, its key in place, then Claude Code on its one seeded login, and a job of a
// worker on it; every other provider is missing on a machine with nothing on its path.
async fn standing(scratch: &Scratch, home: &std::path::Path) -> Ground {
  let data = home.join("data");
  let env = data.join("env");

  std::fs::create_dir_all(&env).unwrap();
  for (name, key) in [("deepseek.key", "a key\n"), ("llm-gateway.key", "a key\n")] {
    let file = env.join(name);

    std::fs::write(&file, key).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
  }

  let members = data.join("llm-gateway");

  std::fs::create_dir_all(&members).unwrap();
  std::fs::write(
    members.join("members.json"),
    r#"[{"name":"deepseek-flash","road":"deepseek","provider":"deepseek","model":"deepseek-flash","api_base":"https://api.deepseek.com/v1","key_file":"deepseek.key","wait_seconds":300,"context_tokens":1048576,"answer_tokens":32768,"thinking":true}]"#,
  )
  .unwrap();

  let order: Vec<OrderEntry> = ["deepseek", "claude-code", "codex", "openrouter", "registered"]
    .iter()
    .map(|name| OrderEntry {
      name: (*name).to_owned(),
      enabled: true,
    })
    .collect();

  let ground = ground_on(scratch.db().clone(), home);

  store::seed::settle(scratch.db(), &deployment(&ground.machine())).await.unwrap();
  set_provider_order(scratch.db(), &order).await.unwrap();
  scratch
    .run(r#"INSERT INTO "agents" ("name", "kind") VALUES ('helper', 'worker')"#, vec![])
    .await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
           "token_count", "started_at", "prompt")
         VALUES (7, 70, 1, 'harness', 1, 'helper', 'running', 0, 0, '# An item')"#,
      vec![],
    )
    .await;

  ground
}

fn job() -> Carried<'static> {
  Carried {
    id: 7,
    worker: "helper",
    prompt: "# An item\n\nDo the work.",
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_turn_whose_road_stopped_answering_goes_out_again_on_the_next_road_and_says_so() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = standing(&scratch, home.path()).await;
  let mut carrier = Ends {
    ends: VecDeque::from([limit(), TurnEnd::answered("job-7", "Done.")]),
    turns: vec![],
  };

  let (state, outcome, report, _) =
    carry_job(&ground, &job(), &mut carrier).await.unwrap().expect("an end");

  assert_eq!((state.as_str(), outcome), ("ended", None));
  assert_eq!(
    carrier.turns.iter().map(|(road, _)| road.provider.as_str()).collect::<Vec<_>>(),
    vec!["deepseek", "claude-code"]
  );
  assert_eq!(carrier.turns[0].1, job().prompt, "the first road takes the job's own prompt");
  assert!(carrier.turns[1].1.contains("already ran on deepseek"), "{}", carrier.turns[1].1);
  assert!(carrier.turns[1].1.ends_with(job().prompt), "{}", carrier.turns[1].1);

  let report = report.unwrap_or_default();

  assert!(report.starts_with("Done."), "{report}");
  assert!(report.contains("The job ran on deepseek"), "{report}");

  let roads: String = store::one(scratch.db(), r#"SELECT "roads" FROM "jobs" WHERE "id" = 7"#, vec![])
    .await
    .unwrap()
    .unwrap()
    .try_get("", "roads")
    .unwrap();
  let roads: serde_json::Value = serde_json::from_str(&roads).unwrap();

  assert_eq!(roads[0]["provider"], "deepseek");
  assert_eq!(roads[0]["reason"], serde_json::Value::Null);
  assert_eq!(roads[1]["provider"], "claude-code");
  assert_eq!(roads[1]["reason"], "the login's window is full");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_goes_out_at_most_once_on_each_provider_and_then_ends_as_its_last_turn_did() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = standing(&scratch, home.path()).await;
  let mut carrier = Ends {
    ends: VecDeque::from([limit(), limit(), TurnEnd::answered("job-7", "never asked")]),
    turns: vec![],
  };

  let (_, outcome, report, _) = carry_job(&ground, &job(), &mut carrier).await.unwrap().expect("an end");

  assert_eq!(carrier.turns.len(), 2, "each provider once");
  assert_eq!(outcome.as_deref(), Some("failed"));
  assert!(report.unwrap_or_default().contains("The job ran on deepseek"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failure_of_the_work_itself_moves_nothing() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = standing(&scratch, home.path()).await;
  let mut carrier = Ends {
    ends: VecDeque::from([TurnEnd::failed("job-7", "the tests failed: 3 of 40", None)]),
    turns: vec![],
  };

  let (_, outcome, report, _) = carry_job(&ground, &job(), &mut carrier).await.unwrap().expect("an end");

  assert_eq!(carrier.turns.len(), 1);
  assert_eq!(outcome.as_deref(), Some("failed"));
  assert_eq!(report.as_deref(), Some("the tests failed: 3 of 40"));
}

#[test]
fn a_road_stops_answering_on_its_quota_window_balance_or_credential_and_never_on_the_work() {
  assert!(road_stopped(&limit()).is_some());
  assert!(road_stopped(&TurnEnd::failed("r", "API Error: 402 insufficient balance", None)).is_some());
  assert!(road_stopped(&TurnEnd::failed("r", "Please run /login", None)).is_some());
  assert!(
    road_stopped(&TurnEnd::failed(
      "r",
      "the engine refused",
      Some(EngineError {
        kind: Some("rate_limit".into())
      })
    ))
    .is_some()
  );
  assert!(road_stopped(&TurnEnd::failed("r", "the tests failed", None)).is_none());
  assert!(
    road_stopped(&TurnEnd::of("r", Outcome::Stopped)).is_none(),
    "a stop from outside is the stop"
  );
  assert!(road_stopped(&TurnEnd::answered("r", "claude usage limit reached")).is_none());
}

#[test]
fn a_job_launched_again_is_told_it_continues_the_work_in_the_checkout() {
  let from = Road {
    provider: "deepseek".into(),
    model: "deepseek-v4-pro".into(),
    account: None,
  };
  let prompt = continued(&from, "its balance is spent", "# An item");

  assert!(prompt.contains("deepseek · deepseek-v4-pro"), "{prompt}");
  assert!(prompt.contains("its balance is spent"), "{prompt}");
  assert!(prompt.contains("checkout"), "{prompt}");
  assert!(prompt.ends_with("# An item"), "{prompt}");
}

// A git command in a directory, with the identity a machine without one configured still commits under.
fn git(path: &Path, args: &[&str]) -> String {
  let done = Command::new("git")
    .arg("-C")
    .arg(path)
    .args(args)
    .env("GIT_AUTHOR_NAME", "tester")
    .env("GIT_AUTHOR_EMAIL", "tester@example.com")
    .env("GIT_COMMITTER_NAME", "tester")
    .env("GIT_COMMITTER_EMAIL", "tester@example.com")
    .output()
    .unwrap();

  assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));

  String::from_utf8_lossy(&done.stdout).trim().to_owned()
}

fn at(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

// A base worker in a repository of its own and a support worker whose worktree is not there: the base
// stands on main, and the first road that can answer is the pane program's.
async fn support_standing(scratch: &Scratch, home: &Path) -> (Ground, PathBuf, PathBuf, String) {
  let data = home.join("data");
  let env = data.join("env");

  std::fs::create_dir_all(&env).unwrap();
  for (name, key) in [("deepseek.key", "a key\n"), ("llm-gateway.key", "a key\n")] {
    let file = env.join(name);

    std::fs::write(&file, key).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
  }

  let members = data.join("llm-gateway");

  std::fs::create_dir_all(&members).unwrap();
  std::fs::write(
    members.join("members.json"),
    r#"[{"name":"deepseek-flash","road":"deepseek","provider":"deepseek","model":"deepseek-flash","api_base":"https://api.deepseek.com/v1","key_file":"deepseek.key","wait_seconds":300,"context_tokens":1048576,"answer_tokens":32768,"thinking":true}]"#,
  )
  .unwrap();

  let order: Vec<OrderEntry> = ["deepseek", "claude-code", "codex", "openrouter", "registered"]
    .iter()
    .map(|name| OrderEntry {
      name: (*name).to_owned(),
      enabled: true,
    })
    .collect();
  let ground = ground_on(scratch.db().clone(), home);

  store::seed::settle(scratch.db(), &deployment(&ground.machine())).await.unwrap();
  set_provider_order(scratch.db(), &order).await.unwrap();

  let base = home.join("web");
  let cwd = home.join("web-w2");
  let mut init = Command::new("git");

  init.args(["init", "-q", "-b", "main"]).arg(&base);

  let done = init.output().unwrap();

  assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));

  std::fs::write(base.join("README.md"), "# The base\n").unwrap();
  std::fs::write(
    base.join("package.json"),
    r#"{"name":"support-test","scripts":{"worktree:setup":"touch setup-ran"}}"#,
  )
  .unwrap();
  git(&base, &["add", "-A"]);
  git(&base, &["commit", "-q", "-m", "the base"]);

  let head = git(&base, &["rev-parse", "HEAD"]);

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "cwd") VALUES ('web', 'worker', $1)"#,
      vec![at(&base).into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "cwd", "support_of", "support_slot")
         VALUES ('web-w2', 'worker', $1, 'web', 2)"#,
      vec![at(&cwd).into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
           "token_count", "started_at", "prompt")
         VALUES (8, 80, 1, 'harness', 1, 'web-w2', 'running', 0, 0, '# An item')"#,
      vec![],
    )
    .await;

  (ground, base, cwd, head)
}

// A stand-in for the session: it takes the turn only where the worktree already stands at the base's head.
struct Standing {
  at: PathBuf,
  head: String,
  providers: Vec<String>,
  stood: bool,
}

impl Carrier for Standing {
  async fn carry(&mut self, taken: &Road, _prompt: &str) -> Result<TurnEnd, String> {
    self.providers.push(taken.provider.clone());
    self.stood = self.at.join(".git").exists() && git(&self.at, &["rev-parse", "HEAD"]) == self.head;

    Ok(TurnEnd::answered("job-8", "Done."))
  }
}

// A first launch on a fresh support makes its worktree from the base's main, sets it up by the
// repository's own script, and only then stands the pane program the first road goes through
// (docs/worker-slots.md, 「Names and places」); a worktree gone is made again at the next launch.
#[tokio::test(flavor = "multi_thread")]
async fn a_first_launch_on_a_fresh_support_makes_its_worktree_before_the_pane_road_takes_the_turn() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let (ground, base, cwd, head) = support_standing(&scratch, home.path()).await;
  let job = || Carried {
    id: 8,
    worker: "web-w2",
    prompt: "# An item\n\nDo the work.",
  };
  let mut carrier = Standing {
    at: cwd.clone(),
    head: head.clone(),
    providers: vec![],
    stood: false,
  };
  let (state, outcome, _, _) = carry_job(&ground, &job(), &mut carrier).await.unwrap().expect("an end");

  assert_eq!((state.as_str(), outcome), ("ended", None));
  assert_eq!(carrier.providers, vec!["deepseek"], "the first road is the pane program's");
  assert!(carrier.stood, "the worktree stood before the pane road's turn was taken");
  assert!(cwd.join(".git").exists());
  assert!(cwd.join("setup-ran").exists(), "the repository's own setup ran in it");
  assert_eq!(git(&cwd, &["rev-parse", "HEAD"]), head, "detached at the base's main");

  // The worktree gone, the next launch makes it again by the same cut.
  git(&base, &["worktree", "remove", "--force", &at(&cwd)]);
  assert!(!cwd.exists());

  let mut again = Standing {
    at: cwd.clone(),
    head: head.clone(),
    providers: vec![],
    stood: false,
  };

  carry_job(&ground, &job(), &mut again).await.unwrap().expect("an end");

  assert!(again.stood, "a missing worktree is made again at the next launch");
  assert!(cwd.join(".git").exists());
  assert_eq!(git(&cwd, &["rev-parse", "HEAD"]), head);
}

// A worker with runs behind it gets its launch's turn: the carrier the launch hands it writes `start`
// rather than the launch sitting silently in a program already carrying another turn
// (docs/agent-control.md, 「A worker's pane lock」).
struct Starts {
  lines: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl Carrier for Starts {
  async fn carry(&mut self, _taken: &Road, _prompt: &str) -> Result<TurnEnd, String> {
    self.lines.lock().unwrap().push("start".to_owned());

    Ok(TurnEnd::answered("job-7", "Done."))
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_launch_on_a_worker_with_runs_writes_start() {
  use std::sync::{Arc, Mutex};
  use std::time::{Duration, Instant};

  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = standing(&scratch, home.path()).await;

  scratch.run(r#"UPDATE "agents" SET "runs" = 3 WHERE "name" = 'helper'"#, vec![]).await;

  let started = Arc::new(Mutex::new(Vec::new()));
  let mut carrier = Starts {
    lines: started.clone(),
  };

  carry_job(&ground, &job(), &mut carrier).await.unwrap().expect("an end");

  let began = Instant::now();

  while started.lock().unwrap().is_empty() && began.elapsed() < Duration::from_secs(60) {
    tokio::time::sleep(Duration::from_millis(50)).await;
  }

  assert_eq!(&*started.lock().unwrap(), &["start".to_owned()]);
}
