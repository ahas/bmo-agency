//! The tier settings a caller edits (docs/model-tiers.md): every
//! provider's mapping of every tier, every work key's tier with what it resolves to on each provider,
//! and each provider's catalogue a new mapping is checked against.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use harness_providers::{ARCHIVE_PROVIDERS, Machine, Offered};
use policy::answer_ceilings::ANSWER_CEILINGS;
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::tiers::TIERS;
use policy::work_keys::WORK_KEYS;
use sea_orm::DatabaseConnection;
use serde_json::{Map, Value, json};

use super::claude_accounts::Judging;
use super::harness::{named, offered, offered_reason, provider_enabled};
use super::tiers::{
  Mapping, assign, assignable, assignments, conflicts, mappings, resolve_execution, set_mapping,
};
use super::{Call, Refused, Route, answer};

/// The line a work key's run goes out on, which is what decides whether a provider carries it.
pub fn line_of(key: &str) -> &str {
  match WORK_KEYS.profile(key) {
    Some(profile) => &profile.line,
    None if key == "browse" => "browse",
    None if key.starts_with("search") => "search",
    None => "worker",
  }
}

/// The efforts a list of words holds as JSON carries them, no effort as null.
fn efforts_json(efforts: &[Option<String>]) -> Value {
  Value::Array(efforts.iter().map(|effort| effort.clone().map_or(Value::Null, Value::String)).collect())
}

/// What a provider offers a tier mapping, one entry to a model, each carrying the facts only its
/// provider's catalogue tells.
pub fn catalogue_entries(provider: &str, offered: &Offered) -> Vec<Value> {
  offered
    .models
    .iter()
    .map(|model| {
      let mut entry = Map::new();

      entry.insert("provider".into(), json!(provider));
      entry.insert("model".into(), json!(model.id));
      entry.insert("efforts".into(), efforts_json(&model.efforts));

      if let Some(free) = model.free {
        entry.insert("free".into(), json!(free));
      }

      if let Some(in_service) = model.in_service {
        entry.insert("inService".into(), json!(in_service));
      }

      if let Some(read_at) = offered.stale_read_at {
        entry.insert("staleReadAt".into(), harness_providers::number(read_at));
      }

      Value::Object(entry)
    })
    .collect()
}

/// A provider's catalogue as tier entries, or the refusal its listing gave.
pub async fn tier_catalogue(
  judging: Judging<'_>,
  machine: &Machine,
  provider: &str,
) -> Result<Vec<Value>, String> {
  let asked = named(provider).map_err(|refused| refused.error)?;

  Ok(catalogue_entries(provider, &offered(judging, machine, asked).await?))
}

async fn effective(
  db: &DatabaseConnection,
  machine: &Machine,
  held: &[Mapping],
  key: &str,
  tier: &str,
) -> Result<Vec<Value>, Refused> {
  let mut views = Vec::new();

  for provider in ARCHIVE_PROVIDERS {
    views.push(match resolve_execution(db, key, provider, None).await {
      Ok(selection) => json!({
        "provider": provider,
        "selection": selection,
        "error": offered_reason(db, machine, named(provider)?, line_of(key)).await?,
      }),
      Err(error) => json!({
        "provider": provider,
        "selection": held.iter().find(|one| one.provider == provider && one.tier == tier),
        "error": error,
      }),
    });
  }

  Ok(views)
}

/// Every provider's mapping of every tier, and every work key's tier with what it resolves to.
pub struct TierSettings;

impl Route for TierSettings {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/model-tiers";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let db = call.store();
    let machine = call.machine();
    let held = mappings(db).await?;
    let mut providers = Vec::new();

    for provider in ARCHIVE_PROVIDERS {
      let declared = named(provider)?.declared();
      let efforts: Vec<Option<String>> =
        declared.effort.vocabulary().into_iter().map(|word| word.map(str::to_owned)).collect();

      providers.push(json!({
        "provider": provider,
        "label": declared.label,
        "capsAnswerTokens": declared.caps_answer_tokens,
        "efforts": efforts_json(&efforts),
        "enabled": provider_enabled(db, provider).await?,
      }));
    }

    let mut rows = Vec::new();

    for row in assignments(db).await? {
      rows.push(json!({
        "key": row.key,
        "tier": row.tier,
        "choices": assignable(db, &row.key).await?,
        "effective": effective(db, machine, &held, &row.key, &row.tier).await?,
      }));
    }

    Ok(answer(
      StatusCode::OK,
      json!({
        "tiers": TIERS.ladder,
        "providers": providers,
        "mappings": held,
        "conflicts": conflicts(db).await?,
        "assignments": rows,
      }),
    ))
  }
}

/// One provider's catalogue, for a new mapping to be chosen from.
pub struct TierModels;

