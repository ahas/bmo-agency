//! The seeds a store is given as it is settled (docs/store.md, 「Its seeds」): each written once, and a
//! row the owner removed left removed.

use store::seed::{
  AccountSeed, BasePromptSeed, CatalogueSeed, LaterSeed, Seeds, TierMapping, TierPlan, WorkKey, settle,
};

use crate::scratch::Scratch;

fn mapping(
  provider: &str,
  tier: &str,
  model: &str,
  effort: Option<&str>,
  answer_tokens: Option<i64>,
) -> TierMapping {
  TierMapping {
    provider: provider.into(),
    tier: tier.into(),
    model: model.into(),
    effort: effort.map(Into::into),
    answer_tokens,
  }
}

fn key(key: &str, tier: &str) -> WorkKey {
  WorkKey {
    key: key.into(),
    tier: tier.into(),
  }
}

fn seeds() -> Seeds {
  Seeds {
    account: Some(AccountSeed {
      id: "default".into(),
      name: "Default account".into(),
      credential_kind: "directory".into(),
      credential_path: "$HOME/.claude".into(),
    }),
    tiers: TierPlan {
      ladder: vec!["Top".into(), "Low".into()],
      mappings: vec![
        mapping("first", "Top", "first-large", Some("high"), None),
        mapping("first", "Low", "first-small", None, None),
        mapping("engine", "Top", "engine-old", Some("thinking"), Some(4096)),
      ],
      keys: vec![key("chat", "Top"), key("gate", "Low")],
      later: vec![],
    },
    catalogue: vec![CatalogueSeed {
      name: "entry".into(),
      label: "Entry".into(),
      file: "entry.gguf".into(),
      bytes: 1_024,
      sha256: "00".repeat(32),
      source: Some("https://example.com/entry.gguf".into()),
      engine: Some("engine".into()),
      registration: Some("engine".into()),
      thinking: Some(false),
      temperature: Some(0.7),
      top_p: None,
      presence_penalty: None,
      max_tokens: Some(512),
      note: None,
    }],
    base_prompts: vec![
      BasePromptSeed {
        name: "harness".into(),
        label: "Harness based".into(),
        text: "You are a harness.".into(),
        agents: vec!["worker".into()],
      },
      BasePromptSeed {
        name: "other".into(),
        label: "Other based".into(),
        text: "You are another harness.".into(),
        agents: vec![],
      },
    ],
  }
}

async fn assignment(scratch: &Scratch, key: &str) -> Option<String> {
  store::one(
    scratch.db(),
    r#"SELECT "tier" FROM "model_tier_assignments" WHERE "key" = $1"#,
    vec![key.into()],
  )
  .await
  .unwrap()
  .map(|row| row.try_get::<String>("", "tier").unwrap())
}

async fn mapped(
  scratch: &Scratch,
  provider: &str,
  tier: &str,
) -> Option<(String, Option<String>, Option<i64>)> {
  store::one(
    scratch.db(),
    r#"SELECT "model", "effort", "answer_tokens" FROM "model_tier_mappings" WHERE "provider" = $1 AND "tier" = $2"#,
    vec![provider.into(), tier.into()],
  )
  .await
  .unwrap()
  .map(|row| {
    (
      row.try_get::<String>("", "model").unwrap(),
      row.try_get::<Option<String>>("", "effort").unwrap(),
      row.try_get::<Option<i64>>("", "answer_tokens").unwrap(),
    )
  })
}

#[tokio::test]
async fn a_fresh_store_is_given_every_seed() {
  let scratch = Scratch::blank().await;

  settle(scratch.db(), &seeds()).await.unwrap();

  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "claude_accounts" WHERE "id" = 'default' AND "place" = 1"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "model_catalogue" WHERE "name" = 'entry' AND "nodes" = '[]'"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "model_tier_meta""#, vec![]).await, 3);
  assert_eq!(
    mapped(&scratch, "first", "Top").await,
    Some(("first-large".into(), Some("high".into()), None))
  );
  assert_eq!(assignment(&scratch, "chat").await.as_deref(), Some("Top"));
  assert_eq!(assignment(&scratch, "gate").await.as_deref(), Some("Low"));
}

