//! The tier store as the routes read and write it (docs/model-tiers.md): a tier names a level of
//! work, each provider maps every tier to one of its own models, and every work key is assigned a
//! tier. Which tiers there are is the policy's ladder; the mappings and assignments are rows.

use harness_providers::ARCHIVE_PROVIDERS;
use policy::answer_ceilings::ANSWER_CEILINGS;
use policy::queue_words::QUEUE_WORDS;
use policy::tiers::TIERS;
use regex::Regex;
use sea_orm::{DatabaseConnection, QueryResult, TransactionTrait};
use serde::Serialize;
use serde_json::Value;

use super::Refused;
use super::harness::{named, provider_enabled};
use store::{all, one, run};

/// One provider's mapping of one tier.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mapping {
  pub provider: String,
  pub tier: String,
  pub model: String,
  pub thinking: Option<bool>,
  pub effort: Option<String>,
  pub answer_tokens: Option<i64>,
}

/// One work key's tier.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Assignment {
  pub key: String,
  pub tier: String,
}

/// An owner's choice from before the tiers, kept for review until its key's tier is accepted.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Conflict {
  pub key: String,
  pub provider: String,
  pub previous: Value,
  pub accepted: bool,
}

fn mapping_of(row: &QueryResult) -> Result<Mapping, Refused> {
  Ok(Mapping {
    provider: row.try_get("", "provider")?,
    tier: row.try_get("", "tier")?,
    model: row.try_get("", "model")?,
    thinking: row.try_get("", "thinking")?,
    effort: row.try_get("", "effort")?,
    answer_tokens: row.try_get("", "answer_tokens")?,
  })
}

pub async fn mappings(db: &DatabaseConnection) -> Result<Vec<Mapping>, Refused> {
  all(
    db,
    "SELECT provider, tier, model, thinking, effort, answer_tokens FROM model_tier_mappings ORDER BY provider, tier",
    vec![],
  )
  .await?
  .iter()
  .map(mapping_of)
  .collect()
}

pub async fn assignments(db: &DatabaseConnection) -> Result<Vec<Assignment>, Refused> {
  all(db, "SELECT key, tier FROM model_tier_assignments ORDER BY key", vec![])
    .await?
    .iter()
    .map(|row| {
      Ok(Assignment {
        key: row.try_get("", "key")?,
        tier: row.try_get("", "tier")?,
      })
    })
    .collect()
}

pub async fn conflicts(db: &DatabaseConnection) -> Result<Vec<Conflict>, Refused> {
  all(
    db,
    "SELECT key, provider, previous, accepted FROM model_tier_conflicts ORDER BY key, provider",
    vec![],
  )
  .await?
  .iter()
  .map(|row| {
    let previous: String = row.try_get("", "previous")?;

    Ok(Conflict {
      key: row.try_get("", "key")?,
      provider: row.try_get("", "provider")?,
      previous: serde_json::from_str(&previous).map_err(Refused::failed)?,
      accepted: row.try_get::<i64>("", "accepted")? == 1,
    })
  })
  .collect()
}

async fn unsettled(db: &DatabaseConnection, key: &str, provider: &str) -> Result<bool, Refused> {
  Ok(
    one(
      db,
      "SELECT 1 AS held FROM model_tier_conflicts WHERE key = $1 AND provider = $2 AND accepted = 0",
      vec![key.into(), provider.into()],
    )
    .await?
    .is_some(),
  )
}

/// What a mapping has to be whoever its provider is: a tier of the ladder, a provider this build
/// carries, a model identifier of the shape a command line takes, and a ceiling of the offered sizes.
pub fn check_mapping(mapping: &Mapping) -> Result<(), String> {
  if !TIERS.holds(&mapping.tier) {
    return Err(format!("Unknown model tier: {}", mapping.tier));
  }

  if !ARCHIVE_PROVIDERS.contains(&mapping.provider.as_str()) {
    return Err("Unknown tier provider".into());
  }

  let shape = Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9._:/-]{0,200}$").expect("the model pattern compiles");

  if !shape.is_match(&mapping.model) {
    return Err("Invalid model identifier".into());
  }

  if mapping.answer_tokens.is_some_and(|ceiling| !ANSWER_CEILINGS.holds(ceiling)) {
    return Err("Unsupported answer token ceiling".into());
  }

  Ok(())
}

/// What a tier maps to on a provider, or the refusal.
pub async fn resolve_tier(db: &DatabaseConnection, tier: &str, provider: &str) -> Result<Mapping, String> {
  if !TIERS.holds(tier) {
    return Err(format!("Unknown model tier: {tier}"));
  }

  let row = one(
    db,
    "SELECT provider, tier, model, thinking, effort, answer_tokens FROM model_tier_mappings WHERE provider = $1 AND tier = $2",
    vec![provider.into(), tier.into()],
  )
  .await
  .map_err(|refusal| refusal.detail)?
  .ok_or_else(|| format!("Missing tier mapping: {provider}/{tier}"))?;
  let mapping = mapping_of(&row).map_err(|refused| refused.error)?;

  check_mapping(&mapping)?;

  Ok(mapping)
}

/// What a work key resolves to on a provider, or the refusal: its assignment, its migration settled, and on
/// the registry's provider the registration the key names of its own in place of its tier's cell, on that
/// registration's own answer size and thinking unless the cell maps to it (docs/own-llm.md, 「A key's own
/// registration」).
pub async fn resolve(db: &DatabaseConnection, key: &str, provider: &str) -> Result<Mapping, String> {
  let mapping = resolve_assigned(db, key, provider).await?;

  key_pinned(db, key, provider, mapping).await
}

