//! The seeds a store is given as it is settled (docs/store.md, 「Its seeds」): each written once, and a
//! row the owner removed left removed.

use store::seed::{
  AccountSeed, BasePromptSeed, CatalogueSeed, CoworkerTier, FoldRatioSeed, LaterSeed, RoleSeed, Seeds,
  TierMapping, TierPlan, WorkKey, settle,
};

use crate::support::scratch::Scratch;

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
    thinking: None,
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
      coworker: vec![CoworkerTier {
        tier: "Low".into(),
        capacity: 12,
      }],
    },
    fold_ratios: vec![
      FoldRatioSeed {
        provider: "first".into(),
        ratio: 0.85,
      },
      FoldRatioSeed {
        provider: "engine".into(),
        ratio: 0.5,
      },
    ],
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
    roles: vec![
      RoleSeed {
        name: "talker".into(),
        label: "Talker".into(),
        bundles: vec!["room.send".into()],
        max_concurrent: Some(3),
        prompt: None,
        sessions: "keep".into(),
      },
      RoleSeed {
        name: "maker".into(),
        label: "Maker".into(),
        bundles: vec![],
        max_concurrent: None,
        prompt: Some("Make things.".into()),
        sessions: "fresh".into(),
      },
    ],
  }
}

// A role's label, bundles, bound and prompt as the store holds it, with its place in the listing.
async fn role(scratch: &Scratch, name: &str) -> Option<(String, String, Option<i64>, Option<String>, i64)> {
  store::one(
    scratch.db(),
    r#"SELECT "label", "bundles", "max_concurrent", "prompt", "position" FROM "agent_roles" WHERE "name" = $1"#,
    vec![name.into()],
  )
  .await
  .unwrap()
  .map(|row| {
    (
      row.try_get("", "label").unwrap(),
      row.try_get("", "bundles").unwrap(),
      row.try_get("", "max_concurrent").unwrap(),
      row.try_get("", "prompt").unwrap(),
      row.try_get("", "position").unwrap(),
    )
  })
}

#[tokio::test]
async fn the_roles_are_given_once_and_a_row_the_store_holds_wins_over_the_seed() {
  let scratch = Scratch::blank().await;

  settle(scratch.db(), &seeds()).await.unwrap();

  assert_eq!(
    role(&scratch, "talker").await,
    Some(("Talker".into(), r#"["room.send"]"#.into(), Some(3), None, 0))
  );
  assert_eq!(
    role(&scratch, "maker").await,
    Some(("Maker".into(), "[]".into(), None, Some("Make things.".into()), 1))
  );

  // The owner's change stands, and a role the owner removed stays removed.
  scratch
    .run(
      r#"UPDATE "agent_roles" SET "label" = 'Speaker', "max_concurrent" = 1 WHERE "name" = 'talker'"#,
      vec![],
    )
    .await;
  scratch.run(r#"DELETE FROM "agent_roles" WHERE "name" = 'maker'"#, vec![]).await;

  let mut later = seeds();
  later.roles[0].label = "Changed in the policy".into();
  later.roles.push(RoleSeed {
    name: "late".into(),
    label: "Late".into(),
    bundles: vec![],
    max_concurrent: None,
    prompt: None,
    sessions: "keep".into(),
  });
  settle(scratch.db(), &later).await.unwrap();

  assert_eq!(
    role(&scratch, "talker").await.map(|(label, _, bound, ..)| (label, bound)),
    Some(("Speaker".into(), Some(1)))
  );
  assert_eq!(role(&scratch, "maker").await, None);
  assert_eq!(role(&scratch, "late").await.map(|(label, ..)| label), Some("Late".into()));
}

async fn sessions(scratch: &Scratch, name: &str) -> Option<String> {
  store::one(
    scratch.db(),
    r#"SELECT "sessions" FROM "agent_roles" WHERE "name" = $1"#,
    vec![name.into()],
  )
  .await
  .unwrap()
  .map(|row| row.try_get("", "sessions").unwrap())
}

#[tokio::test]
async fn a_role_is_seeded_with_whether_its_runs_keep_their_sessions() {
  let scratch = Scratch::blank().await;

  settle(scratch.db(), &seeds()).await.unwrap();

  assert_eq!(sessions(&scratch, "talker").await.as_deref(), Some("keep"));
  assert_eq!(sessions(&scratch, "maker").await.as_deref(), Some("fresh"));
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
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "model_tier_meta""#, vec![]).await, 4);
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

async fn fold_ratio(scratch: &Scratch, provider: &str, tier: &str) -> Option<f64> {
  store::one(
    scratch.db(),
    r#"SELECT "ratio" FROM "fold_ratios" WHERE "provider" = $1 AND "tier" = $2"#,
    vec![provider.into(), tier.into()],
  )
  .await
  .unwrap()
  .map(|row| row.try_get::<f64>("", "ratio").unwrap())
}

#[tokio::test]
async fn every_provider_is_given_its_fold_ratio_at_every_tier_once() {
  let scratch = Scratch::blank().await;

  settle(scratch.db(), &seeds()).await.unwrap();

  for tier in ["Top", "Low"] {
    assert_eq!(fold_ratio(&scratch, "first", tier).await, Some(0.85));
    assert_eq!(fold_ratio(&scratch, "engine", tier).await, Some(0.5));
  }

  scratch
    .run(
      r#"UPDATE "fold_ratios" SET "ratio" = 0.7 WHERE "provider" = 'first' AND "tier" = 'Top'"#,
      vec![],
    )
    .await;
  scratch
    .run(r#"DELETE FROM "fold_ratios" WHERE "provider" = 'engine' AND "tier" = 'Low'"#, vec![])
    .await;

  let mut later = seeds();
  later.fold_ratios.push(FoldRatioSeed {
    provider: "late".into(),
    ratio: 0.6,
  });
  settle(scratch.db(), &later).await.unwrap();

  assert_eq!(fold_ratio(&scratch, "first", "Top").await, Some(0.7), "what the owner set stands");
  assert_eq!(fold_ratio(&scratch, "engine", "Low").await, None, "a provider is seeded once");
  assert_eq!(
    fold_ratio(&scratch, "late", "Low").await,
    Some(0.6),
    "a provider declared later is seeded"
  );
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
async fn a_provider_declared_later_maps_the_tiers_it_names_once() {
  let scratch = Scratch::blank().await;

  settle(scratch.db(), &seeds()).await.unwrap();

  let mut later = seeds();
  later.tiers.later.push(LaterSeed {
    provider: "registry".into(),
    mappings: vec![
      mapping("registry", "Top", "registered", Some("thinking"), Some(4096)),
      mapping("registry", "Low", "registered", Some("no-thinking"), None),
    ],
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
  // The co-worker pool's starting place at the tiers a co-worker is borrowed at.
  assert_eq!(
    scratch
      .count(
        r#"SELECT "capacity" FROM "coworker_pool_tier_limits" WHERE "provider" = 'registry' AND "tier" = 'Low'"#,
        vec![]
      )
      .await,
    12
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
