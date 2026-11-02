//! The provider order (docs/provider-order.md): a row's run goes out on the first provider of its own order
//! that has quota above its share and can answer now, every provider it passes named with why, the share read
//! against the window with the least left, a balance at or below zero closing its provider, and a road its
//! caller fixed walked alone — waited for, never left.

use agency_server::order::{Entry, Order, least_left, quota, window_left};
use agency_server::road::*;
use cache_siren::{KeptWindow, Reading, Slot, Window};

use crate::support::grounds::{deepseek_balance, seeded};
use crate::support::scratch::Scratch;

fn entry(provider: &str, model: &str) -> Entry {
  Entry {
    provider: provider.into(),
    model: model.into(),
    effort: None,
    thinking: None,
    answer_tokens: None,
  }
}

fn order(entries: &[(&str, &str)], share: f64) -> Order {
  Order {
    entries: entries.iter().map(|(provider, model)| entry(provider, model)).collect(),
    quota_share: share,
  }
}

fn step(provider: &str, answer: Answer) -> Step {
  Step {
    provider: provider.into(),
    entry: entry(provider, &format!("{provider}-model")),
    answer,
  }
}

fn road(provider: &str) -> Road {
  Road {
    provider: provider.into(),
    model: format!("{provider}-model"),
    account: None,
  }
}

fn resolution(steps: Vec<Step>) -> Resolution {
  Resolution {
    row: "worker".into(),
    supervisor: false,
    steps,
    line: "worker".into(),
    uncarried: vec![],
    fixed: false,
  }
}

#[test]
fn the_first_provider_that_can_answer_carries_the_run_and_every_one_passed_says_why() {
  let walked = resolution(vec![
    step("deepseek", Answer::Cannot(Closed::plain("deepseek", "its balance is spent"))),
    step("codex", Answer::NoRoad),
    step("claude-code", Answer::Can(road("claude-code"))),
    step("openrouter", Answer::Can(road("openrouter"))),
  ]);

  assert_eq!(walked.road(), Some(&road("claude-code")));
  assert_eq!(walked.taken().map(|step| step.entry.model.as_str()), Some("claude-code-model"));
  assert_eq!(
    walked.passed().iter().map(|closed| closed.reason.as_str()).collect::<Vec<_>>(),
    vec!["its balance is spent"]
  );
  assert_eq!(
    walked.head().map(|head| head.provider),
    Some("deepseek".to_owned()),
    "the order's top heads it"
  );
}

#[test]
fn an_order_none_of_whose_providers_can_answer_is_closed_with_every_reason() {
  let closed = resolution(vec![
    step("deepseek", Answer::Cannot(Closed::plain("deepseek", "its balance is spent"))),
    step(
      "claude-code",
      Answer::Cannot(Closed {
        reopens_at: Some(1_900_000_000),
        ..Closed::plain("claude-code", "1% of its quota is left, at or below the share of 1%")
      }),
    ),
  ]);

  assert!(closed.road().is_none());
  assert_eq!(closed.reopens_at(), Some(1_900_000_000));
  assert!(closed.sentence().contains("deepseek: its balance is spent"));
  assert!(closed.sentence().contains("claude-code: 1% of its quota is left"));
}

fn kept(name: &str, used: f64, resets_at: Option<i64>) -> KeptWindow {
  KeptWindow {
    window: Window {
      name: name.into(),
      utilization: used,
      resets_at,
      duration_minutes: None,
      limit_name: None,
    },
    seen_at: 1_000,
  }
}

#[test]
fn a_provider_has_left_what_its_window_with_the_least_left_has_left() {
  let reading = Reading {
    windows: vec![kept("five_hour", 0.25, Some(5_000)), kept("seven_day", 0.9, Some(9_000))],
    fresh: std::time::Duration::from_secs(10_000),
    unread_at: None,
  };
  let (left, window, resets_at) = least_left(&reading, 1_000, |_| 1.0).unwrap();

  assert!((left - 0.1).abs() < 1e-9);
  assert_eq!(window, "seven_day");
  assert_eq!(resets_at, Some(9_000));
}

#[test]
fn a_window_at_its_ceiling_has_nothing_left_and_one_reset_counts_for_nothing() {
  assert_eq!(window_left(0.97, Some(5_000), 0.95, 1_000), Some(0.0), "at its ceiling: nothing left");
  assert_eq!(window_left(0.97, Some(500), 0.95, 1_000), None, "reset by now: it counts for nothing");
  assert_eq!(window_left(0.4, None, 1.0, 1_000), Some(0.6));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_window_provider_has_quota_while_its_least_left_stands_above_the_share() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let codex = harness_providers::provider("codex").unwrap();
  let now = agency_server::now_ms() / 1000;

  ground
    .windows
    .replace(
      Slot {
        mode: "codex",
        account: "",
      },
      &[Window {
        name: "codex:primary".into(),
        utilization: 0.98,
        resets_at: Some(now + 3_600),
        duration_minutes: None,
        limit_name: None,
      }],
      now,
    )
    .await
    .unwrap();

  let spent = quota(&ground, codex, 0.05, None).await;
  let enough = quota(&ground, codex, 0.01, None).await;

  assert!(!spent.has, "2% left stands at or below a 5% share: {spent:?}");
  assert_eq!(spent.window.as_deref(), Some("codex:primary"));
  assert_eq!(spent.resets_at, Some(now + 3_600));
  assert!(enough.has, "2% left stands above a 1% share: {enough:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_balance_at_zero_closes_its_provider_and_a_balance_topped_up_reopens_it() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;
  let walked = order(&[("deepseek", "deepseek-flash")], 0.01);
  let ask = Ask::of(Row::Role("worker"), "worker", &walked);

  deepseek_balance(&ground, 0.0).await;

  let spent = resolve(&ground, &ask).await.unwrap();

  assert!(spent.road().is_none(), "{spent:?}");
  assert!(spent.closed()[0].reason.contains("balance is spent"), "{spent:?}");

  deepseek_balance(&ground, 5.0).await;

  let topped = resolve(&ground, &ask).await.unwrap();

  assert_eq!(topped.road().map(|road| road.provider.as_str()), Some("deepseek"), "{topped:?}");
  assert_eq!(topped.road().map(|road| road.model.as_str()), Some("deepseek-flash"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_seeded_worker_role_goes_out_on_deepseeks_flash_model_first() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;

  let resolution = for_job(&ground, None, None, None, &[]).await.unwrap();

  assert_eq!(resolution.row, "worker");
  assert_eq!(
    resolution.road().map(|road| road.model.as_str()),
    Some("deepseek-flash"),
    "{resolution:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_repositorys_pinned_model_is_walked_alone_and_its_work_waits_rather_than_falling_back() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&ground, 5.0).await;
  scratch
    .run(
      r#"INSERT INTO "repositories" ("name", "path", "pinned_provider", "pinned_model", "created_at", "updated_at")
         VALUES ('pinned', '/checkouts/pinned', 'registered', 'no-such-registration', 0, 0)"#,
      vec![],
    )
    .await;

  let pinned = for_job(&ground, None, Some("pinned"), None, &[]).await.unwrap();

  assert!(pinned.fixed);
  assert_eq!(pinned.steps.len(), 1, "the pin alone is walked: {pinned:?}");
  assert!(
    pinned.road().is_none(),
    "the role's order could answer, and the pin's work waits all the same"
  );

  let unpinned = for_job(&ground, None, None, None, &[]).await.unwrap();

  assert_eq!(unpinned.road().map(|road| road.provider.as_str()), Some("deepseek"));
}
