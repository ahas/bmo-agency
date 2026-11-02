//! The one road resolution (docs/cache-fallback.md, 「One road resolution」): the walk a pin heads or the
//! order's top begins, the pin carrying the work whenever it can answer, every provider that cannot passed
//! with its reason — turned off, missing, its balance spent, no login open — the order closed only where
//! nothing in it can answer, with what a person must do, and each move, return and closing told once.

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::harness::{OrderEntry, set_provider_order};
use agency_server::now_ms;
use agency_server::places::Ground;
use agency_server::road::*;
use agency_server::seeds::deployment;
use harness_providers::Machine;
use policy::providers::deepseek::DEEPSEEK;
use sea_orm::DatabaseConnection;

use crate::scratch::Scratch;

fn names(list: &[&str]) -> Vec<String> {
  list.iter().map(|one| (*one).to_owned()).collect()
}

fn road(provider: &str, model: &str) -> Road {
  Road {
    provider: provider.into(),
    model: model.into(),
    account: None,
  }
}

fn can(provider: &str) -> Step {
  Step {
    provider: provider.into(),
    model: Some(format!("{provider}-model")),
    answer: Answer::Can(road(provider, &format!("{provider}-model"))),
  }
}

fn cannot(provider: &str, reason: &str) -> Step {
  Step {
    provider: provider.into(),
    model: Some(format!("{provider}-model")),
    answer: Answer::Cannot(Closed::plain(provider, reason)),
  }
}

fn no_road(provider: &str) -> Step {
  Step {
    provider: provider.into(),
    model: None,
    answer: Answer::NoRoad,
  }
}

fn resolution(pinned: Option<&str>, steps: Vec<Step>) -> Resolution {
  Resolution {
    key: "job-worker".into(),
    pinned: pinned.map(str::to_owned),
    steps,
    line: Some("worker".into()),
    uncarried: vec![],
    named: false,
  }
}

#[test]
fn an_automatic_walk_begins_at_the_top_and_a_pin_heads_it_then_takes_the_rest_from_it() {
  let order = names(&["deepseek", "registered", "claude-code", "codex"]);

  assert_eq!(walk(&order, None), order);
  assert_eq!(
    walk(&order, Some("claude-code")),
    names(&["claude-code", "codex", "deepseek", "registered"])
  );
  assert_eq!(walk(&order, Some("deepseek")), order);
  assert_eq!(walk(&order, Some("nowhere")), order, "a pin the order does not hold walks from the top");
}

#[test]
fn a_pin_carries_the_work_whenever_it_can_answer_whatever_leads_the_order() {
  let pinned = resolution(Some("claude-code"), vec![can("claude-code"), can("deepseek")]);

  assert_eq!(pinned.road(), Some(&road("claude-code", "claude-code-model")));
  assert!(pinned.passed().is_empty());
  assert_eq!(pinned.head(), Some(road("claude-code", "claude-code-model")));
}

#[test]
fn a_pin_that_cannot_answer_is_the_head_and_never_a_wall() {
  let pinned = resolution(
    Some("deepseek"),
    vec![cannot("deepseek", "its balance is spent"), no_road("codex"), can("claude-code")],
  );

  assert_eq!(pinned.road().map(|road| road.provider.as_str()), Some("claude-code"));
  assert_eq!(pinned.head().map(|head| head.provider), Some("deepseek".to_owned()));
  assert_eq!(
    pinned.passed().iter().map(|closed| closed.reason.as_str()).collect::<Vec<_>>(),
    vec!["its balance is spent"]
  );
}

#[test]
fn every_reason_a_provider_cannot_answer_is_passed_over_and_named() {
  let walked = resolution(
    None,
    vec![
      cannot("claude-code", "no login of it is open: first is at its limit"),
      cannot("codex", "the codex executable is not on this machine"),
      cannot("deepseek", "its balance is spent (0 USD left)"),
      cannot("openrouter", "the owner turned it off"),
      can("registered"),
    ],
  );

  assert_eq!(walked.road().map(|road| road.provider.as_str()), Some("registered"));
  assert_eq!(walked.passed().len(), 4);
  assert_eq!(walked.head().map(|head| head.provider), Some("claude-code".to_owned()));
}

