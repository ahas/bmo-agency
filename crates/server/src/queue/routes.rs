//! Whether an item has a route to run on now (docs/cache-fallback.md, what a job does when its road
//! fails): the selections its job may go out on, in the order a job walks them, and whether one of
//! them is not blocked. The queue's telling asks it of each item at the head of a worker's queue, so a
//! worker is told open only where its turn can actually be launched.

use std::sync::LazyLock;

use harness_providers::{ARCHIVE_PROVIDERS, Machine};
use policy::fill_marks;
use policy::queue_words::QUEUE_WORDS;
use policy::work_keys::WORK_KEYS;
use regex::Regex;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use store::one;

use super::settings;
use crate::Refusal;
use crate::control::claude_accounts::Judging;
use crate::control::harness::{enabled_providers, missing, named, provider_enabled};
use crate::control::roads::{pinned_provider, walked};
use crate::control::tiers::resolve_execution;
use crate::rest::resting;

/// The work key every job of a worker goes out on.
pub const JOB_WORK_KEY: &str = "job-worker";

// `<provider>::<model>`, where the provider is any this build carries.
static CHOICE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^([a-z0-9-]+)::([a-zA-Z0-9][a-zA-Z0-9._:/-]{0,200})$").unwrap());

// A Claude model named on its own, as the oldest launches name one.
static CLAUDE_MODEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^claude-[a-zA-Z0-9._-]+$").unwrap());

/// One selection read: the provider and the model.
pub struct Choice<'a> {
  pub provider: &'a str,
  pub model: &'a str,
}

/// A selection as a provider and its model, where it names a provider this build carries.
pub fn choice(value: &str) -> Option<Choice<'_>> {
  let read = CHOICE.captures(value)?;
  let provider = read.get(1)?.as_str();

  ARCHIVE_PROVIDERS.contains(&provider).then(|| Choice {
    provider,
    model: read.get(2).map_or("", |model| model.as_str()),
  })
}

/// Whether a value is a selection a job may ask for: the automatic one, a provider's model, or a bare
/// Claude model.
pub fn is_selection(value: &str) -> bool {
  value == "auto" || choice(value).is_some() || CLAUDE_MODEL.is_match(value)
}

/// Why no selection could be made; the walk then has no route.
#[derive(Debug)]
pub struct Unrouted(pub String);

impl<E: std::fmt::Display> From<E> for Unrouted {
  fn from(error: E) -> Self {
    Self(error.to_string())
  }
}

/// The run line a job goes out on, which a provider that does not carry it never answers.
pub fn job_line() -> &'static str {
  WORK_KEYS.profile(JOB_WORK_KEY).map_or("", |profile| profile.line.as_str())
}

// What a worker is pinned to: a provider, and one model of it where the provider's models are named
// one by one — the registry. A support worker is on its base's pin, and a pin to a provider the owner
// turned off reads as none.
pub async fn worker_pin(
  db: &DatabaseConnection,
  worker: &str,
) -> Result<Option<(String, Option<String>)>, Unrouted> {
  let row = one(
    db,
    r#"SELECT COALESCE(b."pinned_provider", CASE WHEN a."support_of" IS NULL THEN a."pinned_provider" END) AS "provider",
              COALESCE(b."pinned_model", CASE WHEN a."support_of" IS NULL THEN a."pinned_model" END) AS "model"
       FROM "agents" a LEFT JOIN "agents" b ON b."name" = a."support_of" WHERE a."name" = $1"#,
    vec![worker.into()],
  )
  .await?;
  let Some(row) = row else {
    return Ok(None);
  };
  let Some(provider) = row.try_get::<Option<String>>("", "provider")? else {
    return Ok(None);
  };

  if !provider_enabled(db, &provider).await.map_err(|refused| Unrouted(refused.error))? {
    return Ok(None);
  }

  let names_models = named(&provider).map_err(|refused| Unrouted(refused.error))?.unregistered().is_some();
  let model = if names_models { row.try_get::<Option<String>>("", "model")? } else { None };

  Ok(Some((provider, model)))
}

/// The worker's selection on each provider the automatic walk asks, in the owner's provider order,
/// or on the one provider the worker is pinned to, or else the job key is set to.
pub async fn worker_selections(
  db: &DatabaseConnection,
  tier: Option<&str>,
  worker: Option<&str>,
) -> Result<Vec<String>, Unrouted> {
  let mut enabled = vec![];

  for name in enabled_providers(db).await.map_err(|refused| Unrouted(refused.error))? {
    let provider = named(&name).map_err(|refused| Unrouted(refused.error))?;

    if walked(db, provider, JOB_WORK_KEY, job_line())
      .await
      .map_err(|refused| Unrouted(refused.error))?
    {
      enabled.push(name);
    }
  }

  let pin = match worker {
    Some(worker) => worker_pin(db, worker).await?,
    None => None,
  };

  if let Some((provider, Some(model))) = &pin {
    return Ok(vec![format!("{provider}::{model}")]);
  }

  let pinned = match pin {
    Some((provider, None)) => Some(provider),
    _ => pinned_provider(db, JOB_WORK_KEY).await.map_err(|refused| Unrouted(refused.error))?,
  };
  let order = pinned.map_or(enabled, |pinned| vec![pinned]);
  let mut selections = vec![];

  for provider in order {
    let mapping = resolve_execution(db, JOB_WORK_KEY, &provider, tier).await.map_err(Unrouted)?;

    selections.push(format!("{provider}::{}", mapping.model));
  }

  Ok(selections)
}

