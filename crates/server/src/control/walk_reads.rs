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

use super::harness::{enabled_providers, named, provider_order, set_provider_order};
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

    Ok(answer(StatusCode::OK, json!({ "items": listed, "more": false })))
  }
}

crate::moved!(ListProviders);

/// One provider, and the owner's switch on it.
pub struct ProviderRoute;

impl Route for ProviderRoute {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/providers/{providerName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("providerName").unwrap_or_default();
    let provider = named(name)?;
    let enabled = provider_order(call.store()).await?.iter().any(|one| one.name == name && one.enabled);

    Ok(answer(
      StatusCode::OK,
      json!({
        "name": name,
        "enabled": enabled,
        "registered": provider.unregistered().is_some(),
        "coworker": provider.declared().lends_coworkers,
      }),
    ))
  }
}

crate::moved!(ProviderRoute);

/// The owner's switch on a provider thrown.
pub struct ChangeProvider;

impl Route for ChangeProvider {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/providers/{providerName}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let name = call.param("providerName").unwrap_or_default().to_owned();
    let asked = call.body().await?;
    let mut order = provider_order(call.store()).await?;

    if let Some(enabled) = asked.get("enabled").and_then(Value::as_bool) {
      for entry in &mut order {
        if entry.name == name {
          entry.enabled = enabled;
        }
      }
      set_provider_order(call.store(), &order).await?;
    }

    let provider = named(&name)?;
    let enabled = provider_order(call.store()).await?.iter().any(|one| one.name == name && one.enabled);

    Ok(answer(
      StatusCode::OK,
      json!({
        "name": name,
        "enabled": enabled,
        "registered": provider.unregistered().is_some(),
        "coworker": provider.declared().lends_coworkers,
      }),
    ))
  }
}

crate::moved!(ChangeProvider);

/// The tier ladder, in its order, with the tiers a co-worker may be borrowed at.
pub struct ListTiers;

impl Route for ListTiers {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/tiers";

  async fn answer(&self, _call: Call) -> Result<Response, Refused> {
    let listed: Vec<Value> = TIERS
      .ladder
      .iter()
      .map(|tier| json!({ "name": tier, "coworker": TIERS.lends(tier), "mapping": {} }))
      .collect();

    Ok(answer(StatusCode::OK, json!({ "items": listed, "more": false })))
  }
}

crate::moved!(ListTiers);

/// Where a work key goes: the named provider's, or the first that serves it in the provider order.
pub struct ResolveRoute;

impl Route for ResolveRoute {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/work-keys/{workKey}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let key = call.param("workKey").unwrap_or_default().to_owned();
    let tier = call.query("tier").unwrap_or_default();
    let asked_provider = call.query("provider").filter(|provider| !provider.trim().is_empty());

    if !tier.is_empty() && !TIERS.holds(&tier) {
      return Err(bad(format!("Unknown model tier: {tier}")));
    }

    let db = call.store();
    let providers = match asked_provider {
      Some(provider) => vec![provider],
      None => enabled_providers(db).await?,
    };
    let mut reasons = vec![];

    for provider in &providers {
      let asked = if tier.is_empty() { None } else { Some(tier.as_str()) };
      match resolve_execution(db, &key, provider, asked).await {
        Ok(mapping) => {
          return Ok(answer(
            StatusCode::OK,
            json!({
              "key": key,
              "provider": mapping.provider,
              "model": mapping.model,
              "effort": mapping.effort,
              "answerTokens": mapping.answer_tokens,
              "capMs": 0,
              "runMode": "run",
            }),
          ));
        }
        Err(reason) => reasons.push(reason),
      }
    }

    let reason = if reasons.is_empty() { "no provider is on".to_owned() } else { reasons.join("; ") };

    Ok(WireError::new(ErrorCode::NotFound, format!("no route serves {key}: {reason}")).into_response())
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
  const PATH: &'static str = "/pin-checks";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let asked: PinValue =
      serde_json::from_value(call.body().await?).map_err(|error| bad(format!("a pin's value: {error}")))?;
    let pin = pin_choice(call.store(), &asked.value).await?.map_err(bad)?;

    Ok(answer(
      StatusCode::OK,
      match pin.provider {
        None => json!({ "kind": "auto", "provider": Value::Null, "model": Value::Null }),
        Some(provider) => json!({
          "kind": if pin.model.is_some() { "model" } else { "provider" },
          "provider": provider,
          "model": pin.model,
        }),
      },
    ))
  }
}

crate::moved!(CheckPin);

/// Whether the engine behind a registered model answers its health and stands aside for nothing.
pub struct ModelHealth;

impl Route for ModelHealth {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/models/{modelKey}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let key = call.param("modelKey").unwrap_or_default();
    let (provider, model) = match call.query("provider") {
      Some(provider) => (provider, key.to_owned()),
      None => {
        let (provider, model) = key.split_once('/').unwrap_or((key, key));
        (provider.to_owned(), model.to_owned())
      }
    };
    let provider = named(&provider)?;
    let answers = match provider.unregistered() {
      None => false,
      Some(_) => registry::registration_health(call.store(), &call.data_root(), &model)
        .await?
        .is_some_and(|health| health.aside.is_none()),
    };

    Ok(answer(
      StatusCode::OK,
      json!({ "key": key, "provider": provider.name(), "model": model, "tier": Value::Null, "healthy": answers }),
    ))
  }
}

crate::moved!(ModelHealth);