#[test]
fn automatic_work_heads_on_the_first_road_of_its_key_and_goes_out_on_the_first_that_can() {
  let walked = resolution(None, vec![no_road("codex"), can("deepseek"), can("claude-code")]);

  assert_eq!(walked.head().map(|head| head.provider), Some("deepseek".to_owned()));
  assert_eq!(walked.road().map(|road| road.provider.as_str()), Some("deepseek"));
  assert_eq!(walked.open().count(), 2);
}

#[test]
fn a_closed_order_names_every_reason_the_moment_it_reopens_and_what_a_person_does() {
  let mut resting = Closed::plain("claude-code", "no login of it is open");
  let mut spent = Closed::plain("deepseek", "its balance is spent");

  resting.reopens_at = Some(1_900_000_000);
  spent.command = Some("top the balance up".into());

  let closed = resolution(
    None,
    vec![
      Step {
        provider: "deepseek".into(),
        model: Some("deepseek-model".into()),
        answer: Answer::Cannot(spent),
      },
      Step {
        provider: "claude-code".into(),
        model: Some("claude-model".into()),
        answer: Answer::Cannot(resting),
      },
    ],
  );
  let sentence = closed.sentence();

  assert_eq!(closed.road(), None);
  assert_eq!(closed.command(), Some(("deepseek", "top the balance up")));
  assert_eq!(closed.reopens_at(), Some(1_900_000_000));
  assert!(sentence.contains("deepseek: its balance is spent"), "{sentence}");
  assert!(sentence.contains("claude-code: no login of it is open"), "{sentence}");
  assert!(sentence.contains("top the balance up"), "{sentence}");
  assert!(sentence.contains("2030-03-17"), "{sentence}");
}

#[test]
fn a_move_is_told_once_and_the_return_to_the_head_once() {
  let moved =
    resolution(Some("deepseek"), vec![cannot("deepseek", "its balance is spent"), can("claude-code")]);
  let claude = road("claude-code", "claude-code-model");
  let (fact_of, kept) = fact(None, &moved, Some(&claude), &[]);

  assert_eq!(
    fact_of,
    Some(Fact::Moved {
      from: Some(road("deepseek", "deepseek-model")),
      to: claude.clone(),
      reasons: vec![Closed::plain("deepseek", "its balance is spent")],
    })
  );
  assert_eq!(
    kept,
    Some(Kept::Moved {
      road: claude.clone()
    })
  );

  let (again, kept) = fact(kept.as_ref(), &moved, Some(&claude), &[]);

  assert_eq!(again, None, "a move that stands is not told again");

  let back = resolution(Some("deepseek"), vec![can("deepseek"), can("claude-code")]);
  let deepseek = road("deepseek", "deepseek-model");
  let (returned, kept) = fact(kept.as_ref(), &back, Some(&deepseek), &[]);

  assert_eq!(
    returned,
    Some(Fact::Returned {
      from: Some(claude),
      to: deepseek.clone(),
    })
  );
  assert_eq!(kept, None);
  assert_eq!(
    fact(None, &back, Some(&deepseek), &[]).0,
    None,
    "work on its head with nothing kept is no fact"
  );
}

#[test]
fn a_closed_order_is_told_once_and_its_reopening_as_a_return() {
  let closed = resolution(None, vec![cannot("claude-code", "no login of it is open")]);
  let (told, kept) = fact(None, &closed, None, &[]);

  assert!(matches!(told, Some(Fact::Closed { ref reasons, .. }) if reasons.len() == 1));
  assert_eq!(fact(kept.as_ref(), &closed, None, &[]).0, None);

  let open = resolution(None, vec![can("claude-code")]);

  assert!(matches!(
    fact(kept.as_ref(), &open, Some(&road("claude-code", "claude-code-model")), &[]).0,
    Some(Fact::Returned { from: None, .. })
  ));
}

