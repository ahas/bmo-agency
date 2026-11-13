//! A job carried across its roads (docs/cache-fallback.md, 「One road resolution」; docs/api.md, 「assignments
//! and jobs」): a turn whose road stopped answering carried again on the next road, the same job, its prompt
//! opened with the line saying it continues the work and its report saying where it ran; at most once on
//! each provider; a failure of the work itself moving nothing; and a job whose item names its model carried
//! on that model alone, over the repository's pin, waiting for it rather than falling back, unless every
//! Claude account refused its turn, which ends it on that login failure with the account moves it made kept.

use std::collections::{HashMap, VecDeque};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::support::grounds::{OrderEntry, deepseek_balance, set_provider_order};
use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::work::{Carried, Carrier, add_road, carry_job, continued, road_stopped};
use agency_server::order::Entry;
use agency_server::places::Ground;
use agency_server::road::Road;
use agency_server::seeds::deployment;
use agent_session::{AccountFailure, EngineError, Failure, Outcome, TurnEnd};
use harness_providers::Machine;
use sea_orm::DatabaseConnection;

use crate::support::rows::repository;
use crate::support::scratch::Scratch;

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
  turns: Vec<(Road, Entry, String)>,
}

impl Carrier for Ends {
  async fn carry(&mut self, taken: &Road, entry: &Entry, prompt: &str) -> Result<TurnEnd, String> {
    self.turns.push((taken.clone(), entry.clone(), prompt.to_owned()));

    self.ends.pop_front().ok_or_else(|| "no end is left".to_owned())
  }
}

// What a job's turn ends on where every Claude account of its walk refused it.
const EVERY_ACCOUNT_REFUSED: &str = "claude coder: every Claude account of the road refused this run, and a person must \
  sign in to each again: default answered \"API Error: 403 Attention Required! | Cloudflare\"";

// A stand-in for a job's turn on Claude Code whose walk moves the turn off the login it went out on and then
// finds every login refusing it: the move kept on the job's row as the walk keeps it, every login left
// resting, and the turn ending on the login failure naming them.
struct EveryAccountRefuses<'g> {
  ground: &'g Ground,
  turns: usize,
}

impl Carrier for EveryAccountRefuses<'_> {
  async fn carry(&mut self, taken: &Road, _entry: &Entry, _prompt: &str) -> Result<TurnEnd, String> {
    self.turns += 1;

    let moved = Road {
      account: Some("second".to_owned()),
      ..taken.clone()
    };
    let until = chrono::Utc::now().timestamp() + 3_600;

    add_road(self.ground, 7, &moved, "Claude account default refused the turn").await;
    store::run(
      &self.ground.db,
      r#"INSERT INTO "settings" ("key", "value") VALUES ('rest_until:default', $1)"#,
      vec![until.to_string().into()],
    )
    .await
    .unwrap();

    Ok(TurnEnd {
      failure: Some(Failure::Account {
        failure: AccountFailure::Login,
        detail: EVERY_ACCOUNT_REFUSED.to_owned(),
      }),
      ..TurnEnd::failed("job-7", EVERY_ACCOUNT_REFUSED, None)
    })
  }
}

// A job on its item's model every Claude account of which refused its turn ends failed on that login failure,
// never waiting for the refusals' rests to end, its roads and its report keeping the account its turn moved to.
#[tokio::test(flavor = "multi_thread")]
async fn a_job_every_claude_account_of_which_refused_it_ends_on_that_failure_and_keeps_its_moves() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = standing(&scratch, home.path()).await;
  let mut carrier = EveryAccountRefuses {
    ground: &ground,
    turns: 0,
  };

  item_names(&scratch, "claude-code", "item-model").await;

  let ended =
    tokio::time::timeout(std::time::Duration::from_secs(20), carry_job(&ground, &job(), &mut carrier))
      .await;
  let (state, outcome, report, _) =
    ended.expect("the job ends rather than waits").unwrap().expect("an end");

  assert_eq!(carrier.turns, 1);
  assert_eq!((state.as_str(), outcome.as_deref()), ("ended", Some("failed")));

  let report = report.unwrap_or_default();

  assert!(report.starts_with(EVERY_ACCOUNT_REFUSED), "{report}");
  assert!(report.contains("claude-code · item-model (second)"), "{report}");

  let roads: serde_json::Value =
    serde_json::from_str(&job_column(&scratch, "roads").await.unwrap()).unwrap();

  assert_eq!(roads[0]["account"], "default");
  assert_eq!(roads[1]["account"], "second");
  assert_eq!(roads[1]["reason"], "Claude account default refused the turn");
  assert_eq!(job_column(&scratch, "held_for").await, None, "it never waited");
}