impl Route for TierModels {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/model-tiers/models";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let Some(provider) = call.query("provider").filter(|name| ARCHIVE_PROVIDERS.contains(&name.as_str()))
    else {
      return Err(Refused::new(StatusCode::BAD_REQUEST, CONTROL_WORDS.tiers.provider.as_str()));
    };

    match tier_catalogue(Judging::of(call.ground()), call.machine(), &provider).await {
      Ok(entries) => Ok(answer(StatusCode::OK, Value::Array(entries))),
      Err(error) => Err(Refused::new(StatusCode::BAD_GATEWAY, listing_refusal(&provider, &error))),
    }
  }
}

/// What a listing that did not arrive is said as.
pub fn listing_refusal(provider: &str, error: &str) -> String {
  fill_marks(&CONTROL_WORDS.fallback.models_unread, &[("provider", provider), ("why", error)])
}

async fn change(judging: Judging<'_>, machine: &Machine, body: &Value) -> Result<(), String> {
  let db = judging.db;
  let field = |name: &str| body.get(name).unwrap_or(&Value::Null);
  let Some(tier) = field("tier").as_str().filter(|tier| TIERS.holds(tier)) else {
    return Err(CONTROL_WORDS.tiers.tier.clone());
  };

  match field("action").as_str() {
    Some("assign") => {
      let held = assignments(db).await.map_err(|refused| refused.error)?;
      let Some(key) = field("key").as_str().filter(|key| held.iter().any(|one| one.key == *key)) else {
        return Err(CONTROL_WORDS.tiers.key.clone());
      };

      if !assignable(db, key)
        .await
        .map_err(|refused| refused.error)?
        .iter()
        .any(|one| one == tier)
      {
        return Err(fill_marks(
          &CONTROL_WORDS.tiers.coworker,
          &[("tiers", &TIERS.coworker_tiers().join(", "))],
        ));
      }

      let unsettled = conflicts(db)
        .await
        .map_err(|refused| refused.error)?
        .iter()
        .any(|one| one.key == key && !one.accepted);

      if unsettled && field("acceptConflicts") != &Value::Bool(true) {
        return Err(CONTROL_WORDS.tiers.conflicts.clone());
      }

      assign(db, key, tier).await
    }
    Some("set-mapping") => {
      let effort = match field("effort") {
        Value::Null if body.get("effort").is_some() => Some(None),
        Value::String(word) => Some(Some(word.clone())),
        _ => None,
      };
      let (Some(provider), Some(model), Some(effort)) = (
        field("provider").as_str().filter(|name| ARCHIVE_PROVIDERS.contains(name)),
        field("model").as_str(),
        effort,
      ) else {
        return Err(CONTROL_WORDS.tiers.mapping.clone());
      };
      let declared = named(provider).map_err(|refused| refused.error)?.declared();
      // Only a provider that caps the answer's tokens carries a ceiling; left out, it is the work's own
      // default.
      let answer_tokens = match field("answerTokens") {
        Value::Null => None,
        sent => {
          let ceiling = super::whole(sent).filter(|ceiling| ANSWER_CEILINGS.holds(*ceiling));

          if !declared.caps_answer_tokens || ceiling.is_none() {
            let mut capping = Vec::new();

            for one in ARCHIVE_PROVIDERS {
              let declared = named(one).map_err(|refused| refused.error)?.declared();

              if declared.caps_answer_tokens {
                capping.push(declared.label.clone());
              }
            }

            let choices: Vec<String> = ANSWER_CEILINGS.choices.iter().map(i64::to_string).collect();

            return Err(fill_marks(
              &CONTROL_WORDS.tiers.answer_tokens,
              &[("providers", &capping.join(", ")), ("choices", &choices.join(", "))],
            ));
          }

          ceiling
        }
      };
      let offered = offered(judging, machine, named(provider).map_err(|refused| refused.error)?).await?;
      let capability = offered.models.iter().find(|one| one.id == model).map(|one| one.efforts.as_slice());
      let mapping = Mapping {
        provider: provider.into(),
        tier: tier.into(),
        model: model.into(),
        effort,
        answer_tokens,
      };

      set_mapping(db, &mapping, capability).await
    }
    _ => Err(CONTROL_WORDS.request.action.clone()),
  }
}

/// A tier mapping or a work key's assignment changed, checked against the catalogue at save time.
pub struct TierChange;

impl Route for TierChange {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/tiers/{tierName}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let mut body = call.body().await?;
    let tier = call.param("tierName").unwrap_or_default();
    if body.get("tier").is_none() {
      body["tier"] = json!(tier);
    }

    change(Judging::of(call.ground()), call.machine(), &body)
      .await
      .map_err(Refused::failed)?;

    Ok(answer(
      StatusCode::OK,
      json!({
        "name": tier,
        "coworker": policy::tiers::TIERS.lends(tier),
        "mapping": body,
      }),
    ))
  }
}

crate::moved!(TierChange);
