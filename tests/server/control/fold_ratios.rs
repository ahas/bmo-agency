//! The fold ratio of each provider at each tier (docs/sessions.md, 「Session compaction」; docs/api.md,
//! 「fold-ratios」): the table's row first, the provider's own declared share where the table holds none, the
//! general share where the provider declares none either; a session's read by the tier its agent stands at;
//! and the routes a console reads and sets them through.

use agency_server::control::fold_ratios::{Source, admissible, agent_tier, fallback, of_agent, ratio};
use policy::resident_turn::RESIDENT_TURN;
use serde_json::{Value, json};

use crate::support::scratch::Scratch;
use crate::support::standing;

#[test]
fn a_provider_s_own_share_stands_before_the_general_one() {
  assert_eq!(fallback("deepseek"), (0.5, Source::Provider));
  assert_eq!(fallback("openrouter"), (0.5, Source::Provider));
  assert_eq!(fallback("claude-code"), (RESIDENT_TURN.fold_ratio, Source::Default));
  assert_eq!(fallback("registered"), (RESIDENT_TURN.fold_ratio, Source::Default));
  assert_eq!(fallback("no-such-provider"), (RESIDENT_TURN.fold_ratio, Source::Default));
  assert_eq!(RESIDENT_TURN.fold_ratio, 0.85);
}

#[test]
fn a_share_is_more_than_nothing_and_at_most_the_window() {
  assert!(admissible(0.5) && admissible(1.0));
  assert!(!admissible(0.0) && !admissible(1.01) && !admissible(-0.2));
}

#[tokio::test]
async fn the_table_s_row_stands_first_then_the_provider_s_share_then_the_general_one() {
  let scratch = Scratch::new().await;

  scratch
    .run(
      r#"INSERT INTO "fold_ratios" ("provider", "tier", "ratio", "set_at")
         VALUES ('deepseek', 'Frontier', 0.7, 1), ('claude-code', 'Frontier', 0.9, 1)"#,
      vec![],
    )
    .await;

  let db = scratch.db();

  assert_eq!(ratio(db, "deepseek", Some("Frontier")).await.unwrap(), 0.7);
  assert_eq!(ratio(db, "claude-code", Some("Frontier")).await.unwrap(), 0.9);
  assert_eq!(ratio(db, "deepseek", Some("Light")).await.unwrap(), 0.5, "the provider's own share");
  assert_eq!(ratio(db, "claude-code", Some("Light")).await.unwrap(), 0.85, "the general share");
  assert_eq!(ratio(db, "deepseek", None).await.unwrap(), 0.5, "a session at no tier");
}

#[tokio::test]
async fn a_session_reads_the_tier_its_agent_s_work_key_is_assigned() {
  let scratch = Scratch::new().await;

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind") VALUES ('chatter', 'chat'), ('worker-one', 'worker'),
           ('companion', 'chat')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('chatter', 'Supervisor'),
           ('job-worker', 'Frontier')
         ON CONFLICT ("key") DO UPDATE SET "tier" = excluded."tier""#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "fold_ratios" ("provider", "tier", "ratio", "set_at")
         VALUES ('openrouter', 'Supervisor', 0.6, 1), ('openrouter', 'Frontier', 0.4, 1)"#,
      vec![],
    )
    .await;

  let db = scratch.db();

  assert_eq!(agent_tier(db, "chatter").await.unwrap().as_deref(), Some("Supervisor"));
  assert_eq!(
    agent_tier(db, "worker-one").await.unwrap().as_deref(),
    Some("Frontier"),
    "the job key's"
  );
  assert_eq!(agent_tier(db, "companion").await.unwrap(), None);
  assert_eq!(of_agent(db, "openrouter", "chatter").await.unwrap(), 0.6);
  assert_eq!(of_agent(db, "openrouter", "worker-one").await.unwrap(), 0.4);
  assert_eq!(
    of_agent(db, "openrouter", "companion").await.unwrap(),
    0.5,
    "no tier reads the provider's"
  );
  assert_eq!(of_agent(db, "claude-code", "companion").await.unwrap(), 0.85);
}

async fn get(port: u16, token: &str, path: &str) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .get(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

async fn put(port: u16, token: &str, path: &str, body: Value) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .put(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

// The member the list holds for a provider at a tier.
fn member<'a>(listed: &'a Value, provider: &str, tier: &str) -> &'a Value {
  listed["items"]
    .as_array()
    .unwrap()
    .iter()
    .find(|one| one["provider"] == provider && one["tier"] == tier)
    .unwrap_or_else(|| panic!("{provider} at {tier} is listed: {listed}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_console_reads_every_ratio_and_sets_one() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  // Whatever the settling gave, the rows these reads judge are taken away, so each falls back.
  scratch
    .run(
      r#"DELETE FROM "fold_ratios" WHERE "tier" = 'Light' AND "provider" IN ('deepseek', 'claude-code')"#,
      vec![],
    )
    .await;

  let (status, listed) = get(port, token, "/fold-ratios").await;

  assert_eq!(status, 200, "{listed}");
  assert_eq!(listed["more"], json!(false));
  assert_eq!(listed["items"].as_array().unwrap().len(), 5 * 6, "every provider at every tier");
  assert_eq!(
    member(&listed, "deepseek", "Light"),
    &json!({ "provider": "deepseek", "tier": "Light", "ratio": 0.5, "source": "provider" })
  );
  assert_eq!(
    member(&listed, "claude-code", "Light"),
    &json!({ "provider": "claude-code", "tier": "Light", "ratio": 0.85, "source": "default" })
  );

  let (status, set) = put(port, token, "/fold-ratios/deepseek/Light", json!({ "ratio": 0.6 })).await;

  assert_eq!(status, 200, "{set}");
  assert_eq!(set, json!({ "provider": "deepseek", "tier": "Light", "ratio": 0.6, "source": "set" }));

  let (_, listed) = get(port, token, "/fold-ratios").await;

  assert_eq!(member(&listed, "deepseek", "Light")["ratio"], json!(0.6));
  assert_eq!(member(&listed, "deepseek", "Light")["source"], json!("set"));

  let (status, again) = put(port, token, "/fold-ratios/deepseek/Light", json!({ "ratio": 0.55 })).await;

  assert_eq!((status, &again["ratio"]), (200, &json!(0.55)), "a set row is set again");

  for (path, body, status, code) in [
    ("/fold-ratios/deepseek/Light", json!({ "ratio": 1.5 }), 400, "bad-request"),
    ("/fold-ratios/deepseek/Light", json!({ "ratio": 0 }), 400, "bad-request"),
    ("/fold-ratios/deepseek/Light", json!({}), 400, "bad-request"),
    ("/fold-ratios/nobody/Light", json!({ "ratio": 0.5 }), 404, "not-found"),
    ("/fold-ratios/deepseek/Nowhere", json!({ "ratio": 0.5 }), 404, "not-found"),
  ] {
    let (answered, refused) = put(port, token, path, body.clone()).await;

    assert_eq!((answered, refused["code"].as_str()), (status, Some(code)), "{path} {body}: {refused}");
  }
}