#[tokio::test]
async fn what_the_owner_removed_stays_removed_and_what_the_owner_set_stands() {
  let scratch = Scratch::blank().await;

  settle(scratch.db(), &seeds()).await.unwrap();
  scratch.run(r#"DELETE FROM "model_catalogue""#, vec![]).await;
  scratch.run(r#"DELETE FROM "claude_accounts""#, vec![]).await;
  scratch
    .run(
      r#"INSERT INTO "claude_accounts" ("id", "name", "place", "credential_kind", "credential_path", "added_at")
         VALUES ('mine', 'Mine', 1, 'directory', '/elsewhere', 1)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"DELETE FROM "model_tier_mappings" WHERE "provider" = 'first' AND "tier" = 'Low'"#,
      vec![],
    )
    .await;
  scratch
    .run(r#"DELETE FROM "model_tier_assignments" WHERE "key" = 'gate'"#, vec![])
    .await;
  scratch
    .run(r#"UPDATE "model_tier_assignments" SET "tier" = 'Low' WHERE "key" = 'chat'"#, vec![])
    .await;

  settle(scratch.db(), &seeds()).await.unwrap();

  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "model_catalogue""#, vec![]).await, 0);
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "claude_accounts""#, vec![]).await, 1);
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "claude_accounts" WHERE "id" = 'mine'"#, vec![])
      .await,
    1
  );
  assert_eq!(mapped(&scratch, "first", "Low").await, None);
  assert_eq!(assignment(&scratch, "gate").await, None);
  assert_eq!(assignment(&scratch, "chat").await.as_deref(), Some("Low"));
}

#[tokio::test]
async fn a_store_holding_no_account_is_given_the_seed_again() {
  let scratch = Scratch::blank().await;

  settle(scratch.db(), &seeds()).await.unwrap();
  scratch.run(r#"DELETE FROM "claude_accounts""#, vec![]).await;
  settle(scratch.db(), &seeds()).await.unwrap();

  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "claude_accounts" WHERE "id" = 'default'"#, vec![])
      .await,
    1
  );
}

#[tokio::test]
async fn a_key_declared_later_is_given_its_tier_once() {
  let scratch = Scratch::blank().await;

  settle(scratch.db(), &seeds()).await.unwrap();

  let mut later = seeds();
  later.tiers.keys.push(key("viewer", "Low"));
  settle(scratch.db(), &later).await.unwrap();
  assert_eq!(assignment(&scratch, "viewer").await.as_deref(), Some("Low"));

  scratch
    .run(r#"DELETE FROM "model_tier_assignments" WHERE "key" = 'viewer'"#, vec![])
    .await;
  settle(scratch.db(), &later).await.unwrap();
  assert_eq!(assignment(&scratch, "viewer").await, None);
}

#[tokio::test]
async fn a_provider_declared_later_maps_every_tier_once_carrying_what_it_names() {
  let scratch = Scratch::blank().await;

  settle(scratch.db(), &seeds()).await.unwrap();

  let mut later = seeds();
  later.tiers.later.push(LaterSeed {
    provider: "registry".into(),
    model: "registered".into(),
    effort: Some("no-thinking".into()),
    carried_from: Some("engine".into()),
  });
  settle(scratch.db(), &later).await.unwrap();

  assert_eq!(
    mapped(&scratch, "registry", "Top").await,
    Some(("registered".into(), Some("thinking".into()), Some(4096)))
  );
  assert_eq!(
    mapped(&scratch, "registry", "Low").await,
    Some(("registered".into(), Some("no-thinking".into()), None))
  );

  scratch
    .run(
      r#"DELETE FROM "model_tier_mappings" WHERE "provider" = 'registry' AND "tier" = 'Low'"#,
      vec![],
    )
    .await;
  settle(scratch.db(), &later).await.unwrap();
  assert_eq!(mapped(&scratch, "registry", "Low").await, None);
}

#[tokio::test]
async fn a_plan_naming_a_tier_off_its_ladder_is_refused_before_anything_is_written() {
  let scratch = Scratch::blank().await;
  let mut wrong = seeds();

  wrong.tiers.keys.push(key("stray", "Middle"));

  let refused = settle(scratch.db(), &wrong).await.unwrap_err();
  assert!(refused.detail.contains("Middle"), "{refused}");
  assert_eq!(
    scratch
      .count(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = current_schema()",
        vec![]
      )
      .await,
    0
  );
}

async fn prompt_text(scratch: &Scratch, name: &str) -> Option<String> {
  store::one(
    scratch.db(),
    r#"SELECT "text" FROM "base_prompts" WHERE "name" = $1"#,
    vec![name.into()],
  )
  .await
  .unwrap()
  .map(|row| row.try_get::<String>("", "text").unwrap())
}

async fn choice(scratch: &Scratch, agent: &str) -> Option<String> {
  store::one(
    scratch.db(),
    r#"SELECT "prompt" FROM "agent_base_prompts" WHERE "agent" = $1"#,
    vec![agent.into()],
  )
  .await
  .unwrap()
  .map(|row| row.try_get::<String>("", "prompt").unwrap())
}

#[tokio::test]
async fn a_base_prompt_declared_later_reaches_a_standing_store_once() {
  let scratch = Scratch::blank().await;
  let mut before = seeds();

  before.base_prompts.clear();
  settle(scratch.db(), &before).await.unwrap();
  assert_eq!(prompt_text(&scratch, "harness").await, None);

  settle(scratch.db(), &seeds()).await.unwrap();
  assert_eq!(prompt_text(&scratch, "harness").await.as_deref(), Some("You are a harness."));
  assert_eq!(prompt_text(&scratch, "other").await.as_deref(), Some("You are another harness."));
  assert_eq!(choice(&scratch, "worker").await.as_deref(), Some("harness"));

  scratch
    .run(
      r#"UPDATE "base_prompts" SET "text" = 'The owner''s own.' WHERE "name" = 'harness'"#,
      vec![],
    )
    .await;
  scratch.run(r#"DELETE FROM "base_prompts" WHERE "name" = 'other'"#, vec![]).await;
  scratch.run(r#"DELETE FROM "agent_base_prompts""#, vec![]).await;
  settle(scratch.db(), &seeds()).await.unwrap();

  assert_eq!(prompt_text(&scratch, "harness").await.as_deref(), Some("The owner's own."));
  assert_eq!(prompt_text(&scratch, "other").await, None);
  assert_eq!(choice(&scratch, "worker").await, None);
}

#[tokio::test]
async fn a_prompt_already_kept_under_a_seeds_name_is_left_and_gives_no_choice() {
  let scratch = Scratch::blank().await;
  let mut before = seeds();

  before.base_prompts.clear();
  settle(scratch.db(), &before).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "base_prompts" ("name", "label", "text") VALUES ('harness', 'Mine', 'Kept before.')"#,
      vec![],
    )
    .await;
  settle(scratch.db(), &seeds()).await.unwrap();

  assert_eq!(prompt_text(&scratch, "harness").await.as_deref(), Some("Kept before."));
  assert_eq!(choice(&scratch, "worker").await, None);
}
