//! The providers and the tiers as a consumer walks them (docs/api.md, 「Models, registry, tiers, roads and
//! fallback」): the providers in the provider order with each one's own declaration, the tier ladder, the
//! mapping a work key goes out on at a tier, the choice a pin's value names, and whether a registered
//! model's engine answers.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bmo_chat::{ErrorCode, WireError};
use harness_providers::ARCHIVE_PROVIDERS;
use policy::tiers::TIERS;
use serde::Deserialize;
use serde_json::{Value, json};

use super::claude_accounts::Judging;
use super::harness::{named, provider_order, set_provider_order};
use super::tiers::resolve_execution;
use super::{Call, Refused, Route, answer, registry, roads};
use crate::queue::pins::pin_choice;
use crate::queue::routes::{Unrouted, route_blocked};

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
        "code": entry.name,
        "label": declared.label,
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
        "code": name,
        "label": provider.declared().label,
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

    if !ARCHIVE_PROVIDERS.contains(&name.as_str()) {
      return Err(Refused::new(StatusCode::NOT_FOUND, format!("no provider named {name} in this build")));
    }

    let mut order = provider_order(call.store()).await?;

    if let Some(enabled) = asked.get("enabled").and_then(Value::as_bool) {
      let others_on = order.iter().any(|entry| entry.name != name && entry.enabled);

      if !enabled && !others_on {
        return Err(Refused::new(
          StatusCode::CONFLICT,
          format!("{name} is the last provider on, and no provider left on would answer anything"),
        ));
      }

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
        "code": name,
        "label": provider.declared().label,
        "enabled": enabled,
        "registered": provider.unregistered().is_some(),
        "coworker": provider.declared().lends_coworkers,
      }),
    ))
  }
}

crate::moved!(ChangeProvider);

/// The tier ladder, in its order, with its mapping on every provider and the tiers a co-worker may be
/// borrowed at.
pub struct ListTiers;

impl Route for ListTiers {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/tiers";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let held = super::tiers::mappings(call.store()).await?;
    let listed: Vec<Value> = TIERS
      .ladder
      .iter()
      .map(|tier| {
        let mut mapping = serde_json::Map::new();

        for one in held.iter().filter(|one| &one.tier == tier) {
          mapping.insert(
            one.provider.clone(),
            json!({
              "model": one.model,
              "thinking": one.thinking,
              "effort": one.effort,
              "answerTokens": one.answer_tokens,
            }),
          );
        }

        json!({ "name": tier, "coworker": TIERS.lends(tier), "mapping": mapping })
      })
      .collect();

    Ok(answer(StatusCode::OK, json!({ "items": listed, "more": false })))
  }
}

crate::moved!(ListTiers);

/// Where a work key goes, and the road it stands on: the named provider's, or the first that serves it of the
/// provider the key is set to and then the providers the walk asks, each carrying the key's line with a
/// healthy model.
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
    let mode = roads::selected_mode(db, &key).await?;
    let pinned = roads::pinned_provider(db, &key).await?;
    let line = roads::profile(&key).map_or("worker", |profile| profile.line.as_str());
    let walking = asked_provider.is_none();
    let mut reasons = vec![];
    // The walk is the road resolution's (docs/cache-fallback.md, 「One road resolution」): its providers
    // that can answer now, in its order, each closed one's reason kept for a read nobody serves.
    let providers = match asked_provider {
      Some(provider) => vec![provider],
      None => {
        let resolution = crate::road::resolve(call.ground(), &crate::road::Ask::key(&key))
          .await
          .map_err(Refused::failed)?;

        reasons.extend(
          resolution
            .closed()
            .iter()
            .map(|closed| format!("{}: {}", closed.provider, closed.reason)),
        );
        resolution.open().map(|road| road.provider.clone()).collect()
      }
    };
    let judging = Judging::of(call.ground());
    let data_root = call.data_root();

    for provider in &providers {
      // The walk asks a provider the key is not set to only where its program carries the key's line.
      let walked = walking && pinned.as_ref() != Some(provider);

      if walked && !roads::walked(db, named(provider)?, &key, line).await? {
        reasons.push(format!("{provider}: not walked on the {line} line"));
        continue;
      }

      let asked = if tier.is_empty() { None } else { Some(tier.as_str()) };
      let mapping = match resolve_execution(db, &key, provider, asked).await {
        Ok(mapping) => mapping,
        Err(reason) => {
          reasons.push(reason);
          continue;
        }
      };

      if walked && let Some(reason) = unhealthy(judging, &data_root, provider, &mapping.model).await? {
        reasons.push(reason);
        continue;
      }

      return Ok(answer(
        StatusCode::OK,
        json!({
          "key": key,
          "provider": mapping.provider,
          "model": mapping.model,
          "thinking": mapping.thinking,
          "effort": mapping.effort,
          "answerTokens": mapping.answer_tokens,
          "capMs": 0,
          "runMode": mode,
        }),
      ));
    }

    let reason = if reasons.is_empty() { "no provider is on".to_owned() } else { reasons.join("; ") };

    Ok(WireError::new(ErrorCode::NotFound, format!("no route serves {key}: {reason}")).into_response())
  }
}

// Why a provider's model is not healthy now, or nothing where it is: its road shut by a failure still standing
// or by the deployment's rest, or, for a registration, an engine that says it stands aside.
async fn unhealthy(
  judging: Judging<'_>,
  data_root: &std::path::Path,
  provider: &str,
  model: &str,
) -> Result<Option<String>, Refused> {
  if route_blocked(judging, &format!("{provider}::{model}"), crate::now_ms())
    .await
    .map_err(|Unrouted(why)| Refused::failed(why))?
  {
    return Ok(Some(format!("{provider}: the road to {model} is shut now")));
  }

  if named(provider)?.unregistered().is_some()
    && registry::registration_health(judging.db, data_root, model)
      .await?
      .is_some_and(|health| health.aside.is_some())
  {
    return Ok(Some(format!("{provider}: the engine behind {model} stands aside")));
  }

  Ok(None)
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

/// A model of a provider, by the name the path and the query give it: the provider and the model apart.
pub fn model_key(key: &str, provider: Option<&str>) -> (String, String) {
  if let Some(provider) = provider.filter(|provider| !provider.trim().is_empty()) {
    return (provider.to_owned(), key.to_owned());
  }

  key.split_once("::").or_else(|| key.split_once('/')).map_or_else(
    || (registry::PROVIDER.to_owned(), key.to_owned()),
    |(provider, model)| (provider.to_owned(), model.to_owned()),
  )
}

/// One model and its health: a registration of the registry's provider, or a model of a provider whose
/// catalogue is its own.
pub struct ModelHealth;

impl Route for ModelHealth {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/models/{modelKey}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let key = call.param("modelKey").unwrap_or_default();
    let (provider, model) = model_key(key, call.query("provider").as_deref());
    let asked = named(&provider).map_err(|_| {
      Refused::new(StatusCode::NOT_FOUND, format!("no provider named {provider} in this build"))
    })?;

    if asked.unregistered().is_some() {
      return match registry::registered_model(call.store(), &call.data_root(), &model).await? {
        Some(view) => Ok(answer(StatusCode::OK, view)),
        None => Err(Refused::new(StatusCode::NOT_FOUND, format!("no model is registered as {model}"))),
      };
    }

    Ok(answer(
      StatusCode::OK,
      json!({
        "key": key,
        "provider": asked.name(),
        "model": model,
        "tier": Value::Null,
        "healthy": false,
      }),
    ))
  }
}

crate::moved!(ModelHealth);