// A key's mapping on a provider with the registration the key names of its own laid over it on the
// registry's provider, whichever tier the mapping is: on the registration's own answer size and thinking
// unless the mapping already names it. A registration that has left the registry, or holds no address, is
// refused rather than given back to the tier.
async fn key_pinned(
  db: &DatabaseConnection,
  key: &str,
  provider: &str,
  mapping: Mapping,
) -> Result<Mapping, String> {
  if provider != super::registry::PROVIDER {
    return Ok(mapping);
  }

  let pinned = super::roads::key_registration(db, key).await.map_err(|refused| refused.error)?;
  let Some(registration) = pinned else { return Ok(mapping) };

  if let Some(refusal) = super::roads::registration_refusal(db, &registration)
    .await
    .map_err(|refused| refused.error)?
  {
    return Err(refusal);
  }

  Ok(if registration == mapping.model {
    mapping
  } else {
    Mapping {
      model: registration,
      thinking: None,
      effort: None,
      answer_tokens: None,
      ..mapping
    }
  })
}

// What a work key's tier maps to on a provider.
async fn resolve_assigned(db: &DatabaseConnection, key: &str, provider: &str) -> Result<Mapping, String> {
  let tier = one(db, "SELECT tier FROM model_tier_assignments WHERE key = $1", vec![key.into()])
    .await
    .map_err(|refusal| refusal.detail)?
    .map(|row| row.try_get::<String>("", "tier"))
    .transpose()
    .map_err(|error| error.to_string())?
    .ok_or_else(|| format!("Missing tier assignment: {key}"))?;

  if unsettled(db, key, provider).await.map_err(|refused| refused.error)? {
    return Err(format!("Unresolved tier migration: {key}/{provider}"));
  }

  resolve_tier(db, &tier, provider).await
}

/// What one attempt of a work key goes out on with one provider: refused where the provider is off,
/// the key's migration unsettled, its tier unresolved, or the provider would not launch the mapping's
/// effort (docs/model-tiers.md).
pub async fn resolve_execution(
  db: &DatabaseConnection,
  key: &str,
  provider: &str,
  requested_tier: Option<&str>,
) -> Result<Mapping, String> {
  if !provider_enabled(db, provider).await.map_err(|refused| refused.error)? {
    return Err(format!("{provider}: {}", QUEUE_WORDS.pool.off));
  }

  if unsettled(db, key, provider).await.map_err(|refused| refused.error)? {
    return Err(format!("Unresolved tier migration: {key}/{provider}"));
  }

  let mapping = match requested_tier {
    Some(tier) => key_pinned(db, key, provider, resolve_tier(db, tier, provider).await?).await?,
    None => resolve(db, key, provider).await?,
  };
  let declared = named(provider).map_err(|refused| refused.error)?.declared();

  match declared.effort.fault(mapping.effort.as_deref()) {
    Some(fault) => Err(fault.to_owned()),
    None => Ok(mapping),
  }
}

/// The tiers a work key may be assigned: a role the pool lends out takes only a co-worker tier; every
/// other key, the builder's own turn included, takes any.
pub async fn assignable(db: &DatabaseConnection, key: &str) -> Result<Vec<String>, Refused> {
  let pooled = match key.strip_prefix("role:") {
    Some(role) => one(db, "SELECT pooled FROM coworker_pool_roles WHERE name = $1", vec![role.into()])
      .await?
      .map(|row| row.try_get::<i64>("", "pooled"))
      .transpose()?
      .is_some_and(|pooled| pooled == 1),
    None => false,
  };

  Ok(if pooled {
    TIERS.coworker_tiers().into_iter().map(str::to_owned).collect()
  } else {
    TIERS.ladder.clone()
  })
}

/// A mapping written, where the catalogue's evidence names its model and its effort.
pub async fn set_mapping(
  db: &DatabaseConnection,
  mapping: &Mapping,
  efforts: Option<&[Option<String>]>,
) -> Result<(), String> {
  check_mapping(mapping)?;

  if !efforts.is_some_and(|efforts| efforts.contains(&mapping.effort)) {
    return Err("Model capability evidence required for this model and effort".into());
  }

  run(
    db,
    r#"INSERT INTO model_tier_mappings (provider, tier, model, thinking, effort, answer_tokens) VALUES ($1, $2, $3, $4, $5, $6)
       ON CONFLICT (provider, tier) DO UPDATE SET model = excluded.model, thinking = excluded.thinking, effort = excluded.effort, answer_tokens = excluded.answer_tokens"#,
    vec![
      mapping.provider.clone().into(),
      mapping.tier.clone().into(),
      mapping.model.clone().into(),
      mapping.thinking.into(),
      mapping.effort.clone().into(),
      mapping.answer_tokens.into(),
    ],
  )
  .await
  .map_err(|refusal| refusal.detail)?;

  Ok(())
}

/// A work key assigned a tier, every migration conflict of the key accepted with it.
pub async fn assign(db: &DatabaseConnection, key: &str, tier: &str) -> Result<(), String> {
  let shape = Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9:._-]{0,200}$").expect("the key pattern compiles");

  if !shape.is_match(key) {
    return Err("Invalid work key".into());
  }

  if !TIERS.holds(tier) {
    return Err(format!("Unknown model tier: {tier}"));
  }

  let written = db.begin().await.map_err(|error| error.to_string())?;

  run(
    &written,
    "INSERT INTO model_tier_assignments (key, tier) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET tier = excluded.tier",
    vec![key.into(), tier.into()],
  )
  .await
  .map_err(|refusal| refusal.detail)?;
  run(
    &written,
    "UPDATE model_tier_conflicts SET accepted = 1 WHERE key = $1",
    vec![key.into()],
  )
  .await
  .map_err(|refusal| refusal.detail)?;
  written.commit().await.map_err(|error| error.to_string())
}