// A store whose order is DeepSeek, its key in place, then Claude Code on its one seeded login, and a job in a
// repository's checkout on it; every other provider is missing on a machine with nothing on its path.
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
    r#"[{"name":"deepseek-flash","road":"deepseek","provider":"deepseek","model":"deepseek-flash","api_base":"https://api.deepseek.com/v1","key_file":"deepseek.key","context_tokens":1048576,"answer_tokens":32768,"thinking":true}]"#,
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
  crate::support::rows::seated_on_order(scratch.db(), "worker").await;
  set_provider_order(scratch.db(), &order).await.unwrap();
  std::fs::create_dir_all(home.join("helper")).unwrap();
  repository(scratch.db(), "helper", &home.join("helper").to_string_lossy(), &[]).await;
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
    holder: "harness",
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
    carrier.turns.iter().map(|(road, _, _)| road.provider.as_str()).collect::<Vec<_>>(),
    vec!["deepseek", "claude-code"]
  );
  assert_eq!(carrier.turns[0].2, job().prompt, "the first road takes the job's own prompt");
  assert!(carrier.turns[1].2.contains("already ran on deepseek"), "{}", carrier.turns[1].2);
  assert!(carrier.turns[1].2.ends_with(job().prompt), "{}", carrier.turns[1].2);

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

// A job carried with one turn its carrier answers `answer` with: the outcome and report that turn leaves.
async fn ended_with(answer: &str) -> (Option<String>, String) {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = standing(&scratch, home.path()).await;
  let mut carrier = Ends {
    ends: VecDeque::from([TurnEnd::answered("job-7", answer)]),
    turns: vec![],
  };
  let (_, outcome, report, _) = carry_job(&ground, &job(), &mut carrier).await.unwrap().expect("an end");

  (outcome, report.unwrap_or_default())
}

