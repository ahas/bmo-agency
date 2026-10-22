//! OpenRouter: present where its key is in the environment, and its catalogue the public listing,
//! which needs no key (docs/provider-presence.md).

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use policy::providers::Declared;
use policy::providers::openrouter::OPENROUTER;
use serde::Serialize;
use serde_json::{Value, json};

use crate::{Machine, Offered, OfferedModel, Offering, PlaceCache, Provider, Reading};

pub struct OpenRouter;

impl Provider for OpenRouter {
  fn name(&self) -> &'static str {
    "openrouter"
  }

  fn declared(&self) -> &'static Declared {
    &OPENROUTER.declared
  }

  // A routed model's cache belongs to the upstream that took the turn, so a place is warmed only on that
  // upstream's published lifetime.
  fn place_cache(&self, model: &str, upstream: Option<&str>) -> Result<PlaceCache, String> {
    let Some(upstream) = upstream else {
      return Err("No OpenRouter upstream was recorded as taking that prefix.".to_owned());
    };

    OPENROUTER
      .cache
      .of(upstream, model)
      .map(|entry| PlaceCache {
        ttl_minutes: entry.ttl_minutes,
        idle_minutes: entry.keepalive_idle_minutes,
      })
      .ok_or_else(|| format!("{upstream} publishes no cache lifetime for {model}."))
  }

  // Its program is bmo-tui standing this provider's dsh session.
  fn owns_program(&self, _machine: &Machine, args: &[String], _env: &BTreeMap<String, String>) -> bool {
    let Some(program) =
      OPENROUTER.declared.resident.as_ref().and_then(|resident| resident.program.as_deref())
    else {
      return false;
    };
    let named = args.iter().position(|arg| arg == "--model").map(|at| at + 1);
    let choice = named
      .and_then(|at| args.get(at))
      .and_then(|choice| serde_json::from_str::<Vec<String>>(choice).ok());

    args
      .first()
      .and_then(|path| Path::new(path).file_name())
      .is_some_and(|name| Some(name) == Path::new(program).file_name())
      && args.get(1).is_some_and(|arg| arg == "dsh")
      && choice.and_then(|choice| choice.first().cloned()).as_deref() == Some(self.name())
  }

  fn missing(&self, machine: &Machine) -> Option<String> {
    machine
      .var(&OPENROUTER.presence.key_env)
      .is_none()
      .then(|| OPENROUTER.presence.missing.clone())
  }

  fn catalogue<'a>(&'a self, machine: &'a Machine, _config_dir: Option<&'a Path>) -> Reading<'a> {
    Box::pin(async move { Ok(json!({ "models": models(machine).await? })) })
  }

  // A mapping names no effort: which levels a routed model offers is that model's, never the tier's.
  fn offered<'a>(
    &'a self,
    machine: &'a Machine,
    _config_dir: Option<&'a Path>,
    _now_ms: f64,
  ) -> Offering<'a> {
    Box::pin(async move {
      let models = models(machine).await?.into_iter().map(|model| OfferedModel {
        id: model.id,
        efforts: vec![None],
        free: Some(model.free),
        in_service: None,
      });

      Ok(Offered {
        models: models.collect(),
        stale_read_at: None,
      })
    })
  }
}

/// What a chooser needs to know about one model: which, what it is called, how much it can be told,
/// and whether it costs anything.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
  pub id: String,
  pub name: String,
  pub context_length: Option<i64>,
  pub free: bool,
}

/// A price as the listing writes it, read as a number the way a person reads one; nothing where it
/// is none.
fn price(value: Option<&Value>) -> Option<f64> {
  match value? {
    Value::Number(number) => number.as_f64(),
    Value::String(text) => text.trim().parse().ok(),
    _ => None,
  }
}

/// Whether a listed model costs nothing: a `:free` id, priced at zero for both prompt and completion.
fn free(id: &str, pricing: Option<&Value>) -> bool {
  let zero = |name: &str| price(pricing.and_then(|pricing| pricing.get(name))) == Some(0.0);

  id.ends_with(":free") && zero("prompt") && zero("completion")
}

/// The listing as the gateway answers it.
pub fn read_models(payload: &Value) -> Vec<Model> {
  payload
    .get("data")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(|row| {
      let id = row.get("id")?.as_str().filter(|id| !id.is_empty())?;

      Some(Model {
        id: id.to_owned(),
        name: row.get("name").and_then(Value::as_str).unwrap_or(id).to_owned(),
        context_length: row.get("context_length").and_then(Value::as_f64).map(|length| length as i64),
        free: free(id, row.get("pricing")),
      })
    })
    .collect()
}

/// Where the listing is read from: the stand-in's address where the environment names one.
fn listing(machine: &Machine) -> &str {
  machine.var(&OPENROUTER.catalogue.url_env).unwrap_or(&OPENROUTER.catalogue.url)
}

/// Every model OpenRouter offers, for a screen to choose a stand-in from.
pub async fn models(machine: &Machine) -> Result<Vec<Model>, String> {
  let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(OPENROUTER.catalogue.seconds))
    .build()
    .map_err(|error| format!("openrouter models: {error}"))?;
  let response = client
    .get(listing(machine))
    .header(reqwest::header::ACCEPT, "application/json")
    .send()
    .await
    .map_err(|error| format!("openrouter models: {error}"))?;
  let status = response.status();

  if !status.is_success() {
    return Err(
      format!(
        "openrouter models: HTTP {} {}",
        status.as_u16(),
        status.canonical_reason().unwrap_or("")
      )
      .trim_end()
      .to_owned(),
    );
  }

  let payload: Value = response.json().await.map_err(|error| format!("openrouter models: {error}"))?;

  Ok(read_models(&payload))
}
