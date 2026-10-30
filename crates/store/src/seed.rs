//! What a store is given beside its tables (docs/store.md, 「Its seeds」): the account, the tiers with
//! their work keys, the fold ratios, the model catalogue, the base prompts and the agents' roles. The
//! values are the deployment's, handed in by whoever settles the store; this module knows none of them and
//! supplies only when and how each is written.

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait, Value};
use sea_orm_migration::MigratorTrait;

use crate::migration::{Migrator, TABLES};
use crate::statements::{Fault, all, one, run};

/// Everything a store is seeded with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seeds {
  /// The account a store holding none is given.
  pub account: Option<AccountSeed>,
  pub tiers: TierPlan,
  /// Each provider's fold ratio, given at every tier of the ladder once.
  pub fold_ratios: Vec<FoldRatioSeed>,
  /// The entries the catalogue is given once, as its table is made.
  pub catalogue: Vec<CatalogueSeed>,
  /// The base prompts each given once by its name.
  pub base_prompts: Vec<BasePromptSeed>,
  /// The roles an agent may hold, each given once by its name, in the order a screen lists them.
  pub roles: Vec<RoleSeed>,
}

/// One role a store is given once by its name (docs/agent-roles.md, 「The roles」): its label, the bundles
/// it gives, the most of its agents that run at once and the text of its prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleSeed {
  pub name: String,
  pub label: String,
  pub bundles: Vec<String>,
  pub max_concurrent: Option<i64>,
  pub prompt: Option<String>,
}

/// One base prompt a store is given once by its name (docs/base-prompts.md, 「The seeds」), with the
/// agents given the choice of it where it is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasePromptSeed {
  pub name: String,
  pub label: String,
  pub text: String,
  pub agents: Vec<String>,
}

/// The share of a model's window a provider's sessions may fill before they are folded, which every tier of
/// the ladder is given on it once (docs/sessions.md, 「Session compaction」).
#[derive(Clone, Debug, PartialEq)]
pub struct FoldRatioSeed {
  pub provider: String,
  pub ratio: f64,
}

/// The one account a store holding none is given: the login the machine already has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountSeed {
  pub id: String,
  pub name: String,
  pub credential_kind: String,
  pub credential_path: String,
}

/// The deployment's plan for its tiers: which there are, what each maps to on every provider, and
/// where each work key starts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TierPlan {
  /// Every tier, in the order the tiers are shown and compared.
  pub ladder: Vec<String>,
  /// What each tier maps to on each provider, written once, with the first seed.
  pub mappings: Vec<TierMapping>,
  /// Every work key with the tier it starts on.
  pub keys: Vec<WorkKey>,
  /// Each provider declared after the first seed, written once the first time the store is settled
  /// with it.
  pub later: Vec<LaterSeed>,
  /// The tiers a co-worker is borrowed at, with the places each starts with, which a provider declared
  /// later is given once.
  pub coworker: Vec<CoworkerTier>,
}

/// One tier a co-worker is borrowed at and where its pool starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoworkerTier {
  pub tier: String,
  pub capacity: i64,
}

/// What a tier maps to on one provider.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TierMapping {
  pub provider: String,
  pub tier: String,
  pub model: String,
  pub thinking: Option<bool>,
  pub effort: Option<String>,
  pub answer_tokens: Option<i64>,
}

/// A work key and the tier it starts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkKey {
  pub key: String,
  pub tier: String,
}

/// A provider declared after the first seed: what each tier maps to on it, written once the first time
/// the store is settled with the provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaterSeed {
  pub provider: String,
  pub mappings: Vec<TierMapping>,
}

/// One model file of the catalogue (docs/model-catalogue.md); it names no node, since where a file lies
/// is a fact of the machines.
#[derive(Clone, Debug, PartialEq)]
pub struct CatalogueSeed {
  pub name: String,
  pub label: String,
  pub file: String,
  pub bytes: i64,
  pub sha256: String,
  pub source: Option<String>,
  pub engine: Option<String>,
  pub registration: Option<String>,
  pub thinking: Option<bool>,
  pub temperature: Option<f64>,
  pub top_p: Option<f64>,
  pub presence_penalty: Option<f64>,
  pub max_tokens: Option<i64>,
  pub note: Option<String>,
}

