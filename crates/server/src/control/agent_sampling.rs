#![allow(
  dead_code,
  unused_imports,
  unused_variables,
  clippy::needless_lifetimes,
  clippy::let_and_return,
  clippy::unnecessary_to_owned
)]
//! A worker's own sampling (docs/own-llm.md, 「A worker pinned to a provider」): the settings it takes with
//! their bounds, the values it holds, and a save of them, kept in the store's settings under its name. A
//! support worker runs on its base's, so only a worker of its own holds any. A save stands the pinned
//! workers' programs again, as a new pin does, so the next run carries the new values.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use policy::companions::GENERATION;
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use sea_orm::DatabaseConnection;
use serde_json::{Map, Value, json};

use super::companions::{generation_refusal, generation_settings};
use super::fault::{Fault, refused};
use super::{Call, Refused, Route, answer};
use crate::queue::settings;
use store::one;

// The setting a run's answer is held under, which a worker's own sampling leaves to its line.
const ANSWER_CEILING: &str = "max_tokens";

// Every refusal of a sampling is about the sampling as a whole, but for a value out of its setting's
// bounds.
const WHOLE: &str = "sampling";

pub fn key(agent: &str) -> String {
  format!("agent_sampling:{agent}")
}

/// The settings a worker samples with, with their bounds: every generation setting but the answer's
/// ceiling.
fn sampling_settings() -> Vec<Value> {
  generation_settings(None)
    .into_iter()
    .filter(|setting| setting["name"] != ANSWER_CEILING)
    .collect()
}

// What a stored sampling holds: every entry whose value is a finite number or a switch.
pub fn held_values(text: Option<String>) -> Map<String, Value> {
  let Some(Value::Object(read)) = text.and_then(|text| serde_json::from_str(&text).ok()) else {
    return Map::new();
  };

  read
    .into_iter()
    .filter(|(_, value)| value.is_boolean() || value.as_f64().is_some_and(f64::is_finite))
    .collect()
}

// The worker a name stands for, where it holds sampling of its own: its name as the store keeps it.
pub async fn sampling_owner(db: &DatabaseConnection, name: &str) -> Result<String, Fault> {
  let words = &CONTROL_WORDS.workers.sampling;
  let Some(row) = one(
    db,
    r#"SELECT "name", "kind", "label", "support_of" FROM "agents" WHERE "name" = $1"#,
    vec![name.into()],
  )
  .await?
  else {
    return refused(WHOLE, fill_marks(&words.no_agent, &[("agent", name)]));
  };
  let label = row.try_get::<Option<String>>("", "label")?.unwrap_or_else(|| name.to_owned());

  if row.try_get::<String>("", "kind")? != "worker" {
    return refused(WHOLE, fill_marks(&words.not_worker, &[("label", &label)]));
  }

  if row.try_get::<Option<String>>("", "support_of")?.is_some() {
    return refused(WHOLE, fill_marks(&words.follows_base, &[("label", &label)]));
  }

  Ok(row.try_get("", "name")?)
}

pub async fn view(db: &DatabaseConnection, agent: &str) -> Result<Value, Fault> {
  let values = held_values(settings::get(db, &key(agent)).await?);

  Ok(json!({ "agent": agent, "settings": sampling_settings(), "values": values }))
}

// The settings a save names laid over the held ones: each a value its setting takes, or null to leave it to
// the engine. One out of its bounds refuses the whole save.
pub fn changed(asked: &Value, mut held: Map<String, Value>) -> Result<Map<String, Value>, Fault> {
  let words = &CONTROL_WORDS.workers.sampling;
  let Value::Object(asked) = asked else {
    return refused(WHOLE, words.shape.clone());
  };

  for (name, value) in asked {
    let Some(setting) = GENERATION.setting(name).filter(|setting| setting.name != ANSWER_CEILING) else {
      return refused(WHOLE, fill_marks(&words.unknown, &[("name", name)]));
    };

    if value.is_null() {
      held.remove(name);
      continue;
    }

    if let Some(refusal) = generation_refusal(setting, value, None) {
      return refused(name, refusal);
    }

    held.insert(name.clone(), value.clone());
  }

  Ok(held)
}

/// One worker's own sampling with the settings it takes.
pub struct AgentSampling;

impl Route for AgentSampling {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/agent-sampling";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let db = call.store();
    let name = call.query("agent").unwrap_or_default().trim().to_owned();
    let shown = async { view(db, &sampling_owner(db, &name).await?).await };

    match shown.await {
      Ok(view) => Ok(answer(StatusCode::OK, view)),
      Err(fault) => fault.answer(),
    }
  }
}

/// A worker's own sampling saved, answered as it then stands.
pub struct AgentSamplingSave;

impl Route for AgentSamplingSave {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/agent-sampling";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let name = body.get("agent").and_then(Value::as_str).map(str::trim).unwrap_or_default();
    let db = call.store();
    let saved = async {
      let agent = sampling_owner(db, name).await?;
      let held = held_values(settings::get(db, &key(&agent)).await?);
      let next = changed(body.get("sampling").unwrap_or(&Value::Null), held)?;

      settings::set(db, &key(&agent), &Value::Object(next).to_string()).await?;

      view(db, &agent).await
    };

    match saved.await {
      Ok(view) => {
        if let Some(places) = call.places().cloned() {
          tokio::spawn(async move { crate::places::standby::restand_all(&places.ground, false).await });
        }

        Ok(answer(StatusCode::OK, view))
      }
      Err(fault) => fault.answer(),
    }
  }
}