/// The selection a job asked for as it goes out: a provider's model as it is named, a bare Claude
/// model on Claude Code, and the automatic choice on the first the worker's walk asks.
pub async fn resolve_job_model(
  db: &DatabaseConnection,
  selection: Option<&str>,
  worker: Option<&str>,
) -> Result<String, Unrouted> {
  if let Some(selection) = selection.filter(|selection| *selection != "auto" && !selection.is_empty()) {
    if !is_selection(selection) {
      return Err(Unrouted(fill_marks(&QUEUE_WORDS.routes.model_wrong, &[("selection", selection)])));
    }

    return Ok(if choice(selection).is_some() {
      selection.to_owned()
    } else {
      format!("claude-code::{selection}")
    });
  }

  worker_selections(db, None, worker)
    .await?
    .into_iter()
    .next()
    .ok_or_else(|| Unrouted(QUEUE_WORDS.routes.no_model.clone()))
}

/// Every selection a job of a worker may go out on now, in the order the job walks them: the one it
/// asked for where it named one, then the worker's walk on its tier, each kept only where its provider
/// is on, carries the job's line and can answer on this machine.
pub async fn job_route_candidates(
  db: &DatabaseConnection,
  machine: &Machine,
  requested: Option<&str>,
  worker: Option<&str>,
  tier: Option<&str>,
) -> Result<Vec<String>, Unrouted> {
  let mut choices: Vec<String> = vec![];
  let mut add = |selection: String| {
    if choice(&selection).is_some() && !choices.contains(&selection) {
      choices.push(selection);
    }
  };

  if let Some(requested) = requested.filter(|requested| *requested != "auto" && !requested.is_empty())
    && let Some(asked) = choice(requested)
    && provider_enabled(db, asked.provider).await.map_err(|refused| Unrouted(refused.error))?
  {
    add(resolve_job_model(db, Some(requested), worker).await?);
  }

  for selection in worker_selections(db, tier, worker).await? {
    add(selection);
  }

  let mut open = vec![];

  for selection in choices {
    let Some(asked) = choice(&selection) else { continue };
    let provider = named(asked.provider).map_err(|refused| Unrouted(refused.error))?;

    if !provider_enabled(db, asked.provider).await.map_err(|refused| Unrouted(refused.error))?
      || !provider.declared().carries(job_line())
    {
      continue;
    }

    if missing(db, machine, provider)
      .await
      .map_err(|refused| Unrouted(refused.error))?
      .is_none()
    {
      open.push(selection);
    }
  }

  Ok(open)
}

/// Whether a provider's models are services of their own, the registry's registrations.
pub fn own_services(provider: &str) -> bool {
  named(provider).is_ok_and(|provider| provider.unregistered().is_some())
}

// The road a failure closes: the provider, or on a provider whose models are services of their own the
// one model that did not answer.
pub fn health_key(selection: &Choice<'_>) -> String {
  if own_services(selection.provider) {
    format!("job_route_block:{}::{}", selection.provider, selection.model)
  } else {
    format!("job_route_block:{}", selection.provider)
  }
}

/// A selection's road closed until a moment, in epoch milliseconds, with the reason the failure that
/// closed it gives (docs/api.md, 「Workers」, a road's block). A value naming no provider this build carries
/// closes nothing.
pub async fn block_route(
  db: &DatabaseConnection,
  selection: &str,
  until_ms: i64,
  reason: &str,
) -> Result<bool, Refusal> {
  let Some(asked) = choice(selection) else { return Ok(false) };

  settings::set(db, &health_key(&asked), &json!({ "until": until_ms, "reason": reason }).to_string())
    .await?;

  Ok(true)
}

/// A selection's road opened again before its block ran out.
pub async fn clear_route_block(db: &DatabaseConnection, selection: &str) -> Result<(), Refusal> {
  match choice(selection) {
    Some(asked) => settings::forget(db, &health_key(&asked)).await,
    None => Ok(()),
  }
}

/// Whether a selection's road is shut now: its provider off, Claude Code while the deployment rests, or a
/// failure that closed it still standing.
pub async fn route_blocked(judging: Judging<'_>, selection: &str, now_ms: i64) -> Result<bool, Unrouted> {
  let db = judging.db;
  let Some(asked) = choice(selection) else {
    return Ok(false);
  };

  if !provider_enabled(db, asked.provider).await.map_err(|refused| Unrouted(refused.error))? {
    return Ok(true);
  }

  if asked.provider == "claude-code"
    && resting(judging, now_ms / 1000).await.map_err(|refusal| Unrouted(refusal.detail))?
  {
    return Ok(true);
  }

  let block = settings::get(db, &health_key(&asked))
    .await
    .map_err(|refusal| Unrouted(refusal.detail))?;
  let until = block
    .and_then(|text| serde_json::from_str::<Value>(&text).ok())
    .and_then(|read| read.get("until").and_then(Value::as_f64));

  Ok(until.is_some_and(|until| until > now_ms as f64))
}

/// Whether an item has a route to run on now: some selection its job may go out on that is not
/// blocked. A walk that could not be read is no route.
pub async fn item_route_open(
  judging: Judging<'_>,
  machine: &Machine,
  model: Option<&str>,
  worker: Option<&str>,
  now_ms: i64,
) -> bool {
  let Ok(candidates) =
    job_route_candidates(judging.db, machine, Some(model.unwrap_or("auto")), worker, None).await
  else {
    return false;
  };

  for selection in candidates {
    match route_blocked(judging, &selection, now_ms).await {
      Ok(false) => return true,
      Ok(true) => {}
      Err(_) => return false,
    }
  }

  false
}