/// The store brought to this build's shape and its seeds given, as one settling: the migrations run,
/// then every seed in one transaction. A seed given once is not given again, so a row the owner removed
/// stays removed.
pub async fn settle(db: &DatabaseConnection, seeds: &Seeds) -> Result<(), Fault> {
  check(&seeds.tiers)?;

  let made = Migrator::get_pending_migrations(db)
    .await?
    .iter()
    .any(|pending| pending.name() == TABLES);

  Migrator::up(db, None).await?;

  let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs() as i64);
  let writing = db.begin().await?;

  if made {
    for entry in &seeds.catalogue {
      catalogue_entry(&writing, entry, now).await?;
    }
  }
  if let Some(account) = &seeds.account {
    account_seed(&writing, account, now).await?;
  }
  first_tiers(&writing, &seeds.tiers).await?;
  backfill_keys(&writing, &seeds.tiers).await?;
  settle_ladder(&writing, &seeds.tiers).await?;
  later_tiers(&writing, &seeds.tiers, now).await?;
  fold_ratios(&writing, &seeds.tiers.ladder, &seeds.fold_ratios, now).await?;
  for prompt in &seeds.base_prompts {
    base_prompt_seed(&writing, prompt, now).await?;
  }
  for (place, role) in seeds.roles.iter().enumerate() {
    role_seed(&writing, role, place as i64, now * 1000).await?;
  }

  writing.commit().await?;

  Ok(())
}

// A plan naming a tier its ladder does not hold is refused before anything is written.
fn check(plan: &TierPlan) -> Result<(), Fault> {
  let named = plan
    .mappings
    .iter()
    .map(|mapping| &mapping.tier)
    .chain(plan.later.iter().flat_map(|later| later.mappings.iter().map(|mapping| &mapping.tier)))
    .chain(plan.keys.iter().map(|key| &key.tier));

  for tier in named {
    if !plan.ladder.contains(tier) {
      return Err(Fault::new(format!("the tier plan names {tier}, which its ladder does not hold")));
    }
  }

  Ok(())
}

fn text(value: &Option<String>) -> Value {
  Value::from(value.clone())
}

async fn catalogue_entry<C: ConnectionTrait>(db: &C, entry: &CatalogueSeed, now: i64) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "model_catalogue" ("name", "label", "file", "bytes", "sha256", "source", "engine",
         "registration", "thinking", "temperature", "top_p", "presence_penalty", "max_tokens", "note", "added_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"#,
    vec![
      entry.name.clone().into(),
      entry.label.clone().into(),
      entry.file.clone().into(),
      entry.bytes.into(),
      entry.sha256.clone().into(),
      text(&entry.source),
      text(&entry.engine),
      text(&entry.registration),
      entry.thinking.map(i64::from).into(),
      entry.temperature.into(),
      entry.top_p.into(),
      entry.presence_penalty.into(),
      entry.max_tokens.into(),
      text(&entry.note),
      now.into(),
    ],
  )
  .await?;

  Ok(())
}

// The account is given to a store holding none; a list the owner has written to is left alone.
async fn account_seed<C: ConnectionTrait>(db: &C, account: &AccountSeed, now: i64) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "claude_accounts" ("id", "name", "place", "enabled", "credential_kind", "credential_path", "added_at")
       SELECT $1, $2, 1, 1, $3, $4, $5
       WHERE NOT EXISTS (SELECT 1 FROM "claude_accounts")"#,
    vec![
      account.id.clone().into(),
      account.name.clone().into(),
      account.credential_kind.clone().into(),
      account.credential_path.clone().into(),
      now.into(),
    ],
  )
  .await?;

  Ok(())
}

// A base prompt is given once by its name, recorded as given, so one the owner removes stays removed. A
// prompt of that name already kept — the owner's own, or one an earlier store held — is left as it stands,
// and only a prompt written now gives its agents the choice of it; a choice an agent already holds stands.
async fn base_prompt_seed<C: ConnectionTrait>(
  db: &C,
  prompt: &BasePromptSeed,
  now: i64,
) -> Result<(), Fault> {
  let given = run(
    db,
    r#"INSERT INTO "base_prompt_seeded" ("name") VALUES ($1) ON CONFLICT DO NOTHING"#,
    vec![prompt.name.clone().into()],
  )
  .await?;

  if given == 0 {
    return Ok(());
  }

  let written = run(
    db,
    r#"INSERT INTO "base_prompts" ("name", "label", "text", "updated_at") VALUES ($1, $2, $3, $4)
       ON CONFLICT DO NOTHING"#,
    vec![
      prompt.name.clone().into(),
      prompt.label.clone().into(),
      prompt.text.clone().into(),
      now.into(),
    ],
  )
  .await?;

  if written == 0 {
    return Ok(());
  }

  for agent in &prompt.agents {
    run(
      db,
      r#"INSERT INTO "agent_base_prompts" ("agent", "prompt") VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
      vec![agent.clone().into(), prompt.name.clone().into()],
    )
    .await?;
  }

  Ok(())
}