// ---- asked of the store and the machine ----

fn ground_on(db: DatabaseConnection, home: &std::path::Path) -> Ground {
  let variables: HashMap<String, String> = HashMap::new();
  let service = Service::new(db, Config::read(|name| variables.get(name).cloned()).unwrap());
  let env = vec![
    ("HOME".to_owned(), home.to_string_lossy().into_owned()),
    ("PATH".to_owned(), "/nowhere".to_owned()),
  ];

  Ground::new(&service, &Machine::with_env(env, home.join("data")), None)
}

fn deepseek_key(home: &std::path::Path) {
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
}

async fn order(db: &DatabaseConnection, names: &[(&str, bool)]) {
  let order: Vec<OrderEntry> = names
    .iter()
    .map(|(name, enabled)| OrderEntry {
      name: (*name).to_owned(),
      enabled: *enabled,
    })
    .collect();

  set_provider_order(db, &order).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_spent_balance_and_a_closed_login_each_close_their_provider_and_a_balance_topped_up_returns() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());
  let job = Ask::key("job-worker");

  store::seed::settle(scratch.db(), &deployment(&ground.machine())).await.unwrap();

  deepseek_key(home.path());
  order(
    scratch.db(),
    &[
      ("deepseek", true),
      ("claude-code", true),
      ("codex", true),
      ("openrouter", true),
      ("registered", true),
    ],
  )
  .await;

  let open = resolve(&ground, &job).await.unwrap();

  assert_eq!(open.road().map(|road| road.provider.as_str()), Some("deepseek"), "{open:?}");

  scratch
    .run(
      r#"INSERT INTO "usage_balances" ("mode", "account", "amount", "currency", "seen_at") VALUES ('deepseek', '', 0, 'USD', 1)"#,
      vec![],
    )
    .await;

  let spent = resolve(&ground, &job).await.unwrap();
  let taken = spent.road().expect("Claude Code carries the work");

  assert_eq!(taken.provider, "claude-code");
  assert!(taken.account.is_some(), "the first open login is named: {taken:?}");
  assert_eq!(spent.command(), Some(("deepseek", DEEPSEEK.balance.top_up.as_str())));

  // Every login rests: nothing in the order can answer, and the moment the first reopens is named.
  let until = now_ms() / 1000 + 3600;

  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") SELECT 'rest_until:' || "id", $1 FROM "claude_accounts""#,
      vec![until.to_string().into()],
    )
    .await;

  let closed = resolve(&ground, &job).await.unwrap();

  assert_eq!(closed.road(), None, "{closed:?}");
  assert_eq!(closed.reopens_at(), Some(until));
  assert!(
    closed.sentence().contains("claude-code: no login of it is open"),
    "{}",
    closed.sentence()
  );

  // The balance topped up: DeepSeek, the head, carries the work again.
  scratch.run(r#"UPDATE "usage_balances" SET "amount" = 5"#, vec![]).await;

  assert_eq!(
    resolve(&ground, &job).await.unwrap().road().map(|road| road.provider.clone()),
    Some("deepseek".into())
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_key_set_to_one_provider_goes_out_on_it_while_it_can_and_walks_on_while_it_cannot() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());
  let job = Ask::key("job-worker");

  store::seed::settle(scratch.db(), &deployment(&ground.machine())).await.unwrap();

  deepseek_key(home.path());
  order(
    scratch.db(),
    &[
      ("claude-code", true),
      ("codex", true),
      ("openrouter", true),
      ("registered", true),
      ("deepseek", true),
    ],
  )
  .await;
  scratch
    .run(
      r#"INSERT INTO "fallback_routes" ("key", "set_at", "route_mode") VALUES ('job-worker', 0, 'deepseek')"#,
      vec![],
    )
    .await;

  let pinned = resolve(&ground, &job).await.unwrap();

  assert_eq!(pinned.road().map(|road| road.provider.as_str()), Some("deepseek"), "{pinned:?}");

  // Turned off, the pin is the head of a walk that goes on to Claude Code.
  order(
    scratch.db(),
    &[
      ("claude-code", true),
      ("codex", true),
      ("openrouter", true),
      ("registered", true),
      ("deepseek", false),
    ],
  )
  .await;

  let off = resolve(&ground, &job).await.unwrap();

  assert_eq!(off.road().map(|road| road.provider.as_str()), Some("claude-code"), "{off:?}");
  assert_eq!(off.head().map(|head| head.provider), Some("deepseek".to_owned()));
  assert_eq!(off.passed().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_agents_fixed_model_heads_its_keys_walk_and_its_clearing_returns_the_key_its_road() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());

  store::seed::settle(scratch.db(), &deployment(&ground.machine())).await.unwrap();
  order(scratch.db(), &[("claude-code", true), ("registered", true)]).await;
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "turn_shape", "pinned_provider", "pinned_model")
         VALUES ('bmo', 'chat', 'window', 'registered', 'local-model')"#,
      vec![],
    )
    .await;

  let fixed = resolve(&ground, &Ask::key("bmo")).await.unwrap();

  assert_eq!(fixed.pinned.as_deref(), Some("registered"), "{fixed:?}");
  assert_eq!(
    fixed.head().map(|head| (head.provider, head.model)),
    Some(("registered".to_owned(), "local-model".to_owned()))
  );

  // Its fixed model let go, the supervisor answers on its key's road again.
  scratch
    .run(r#"UPDATE "agents" SET "pinned_provider" = NULL, "pinned_model" = NULL"#, vec![])
    .await;

  let route = resolve(&ground, &Ask::key("bmo")).await.unwrap();

  assert_eq!(route.pinned, None, "{route:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_claude_pin_keeps_its_model_at_the_head_of_a_workers_jobs_and_a_runners_key() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground_on(scratch.db().clone(), home.path());

  store::seed::settle(scratch.db(), &deployment(&ground.machine())).await.unwrap();
  order(scratch.db(), &[("claude-code", true), ("registered", true)]).await;
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "pinned_provider", "pinned_model", "support_of")
         VALUES ('helper', 'worker', 'claude-code', 'claude-fable-5-1', NULL),
                ('helper-support', 'worker', NULL, NULL, 'helper'),
                ('plain', 'worker', NULL, NULL, NULL),
                ('search', 'runner', 'claude-code', 'claude-fable-5-1', NULL)"#,
      vec![],
    )
    .await;

  // A runner's agent keeps its fixed model at the head of the key named after it, as any agent does.
  let runner = resolve(&ground, &Ask::key("search")).await.unwrap();

  assert_eq!(
    runner.head().map(|head| (head.provider, head.model)),
    Some(("claude-code".to_owned(), "claude-fable-5-1".to_owned())),
    "{runner:?}"
  );

  // The pin's provider and model head the job's walk, and the job's selection names them.
  for worker in ["helper", "helper-support"] {
    let ask = Ask {
      worker: Some(worker),
      ..Ask::key("job-worker")
    };
    let pinned = resolve(&ground, &ask).await.unwrap();

    assert_eq!(pinned.pinned.as_deref(), Some("claude-code"), "{worker}: {pinned:?}");
    assert_eq!(
      pinned.road().map(|road| (road.provider.as_str(), road.model.as_str())),
      Some(("claude-code", "claude-fable-5-1")),
      "{worker}: {pinned:?}"
    );
    assert_eq!(
      agency_server::queue::routes::resolve_job_model(scratch.db(), None, Some(worker))
        .await
        .unwrap(),
      "claude-code::claude-fable-5-1"
    );
  }

  // A worker with no pin goes out on the job key's tier mapping.
  let ask = Ask {
    worker: Some("plain"),
    ..Ask::key("job-worker")
  };
  let plain = resolve(&ground, &ask).await.unwrap();

  assert_eq!(plain.pinned, None, "{plain:?}");
  assert_ne!(plain.road().map(|road| road.model.as_str()), Some("claude-fable-5-1"), "{plain:?}");
}
