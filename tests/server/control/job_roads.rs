//! A job carried across its roads (docs/cache-fallback.md, 「One road resolution」; docs/api.md, 「assignments
//! and jobs」): a turn whose road stopped answering carried again on the next road, the same job, its prompt
//! opened with the line saying it continues the work and its report saying where it ran; at most once on
//! each provider; and a failure of the work itself moving nothing.

use std::collections::{HashMap, VecDeque};
use std::os::unix::fs::PermissionsExt;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::harness::{OrderEntry, set_provider_order};
use agency_server::control::work::{Carried, Carrier, carry_job, continued, road_stopped};
use agency_server::places::Ground;
use agency_server::road::Road;
use agency_server::seeds::deployment;
use agent_session::{EngineError, Failure, Outcome, TurnEnd};
use harness_providers::Machine;
use policy::providers::deepseek::DEEPSEEK;
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
  let env = home.join("data").join("env");
  let key = env.join(&DEEPSEEK.presence.key_file);

  std::fs::create_dir_all(&env).unwrap();
  std::fs::write(&key, "a key\n").unwrap();
  std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();

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