// A role is given once by its name, recorded as given, so one the owner removes stays removed; a role of
// that name already held stands as it is. Its place is the one the policy declares it at, which a role the
// owner adds later comes after.
async fn role_seed<C: ConnectionTrait>(
  db: &C,
  role: &RoleSeed,
  place: i64,
  at_ms: i64,
) -> Result<(), Fault> {
  let given = run(
    db,
    r#"INSERT INTO "agent_role_seeded" ("name") VALUES ($1) ON CONFLICT DO NOTHING"#,
    vec![role.name.clone().into()],
  )
  .await?;

  if given == 0 {
    return Ok(());
  }

  let bundles = serde_json::to_string(&role.bundles).map_err(|error| Fault::new(error.to_string()))?;

  run(
    db,
    r#"INSERT INTO "agent_roles" ("name", "label", "bundles", "max_concurrent", "prompt", "position", "created_at", "updated_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $7) ON CONFLICT DO NOTHING"#,
    vec![
      role.name.clone().into(),
      role.label.clone().into(),
      bundles.into(),
      role.max_concurrent.into(),
      text(&role.prompt),
      place.into(),
      at_ms.into(),
    ],
  )
  .await?;

  Ok(())
}

// The first seed: every mapping, and the tier store's versions recorded, once. The keys' assignments
// are given beside it, as every key's is (`backfill_keys`).
async fn first_tiers<C: ConnectionTrait>(db: &C, plan: &TierPlan) -> Result<(), Fault> {
  if one(db, r#"SELECT 1 FROM "model_tier_meta" WHERE "version" = 1"#, vec![])
    .await?
    .is_some()
  {
    return Ok(());
  }

  for mapping in &plan.mappings {
    run(
      db,
      r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "thinking", "effort", "answer_tokens")
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING"#,
      vec![
        mapping.provider.clone().into(),
        mapping.tier.clone().into(),
        mapping.model.clone().into(),
        mapping.thinking.into(),
        text(&mapping.effort),
        mapping.answer_tokens.into(),
      ],
    )
    .await?;
  }
  run(
    db,
    r#"INSERT INTO "model_tier_meta" ("version") VALUES (1), (2), (3) ON CONFLICT DO NOTHING"#,
    vec![],
  )
  .await?;

  Ok(())
}

// Each work key gets the tier it starts on once, at the first seed or the first settling after the plan
// declared it: it is recorded as given, so an assignment the owner deletes stays deleted, and one that
// stands is never touched.
async fn backfill_keys<C: ConnectionTrait>(db: &C, plan: &TierPlan) -> Result<(), Fault> {
  for key in &plan.keys {
    let given = run(
      db,
      r#"INSERT INTO "model_tier_backfilled" ("key") VALUES ($1) ON CONFLICT DO NOTHING"#,
      vec![key.key.clone().into()],
    )
    .await?;

    if given == 1 {
      run(
        db,
        r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
        vec![key.key.clone().into(), key.tier.clone().into()],
      )
      .await?;
    }
  }

  Ok(())
}

