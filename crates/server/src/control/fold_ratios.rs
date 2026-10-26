//! The fold ratio of each provider at each tier (docs/sessions.md, 「Session compaction」; docs/api.md,
//! 「fold-ratios」): the share of a model's window a session may fill before it is folded once the turn
//! that crossed it is over. The table holds a row a provider and tier; a missing row reads the provider's
//! own declared share, and a provider declaring none the general share the resident-turn policy names.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use policy::resident_turn::RESIDENT_TURN;
use policy::tiers::TIERS;
use sea_orm::ConnectionTrait;
use serde_json::{Value, json};
use store::{all, one, run};

use super::{Call, Refused, Route, answer};
use crate::now_ms;
use crate::queue::routes::JOB_WORK_KEY;

/// Where a ratio comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
  /// The table's row.
  Set,
  /// The provider's own declared share, the table holding no row.
  Provider,
  /// The general share, neither the table nor the provider naming one.
  Default,
}

impl Source {
  pub fn word(self) -> &'static str {
    match self {
      Self::Set => "set",
      Self::Provider => "provider",
      Self::Default => "default",
    }
  }
}

/// A provider's ratio at a tier, and where it comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct FoldRatio {
  pub provider: String,
  pub tier: String,
  pub ratio: f64,
  pub source: Source,
}

impl FoldRatio {
  fn member(&self) -> Value {
    json!({
      "provider": self.provider,
      "tier": self.tier,
      "ratio": self.ratio,
      "source": self.source.word(),
    })
  }
}

/// The share that stands for a provider where the table holds no row: its own declared share, else the
/// general one.
pub fn fallback(provider: &str) -> (f64, Source) {
  match harness_providers::provider(provider).and_then(|one| one.declared().fold_ratio) {
    Some(ratio) => (ratio, Source::Provider),
    None => (RESIDENT_TURN.fold_ratio, Source::Default),
  }
}

/// The share a session is folded at: the ratio, or less where its provider bounds a session in tokens and
/// the window its last turn read is known, so the bound is reached first.
pub fn bounded(ratio: f64, provider: &str, window: Option<u64>) -> f64 {
  let tokens = harness_providers::provider(provider).and_then(|one| one.declared().fold_tokens);

  match (tokens, window.filter(|window| *window > 0)) {
    (Some(tokens), Some(window)) => ratio.min(tokens as f64 / window as f64),
    _ => ratio,
  }
}

/// Whether a share may be kept: more than nothing and at most the whole window.
pub fn admissible(ratio: f64) -> bool {
  ratio > 0.0 && ratio <= 1.0
}

/// The ratio of a provider at a tier: the table's row, else the provider's fallback, which a session at no
/// tier reads as well.
pub async fn ratio<C: ConnectionTrait>(db: &C, provider: &str, tier: Option<&str>) -> Result<f64, Refused> {
  let Some(tier) = tier else { return Ok(fallback(provider).0) };
  let row = one(
    db,
    r#"SELECT "ratio" FROM "fold_ratios" WHERE "provider" = $1 AND "tier" = $2"#,
    vec![provider.into(), tier.into()],
  )
  .await?;

  match row {
    Some(row) => Ok(row.try_get::<f64>("", "ratio")?),
    None => Ok(fallback(provider).0),
  }
}

/// The tier an agent's sessions stand at: the tier its work key is assigned, a worker's the job key's, and
/// none where the key is assigned none.
pub async fn agent_tier<C: ConnectionTrait>(db: &C, agent: &str) -> Result<Option<String>, Refused> {
  let kind = one(db, r#"SELECT "kind" FROM "agents" WHERE "name" = $1"#, vec![agent.into()])
    .await?
    .map(|row| row.try_get::<String>("", "kind"))
    .transpose()?;
  let key = if kind.as_deref() == Some("worker") { JOB_WORK_KEY } else { agent };
  let tier = one(db, r#"SELECT "tier" FROM "model_tier_assignments" WHERE "key" = $1"#, vec![key.into()])
    .await?
    .map(|row| row.try_get::<String>("", "tier"))
    .transpose()?;

  Ok(tier)
}

/// The ratio a session of an agent is folded at on the provider it stands on, read at this moment.
pub async fn of_agent<C: ConnectionTrait>(db: &C, provider: &str, agent: &str) -> Result<f64, Refused> {
  let tier = agent_tier(db, agent).await?;

  ratio(db, provider, tier.as_deref()).await
}

/// Every provider at every tier of the ladder, in the providers' order and the ladder's.
pub async fn grid<C: ConnectionTrait>(db: &C) -> Result<Vec<FoldRatio>, Refused> {
  let rows = all(db, r#"SELECT "provider", "tier", "ratio" FROM "fold_ratios""#, vec![]).await?;
  let mut set = Vec::with_capacity(rows.len());

  for row in rows {
    set.push((
      row.try_get::<String>("", "provider")?,
      row.try_get::<String>("", "tier")?,
      row.try_get::<f64>("", "ratio")?,
    ));
  }

  let mut grid = vec![];

  for provider in harness_providers::providers() {
    let provider = provider.name();

    for tier in &TIERS.ladder {
      let kept = set.iter().find(|(one, at, _)| one == provider && at == tier);
      let (ratio, source) = match kept {
        Some((_, _, ratio)) => (*ratio, Source::Set),
        None => fallback(provider),
      };

      grid.push(FoldRatio {
        provider: provider.to_owned(),
        tier: tier.clone(),
        ratio,
        source,
      });
    }
  }

  Ok(grid)
}

/// Every provider's fold ratio at every tier.
pub struct FoldRatios;

impl Route for FoldRatios {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/fold-ratios";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let items: Vec<Value> = grid(call.store()).await?.iter().map(FoldRatio::member).collect();

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": false })))
  }
}

crate::moved!(FoldRatios);

/// A provider's fold ratio at a tier set.
pub struct FoldRatioSet;

impl Route for FoldRatioSet {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/fold-ratios/{provider}/{tier}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let provider = call.param("provider").unwrap_or_default().to_owned();
    let tier = call.param("tier").unwrap_or_default().to_owned();

    if harness_providers::provider(&provider).is_none() {
      return Err(Refused::new(StatusCode::NOT_FOUND, format!("no provider is named {provider}")));
    }

    if !TIERS.holds(&tier) {
      return Err(Refused::new(StatusCode::NOT_FOUND, format!("no tier is named {tier}")));
    }

    let body = call.body().await?;
    let Some(ratio) = body.get("ratio").and_then(Value::as_f64).filter(|ratio| admissible(*ratio)) else {
      return Err(Refused::new(
        StatusCode::BAD_REQUEST,
        "ratio is a number above 0 and at most 1: the share of the window a session may fill",
      ));
    };

    run(
      call.store(),
      r#"INSERT INTO "fold_ratios" ("provider", "tier", "ratio", "set_at") VALUES ($1, $2, $3, $4)
         ON CONFLICT ("provider", "tier") DO UPDATE SET "ratio" = excluded."ratio", "set_at" = excluded."set_at""#,
      vec![provider.clone().into(), tier.clone().into(), ratio.into(), now_ms().into()],
    )
    .await?;

    let set = FoldRatio {
      provider,
      tier,
      ratio,
      source: Source::Set,
    };

    Ok(answer(StatusCode::OK, set.member()))
  }
}

crate::moved!(FoldRatioSet);
