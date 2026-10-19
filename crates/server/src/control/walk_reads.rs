//! The providers and the tiers as a consumer walks them (docs/api.md, 「Models, registry, tiers, roads and
//! fallback」): the providers in the provider order with each one's own declaration, the tier ladder, the
//! mapping a work key goes out on at a tier, the choice a pin's value names, and whether a registered
//! model's engine answers.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bmo_chat::{ErrorCode, WireError};
use policy::tiers::TIERS;
use serde::Deserialize;
use serde_json::{Value, json};

use super::harness::{enabled_providers, named, provider_order};
use super::tiers::resolve_execution;
use super::{Call, Refused, Route, answer, registry};
use crate::queue::pins::pin_choice;

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

/// Every provider in the provider order, with whether the owner has it on and what it declares of itself.
pub struct ListProviders;

impl Route for ListProviders {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/providers";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let mut listed: Vec<Value> = vec![];

    for entry in provider_order(call.store()).await? {
      let provider = named(&entry.name)?;
      let declared = provider.declared();

      listed.push(json!({
        "name": entry.name,
        "enabled": entry.enabled,
        "registered": provider.unregistered().is_some(),
        "coworker": declared.lends_coworkers,
        "lines": declared.lines,
      }));
    }

    Ok(answer(StatusCode::OK, json!(listed)))
  }
}

crate::moved!(ListProviders);

/// The tier ladder, in its order, with the tiers a co-worker may be borrowed at.
pub struct ListTiers;

impl Route for ListTiers {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/tiers";

  async fn answer(&self, _call: Call) -> Result<Response, Refused> {
    let listed: Vec<Value> = TIERS
      .ladder
      .iter()
      .map(|tier| json!({ "name": tier, "coworker": TIERS.lends(tier) }))
      .collect();

    Ok(answer(StatusCode::OK, json!(listed)))
  }
}

crate::moved!(ListTiers);

/// A route asked: a work key at a tier, on one provider or none named.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RouteAsk {
  work_key: String,
  provider: Option<String>,
  tier: String,
}

/// The mapping a work key goes out on at a tier: the named provider's, or the first that serves it in the
/// provider order.
pub struct ResolveRoute;

impl Route for ResolveRoute {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/routes/resolve";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let asked: RouteAsk =
      serde_json::from_value(call.body().await?).map_err(|error| bad(format!("a route asked: {error}")))?;

    if !TIERS.holds(&asked.tier) {
      return Err(bad(format!("Unknown model tier: {}", asked.tier)));
    }

    let db = call.store();
    let providers = match asked.provider {
      Some(provider) => vec![provider],
      None => enabled_providers(db).await?,
    };
    let mut reasons = vec![];

    for provider in &providers {
      match resolve_execution(db, &asked.work_key, provider, Some(&asked.tier)).await {
        Ok(mapping) => return Ok(answer(StatusCode::OK, json!(mapping))),
        Err(reason) => reasons.push(reason),
      }
    }

    let reason = if reasons.is_empty() { "no provider is on".to_owned() } else { reasons.join("; ") };

    Ok(
      WireError::new(ErrorCode::NothingHeld, format!("no route serves {}: {reason}", asked.work_key))
        .into_response(),
    )
  }
}

crate::moved!(ResolveRoute);

/// A pin's value, as a worker's pin is written.
#[derive(Deserialize)]
struct PinValue {
  value: String,
}

/// The choice a pin's value names, or why nothing could run on it.
pub struct CheckPin;

impl Route for CheckPin {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/pins/check";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let asked: PinValue =
      serde_json::from_value(call.body().await?).map_err(|error| bad(format!("a pin's value: {error}")))?;
    let pin = pin_choice(call.store(), &asked.value).await?.map_err(bad)?;

    Ok(answer(
      StatusCode::OK,
      match pin.provider {
        None => json!({ "kind": "auto" }),
        Some(provider) => json!({ "kind": "pinned", "provider": provider, "model": pin.model }),
      },
    ))
  }
}

crate::moved!(CheckPin);

/// Whether the engine behind a registered model answers its health and stands aside for nothing.
pub struct ModelHealth;

impl Route for ModelHealth {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/providers/{provider}/models/{model}/health";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let provider = named(call.param("provider").unwrap_or_default())?;
    let model = call.param("model").unwrap_or_default();
    let answers = match provider.unregistered() {
      None => false,
      Some(_) => registry::registration_health(call.store(), &call.data_root(), model)
        .await?
        .is_some_and(|health| health.aside.is_none()),
    };

    Ok(answer(StatusCode::OK, json!({ "answers": answers })))
  }
}

crate::moved!(ModelHealth);