// The ladder is the policy's: a tier it has gained is mapped on every provider the store already carries,
// and a store settled under an earlier ladder keeps no mapping or assignment on a tier the ladder no
// longer holds. An assignment moves to the tier its key starts on, and the mappings of a tier left behind
// are dropped. Recorded once, so a row the owner writes later stands.
async fn settle_ladder<C: ConnectionTrait>(db: &C, plan: &TierPlan) -> Result<(), Fault> {
  let given = run(
    db,
    r#"INSERT INTO "model_tier_meta" ("version") VALUES (4) ON CONFLICT DO NOTHING"#,
    vec![],
  )
  .await?;

  if given == 0 {
    return Ok(());
  }

  let mapped = all(db, r#"SELECT DISTINCT "provider", "tier" FROM "model_tier_mappings""#, vec![]).await?;
  let mut providers: BTreeSet<String> = BTreeSet::new();
  let mut held: BTreeSet<String> = BTreeSet::new();

  for row in &mapped {
    providers.insert(row.try_get("", "provider")?);
    held.insert(row.try_get("", "tier")?);
  }

  for tier in &plan.ladder {
    if held.contains(tier) {
      continue;
    }

    for provider in &providers {
      let Some(mapping) = planned(plan, provider, tier) else { continue };

      run(
        db,
        r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "thinking", "effort", "answer_tokens")
           VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING"#,
        vec![
          mapping.provider.clone().into(),
          mapping.tier.clone().into(),
          mapping.model.clone().into(),
          mapping.thinking.into(),
          text(&mapping.effort),
          mapping.answer_tokens.into(),
        ],
      )
      .await?;
    }
  }

  for row in mapped {
    let tier: String = row.try_get("", "tier")?;

    if plan.ladder.contains(&tier) {
      continue;
    }

    run(db, r#"DELETE FROM "model_tier_mappings" WHERE "tier" = $1"#, vec![tier.into()]).await?;
  }

  let assigned = all(db, r#"SELECT "key", "tier" FROM "model_tier_assignments""#, vec![]).await?;

  for row in assigned {
    let key: String = row.try_get("", "key")?;
    let tier: String = row.try_get("", "tier")?;

    if plan.ladder.contains(&tier) {
      continue;
    }

    match plan.keys.iter().find(|one| one.key == key) {
      Some(start) => {
        run(
          db,
          r#"UPDATE "model_tier_assignments" SET "tier" = $2 WHERE "key" = $1"#,
          vec![key.into(), start.tier.clone().into()],
        )
        .await?;
      }
      None => {
        run(db, r#"DELETE FROM "model_tier_assignments" WHERE "key" = $1"#, vec![key.into()]).await?;
      }
    }
  }

  Ok(())
}

// What a provider's tier starts on: its first-seed mapping, or the mapping a later seed gives it.
fn planned<'a>(plan: &'a TierPlan, provider: &str, tier: &str) -> Option<&'a TierMapping> {
  plan
    .mappings
    .iter()
    .find(|mapping| mapping.provider == provider && mapping.tier == tier)
    .or_else(|| {
      plan
        .later
        .iter()
        .find(|later| later.provider == provider)
        .and_then(|later| later.mappings.iter().find(|mapping| mapping.tier == tier))
    })
}

// Each provider's fold ratio given at every tier of the ladder once, the first time the store is settled
// with the provider, and recorded as given, so a ratio the owner set is never seeded over.
async fn fold_ratios<C: ConnectionTrait>(
  db: &C,
  ladder: &[String],
  seeds: &[FoldRatioSeed],
  now: i64,
) -> Result<(), Fault> {
  for seed in seeds {
    let given = run(
      db,
      r#"INSERT INTO "fold_ratio_seeded" ("provider") VALUES ($1) ON CONFLICT DO NOTHING"#,
      vec![seed.provider.clone().into()],
    )
    .await?;

    if given == 0 {
      continue;
    }

    for tier in ladder {
      run(
        db,
        r#"INSERT INTO "fold_ratios" ("provider", "tier", "ratio", "set_at") VALUES ($1, $2, $3, $4)
           ON CONFLICT DO NOTHING"#,
        vec![seed.provider.clone().into(), tier.clone().into(), seed.ratio.into(), now.into()],
      )
      .await?;
    }
  }

  Ok(())
}

// A provider declared after the first seed gains its tiers' mappings and its co-worker pool's starting
// places once, recorded as given, so a mapping or a place the owner removes stays removed and one
// already standing is never touched.
async fn later_tiers<C: ConnectionTrait>(db: &C, plan: &TierPlan, now: i64) -> Result<(), Fault> {
  for later in &plan.later {
    let given = run(
      db,
      r#"INSERT INTO "model_tier_seeded" ("provider") VALUES ($1) ON CONFLICT DO NOTHING"#,
      vec![later.provider.clone().into()],
    )
    .await?;

    if given == 0 {
      continue;
    }

    for mapping in &later.mappings {
      run(
        db,
        r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "thinking", "effort", "answer_tokens")
           VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING"#,
        vec![
          later.provider.clone().into(),
          mapping.tier.clone().into(),
          mapping.model.clone().into(),
          mapping.thinking.into(),
          text(&mapping.effort),
          mapping.answer_tokens.into(),
        ],
      )
      .await?;
    }

    for tier in &plan.coworker {
      run(
        db,
        r#"INSERT INTO "coworker_pool_tier_limits" ("provider", "tier", "capacity", "set_at")
           VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING"#,
        vec![
          later.provider.clone().into(),
          tier.tier.clone().into(),
          tier.capacity.into(),
          now.into(),
        ],
      )
      .await?;
    }
  }

  Ok(())
}