/// an answer that is a tool call written as text is no report, and an ordinary answer still stands as one
#[tokio::test(flavor = "multi_thread")]
async fn an_answer_written_as_a_tool_call_leaves_no_report_and_an_ordinary_answer_stands() {
  let raw = "<tool_call><function=Read><parameter=file_path>/tmp/one</parameter></function></tool_call>";
  let (outcome, report) = ended_with(raw).await;

  assert_eq!(outcome.as_deref(), Some("failed"));
  assert!(!report.contains("<tool_call>"), "the call's own text is no report: {report}");
  assert!(report.contains("tool call written as text"), "{report}");

  let (outcome, report) = ended_with("The work is done.").await;

  assert_eq!(outcome, None);
  assert_eq!(report, "The work is done.");
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

// The job's row naming the model its item names, and its repository pinned to DeepSeek.
async fn item_names(scratch: &Scratch, provider: &str, model: &str) {
  scratch
    .run(
      r#"UPDATE "jobs" SET "item_provider" = $1, "item_model" = $2 WHERE "id" = 7"#,
      vec![provider.to_owned().into(), model.to_owned().into()],
    )
    .await;
  scratch
    .run(
      r#"UPDATE "repositories" SET "pinned_provider" = 'deepseek', "pinned_model" = 'deepseek-flash'
         WHERE "name" = 'helper'"#,
      vec![],
    )
    .await;
}

async fn job_column(scratch: &Scratch, column: &str) -> Option<String> {
  store::one(scratch.db(), &format!(r#"SELECT "{column}" FROM "jobs" WHERE "id" = 7"#), vec![])
    .await
    .unwrap()
    .unwrap()
    .try_get("", column)
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_whose_item_names_its_model_goes_out_on_it_over_the_pin_and_never_moves_off_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = standing(&scratch, home.path()).await;
  let mut carrier = Ends {
    ends: VecDeque::from([limit(), TurnEnd::answered("job-7", "never asked")]),
    turns: vec![],
  };

  item_names(&scratch, "claude-code", "item-model").await;

  let (_, outcome, _, _) = carry_job(&ground, &job(), &mut carrier).await.unwrap().expect("an end");

  assert_eq!(
    carrier
      .turns
      .iter()
      .map(|(road, _, _)| (road.provider.as_str(), road.model.as_str()))
      .collect::<Vec<_>>(),
    vec![("claude-code", "item-model")],
    "the item's model alone, over the pin, and nothing after its road stopped answering"
  );
  assert_eq!(outcome.as_deref(), Some("failed"));
  assert_eq!(job_column(&scratch, "chosen_by").await.as_deref(), Some("item"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_whose_item_s_model_cannot_answer_is_held_with_the_reason_and_never_falls_back() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = standing(&scratch, home.path()).await;
  let mut carrier = Ends {
    ends: VecDeque::from([TurnEnd::answered("job-7", "never asked")]),
    turns: vec![],
  };

  item_names(&scratch, "deepseek", "deepseek-flash").await;
  deepseek_balance(&ground, 0.0).await;

  let waited =
    tokio::time::timeout(std::time::Duration::from_secs(3), carry_job(&ground, &job(), &mut carrier)).await;

  assert!(waited.is_err(), "the job waits for its item's model");
  assert!(carrier.turns.is_empty(), "no turn went out on another road: {:?}", carrier.turns);

  let held = job_column(&scratch, "held_for").await.unwrap_or_default();

  assert!(held.contains("balance is spent"), "{held}");
}

// A job whose road cannot answer as it starts waits held with the reason, its item's orderer told once, and
// goes out as soon as a quota move says its road answers again, not at the policy's clock.
#[tokio::test(flavor = "multi_thread")]
async fn a_job_whose_road_cannot_answer_is_told_once_and_goes_out_as_its_road_reopens() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let _ = standing(&scratch, home.path()).await;
  let (peers, vars) = crate::support::peers::stand().await;
  let ground = {
    let service = Service::new(scratch.db().clone(), Config::read(|_| None).unwrap());
    let mut env = vec![
      ("HOME".to_owned(), home.path().to_string_lossy().into_owned()),
      ("PATH".to_owned(), "/nowhere".to_owned()),
    ];

    env.extend(vars);
    Ground::new(&service, &Machine::with_env(env, home.path().join("data")), None)
  };
  let mut carrier = Ends {
    ends: VecDeque::from([TurnEnd::answered("job-7", "done")]),
    turns: vec![],
  };

  peers
    .lock()
    .unwrap()
    .items
    .insert(70, crate::support::peers::item(70, "leased", serde_json::json!({ "scope": "-400" })));
  item_names(&scratch, "deepseek", "deepseek-flash").await;
  deepseek_balance(&ground, 0.0).await;

  let started = std::time::Instant::now();
  let reopen = async {
    while job_column(&scratch, "held_for").await.is_none() {
      tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    deepseek_balance(&ground, 5.0).await;
    ground.events.publish(agency_server::rest::DeploymentResting::of(
      agency_server::rest::RestKind::Limit,
      "login-a",
      0,
    ));
  };
  let carried_job = job();
  let (carried, ()) = tokio::join!(carry_job(&ground, &carried_job, &mut carrier), reopen);
  let (_, outcome, _, _) = carried.unwrap().expect("an end");

  assert_eq!(outcome, None, "the turn answered: {:?}", carrier.turns);
  assert_eq!(carrier.turns.len(), 1);
  assert!(
    started.elapsed() < std::time::Duration::from_secs(30),
    "woken by the move, not the clock"
  );
  assert_eq!(job_column(&scratch, "held_for").await, None);

  let told = peers.lock().unwrap().instructed();

  assert_eq!(told.len(), 1, "{told:?}");
  assert_eq!(told[0]["kind"], "job-waiting:70");
  assert!(told[0]["said"]["order"].as_str().unwrap().contains("balance is spent"), "{told:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_job_on_its_role_s_order_or_its_repository_s_pin_says_which_chose_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = standing(&scratch, home.path()).await;
  let answered = || Ends {
    ends: VecDeque::from([TurnEnd::answered("job-7", "Done.")]),
    turns: vec![],
  };

  let mut carrier = answered();

  carry_job(&ground, &job(), &mut carrier).await.unwrap().expect("an end");
  assert_eq!(job_column(&scratch, "chosen_by").await.as_deref(), Some("order"));

  scratch
    .run(
      r#"UPDATE "repositories" SET "pinned_provider" = 'claude-code', "pinned_model" = 'pinned-model'
         WHERE "name" = 'helper'"#,
      vec![],
    )
    .await;

  let mut carrier = answered();

  carry_job(&ground, &job(), &mut carrier).await.unwrap().expect("an end");
  assert_eq!(carrier.turns[0].0.model, "pinned-model");
  assert_eq!(job_column(&scratch, "chosen_by").await.as_deref(), Some("pin"));
}

// A job of a role whose switched-on agents all name no road goes out on the road of the supervisor that asks
// it, at that supervisor's effort, and the road it went out on is kept on the job
// (docs/provider-order.md, 「Runs that do not walk their own role's order」; docs/role-agents.md, 「Which agent
// a run takes」).
#[tokio::test(flavor = "multi_thread")]
async fn a_job_of_a_role_whose_agents_name_no_road_goes_out_on_the_road_of_the_supervisor_that_asks_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = standing(&scratch, home.path()).await;

  // The worker's agents are switched on and name no road.
  scratch
    .run(
      r#"UPDATE "role_agents" SET "provider" = NULL, "model" = NULL WHERE "role" = 'worker'"#,
      vec![],
    )
    .await;
  // The supervisor that asks stands on DeepSeek's flash model at the high effort.
  crate::support::rows::supervisor(
    scratch.db(),
    "asking",
    &[(
      "provider_order",
      r#"[{"provider":"deepseek","model":"deepseek-flash","effort":"high"}]"#.into(),
    )],
  )
  .await;
  scratch
    .run(r#"UPDATE "jobs" SET "holder_name" = 'asking' WHERE "id" = 7"#, vec![])
    .await;
  deepseek_balance(&ground, 5.0).await;

  let mut carrier = Ends {
    ends: VecDeque::from([TurnEnd::answered("job-7", "Done.")]),
    turns: vec![],
  };
  let carried = Carried {
    id: 7,
    worker: "helper",
    holder: "asking",
    prompt: "# An item\n\nDo the work.",
  };
  let (state, outcome, _, _) = carry_job(&ground, &carried, &mut carrier).await.unwrap().expect("an end");

  assert_eq!((state.as_str(), outcome), ("ended", None));

  let (taken, entry, _) = &carrier.turns[0];

  assert_eq!((taken.provider.as_str(), taken.model.as_str()), ("deepseek", "deepseek-flash"));
  assert_eq!(entry.effort.as_deref(), Some("high"), "the supervisor's effort");
  assert_eq!(job_column(&scratch, "chosen_by").await.as_deref(), Some("order"));

  let roads: String = store::one(scratch.db(), r#"SELECT "roads" FROM "jobs" WHERE "id" = 7"#, vec![])
    .await
    .unwrap()
    .unwrap()
    .try_get("", "roads")
    .unwrap();
  let roads: serde_json::Value = serde_json::from_str(&roads).unwrap();

  assert_eq!(roads[0]["provider"], "deepseek");
  assert_eq!(roads[0]["model"], "deepseek-flash");
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

// A failed request the program reported is the provider's where the provider could not answer — a server
// error, its connection refused, as the model gateway refuses it while it restarts — and the work's own where
// the request was refused for its shape (docs/provider-order.md, 「Falling over and coming back」).
#[test]
fn a_reported_api_failure_moves_the_road_on_a_server_error_and_never_on_the_request_s_shape() {
  let reported = |said: &str, kind: &str| {
    TurnEnd::failed(
      "r",
      said,
      Some(EngineError {
        kind: Some(kind.into()),
      }),
    )
  };
  let busy = "API Error: 503 litellm.ServiceUnavailableError: DeepseekException - Service is too busy.";
  let refused = "API Error: Connection refused — a firewall or proxy may be blocking it (ECONNREFUSED)";

  assert!(road_stopped(&reported(busy, "server_error")).is_some());
  assert!(road_stopped(&TurnEnd::failed("r", busy, None)).is_some(), "the status alone says it");
  assert!(road_stopped(&reported(refused, "server_error")).is_some());
  assert!(
    road_stopped(&TurnEnd::failed("r", refused, None)).is_some(),
    "the refusal alone says it"
  );
  assert!(road_stopped(&reported("API Error: 529 Overloaded", "overloaded")).is_some());

  assert!(
    road_stopped(&reported(
      "API Error: 400 messages: text content blocks must be non-empty",
      "invalid_request"
    ))
    .is_none()
  );
  assert!(road_stopped(&TurnEnd::failed("r", "API Error: 400 the request is malformed", None)).is_none());
  assert!(road_stopped(&reported("API Error: 413 request too large", "server_error")).is_none());
  assert!(road_stopped(&reported("the answer ran past its ceiling", "max_output_tokens")).is_none());
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

// A repository whose base checkout stands on main and whose one support worktree is not there yet, and a job
// in that support checkout: the first road that can answer is the pane program's.
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
    r#"[{"name":"deepseek-flash","road":"deepseek","provider":"deepseek","model":"deepseek-flash","api_base":"https://api.deepseek.com/v1","key_file":"deepseek.key","context_tokens":1048576,"answer_tokens":32768,"thinking":true}]"#,
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
  crate::support::rows::seated_on_order(scratch.db(), "worker").await;
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

  repository(scratch.db(), "web", &at(&base), &[("support_count", 1i64.into())]).await;
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
  async fn carry(&mut self, taken: &Road, _entry: &Entry, _prompt: &str) -> Result<TurnEnd, String> {
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
    holder: "harness",
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

// A checkout keeping a session with runs behind it gets its launch's turn: the carrier the launch hands it
// writes `start` rather than the launch sitting silently in a program already carrying another turn
// (docs/agent-control.md, 「A worker's pane lock」).
struct Starts {
  lines: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl Carrier for Starts {
  async fn carry(&mut self, _taken: &Road, _entry: &Entry, _prompt: &str) -> Result<TurnEnd, String> {
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

  agency_server::worker_session::records::record(
    scratch.db(),
    "helper",
    "claude-opus-5",
    "",
    "kept",
    None,
    None,
  )
  .await
  .unwrap();

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
