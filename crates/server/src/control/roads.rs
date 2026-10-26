//! Each work key's road as the fallback settings read and set it (docs/cache-fallback.md, 「The per-key
//! choice is automatic or one provider」): the route row the owner wrote, the provider the key's road
//! begins on, the roads the key is offered with why each is shut, and Laya for a gate whose profile
//! says Laya can give its whole verdict, the gate's own service, which runs on no agent provider.

use harness_providers::{ARCHIVE_PROVIDERS, Machine, Provider, Reason, providers};
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::work_keys::{Profile, WORK_KEYS};
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::Refused;
use super::harness::{enabled_providers, named, offered_reason, provider_enabled};
use super::registry;
use store::{one, run};

/// The road of a gate that stands on Laya, which is no agent provider.
pub const LAYA: &str = "laya";

/// The road that walks the providers in the owner's order.
pub const AUTO: &str = "auto";

/// What the owner wrote for one work key: its road, the Claude login it is pinned to, the registration it
/// runs on at the registry's provider in place of its tier's cell (docs/own-llm.md, 「A key's own
/// registration」), and when.
#[derive(Clone, Debug)]
pub struct RouteRow {
  pub route_mode: String,
  pub account: Option<String>,
  pub registration: Option<String>,
  pub set_at: i64,
}

/// The profile of a work key, where one declares it.
pub fn profile(key: &str) -> Option<&'static Profile> {
  WORK_KEYS.profile(key)
}

/// Whether a road may stand in a route row: the walk, a provider this build carries, or Laya.
pub fn is_route_mode(mode: &str) -> bool {
  mode == AUTO || mode == LAYA || ARCHIVE_PROVIDERS.contains(&mode)
}

/// The row the owner wrote for a key, or none.
pub async fn route_row(db: &DatabaseConnection, key: &str) -> Result<Option<RouteRow>, Refused> {
  let Some(row) = one(
    db,
    "SELECT route_mode, account, registration, set_at FROM fallback_routes WHERE key = $1",
    vec![key.into()],
  )
  .await?
  else {
    return Ok(None);
  };

  Ok(Some(RouteRow {
    route_mode: row.try_get("", "route_mode")?,
    account: row.try_get("", "account")?,
    registration: row.try_get("", "registration")?,
    set_at: row.try_get("", "set_at")?,
  }))
}

/// The road a key takes where the owner wrote none.
pub fn default_mode(key: &str) -> &str {
  profile(key).map_or(AUTO, |profile| profile.default_route_mode.as_str())
}

/// The road the key stands on: the owner's row, or its profile's default.
pub async fn selected_mode(db: &DatabaseConnection, key: &str) -> Result<String, Refused> {
  Ok(match route_row(db, key).await? {
    Some(row) => row.route_mode,
    None => default_mode(key).to_owned(),
  })
}

/// The one provider a key is set to run on, or none where it walks: a key set to a provider the owner
/// has turned off walks too, and a gate on Laya runs on no agent provider.
pub async fn pinned_provider(db: &DatabaseConnection, key: &str) -> Result<Option<String>, Refused> {
  let mode = selected_mode(db, key).await?;

  if mode == AUTO || mode == LAYA || !provider_enabled(db, &mode).await? {
    return Ok(None);
  }

  Ok(Some(mode))
}

/// Whether the automatic walk of a key asks a provider: it carries the key's line and, where its
/// models are the registry's, the registration the key's tier maps to there is walked.
pub(crate) async fn walked(
  db: &DatabaseConnection,
  provider: &dyn Provider,
  key: &str,
  line: &str,
) -> Result<bool, Refused> {
  if !provider.declared().carries(line) {
    return Ok(false);
  }

  if provider.unregistered().is_some() {
    return Ok(registry::walks(db, key).await);
  }

  Ok(true)
}

/// The provider a work key's road begins on: the one it is set to, the first the walk asks, the first
/// on that carries its line, or the first on at all.
pub async fn road_provider(db: &DatabaseConnection, key: &str) -> Result<String, Refused> {
  if let Some(pinned) = pinned_provider(db, key).await? {
    return Ok(pinned);
  }

  let line = profile(key).map_or("worker", |profile| profile.line.as_str());
  let enabled = enabled_providers(db).await?;

  for name in &enabled {
    if walked(db, named(name)?, key, line).await? {
      return Ok(name.clone());
    }
  }

  for name in &enabled {
    if named(name)?.declared().carries(line) {
      return Ok(name.clone());
    }
  }

  enabled.first().cloned().ok_or_else(|| Refused::failed("no provider is on"))
}

/// One road a key is offered: a provider, or Laya, selectable or shut with the reason why.
#[derive(Clone, Debug)]
pub struct OfferedRoad {
  pub name: String,
  pub label: String,
  pub reason: Option<Reason>,
}

/// The code of the reason Laya gives where the process names no address to ask it at.
pub const LAYA_UNSET: &str = "laya-url-unset";

impl OfferedRoad {
  pub fn view(&self) -> Value {
    json!({
      "name": self.name,
      "code": self.name,
      "label": self.label,
      "reason": self.reason.as_ref().map(|reason| &reason.text),
      "reasonCode": self.reason.as_ref().map(|reason| &reason.code),
    })
  }
}

/// Every road a work key may be put on: each provider on its line, shut where this machine lacks it or
/// its program does not carry the line, and Laya for a gate whose profile offers it, shut where the
/// process names no address to ask it at.
pub async fn offered_roads(
  db: &DatabaseConnection,
  machine: &Machine,
  key: &str,
) -> Result<Vec<OfferedRoad>, Refused> {
  let profile = profile(key);
  let line = profile.map_or("worker", |profile| profile.line.as_str());
  let mut roads = Vec::new();

  for provider in providers() {
    roads.push(OfferedRoad {
      name: provider.name().to_owned(),
      label: provider.declared().label.clone(),
      reason: offered_reason(db, machine, provider, line).await?,
    });
  }

  if profile.is_some_and(|profile| profile.laya_road) {
    let addressed = machine.var("BMO_LAYA_URL").is_some_and(|url| !url.trim().is_empty());

    roads.push(OfferedRoad {
      name: LAYA.to_owned(),
      label: "Laya".to_owned(),
      reason: (!addressed).then(|| Reason::new(LAYA_UNSET, &CONTROL_WORDS.fallback.laya_unset)),
    });
  }

  Ok(roads)
}

/// Why a road may not be chosen for a key, or nothing where it may: the walk always may, a road is
/// refused for the reason the screen shows beside it, and one the key is not offered as such.
pub async fn mode_refusal(
  db: &DatabaseConnection,
  machine: &Machine,
  key: &str,
  mode: &str,
) -> Result<Option<String>, Refused> {
  if mode == AUTO {
    return Ok(None);
  }

  Ok(match offered_roads(db, machine, key).await?.into_iter().find(|one| one.name == mode) {
    Some(road) => road.reason.map(|reason| reason.text),
    None => Some(fill_marks(&CONTROL_WORDS.fallback.not_offered, &[("key", key), ("mode", mode)])),
  })
}

/// A key's road written.
pub async fn set_mode(db: &DatabaseConnection, key: &str, mode: &str, at: i64) -> Result<(), Refused> {
  run(
    db,
    "INSERT INTO fallback_routes (key, set_at, route_mode) VALUES ($1, $2, $3)
     ON CONFLICT (key) DO UPDATE SET route_mode = excluded.route_mode, set_at = excluded.set_at",
    vec![key.into(), at.into(), mode.into()],
  )
  .await?;

  delete_if_empty(db, key).await
}

/// The Claude login a key's runs go out on written: one account, or none to walk the order.
pub async fn set_account(
  db: &DatabaseConnection,
  key: &str,
  account: Option<&str>,
  at: i64,
) -> Result<(), Refused> {
  run(
    db,
    "INSERT INTO fallback_routes (key, set_at, route_mode, account) VALUES ($1, $2, $3, $4)
     ON CONFLICT (key) DO UPDATE SET account = excluded.account, set_at = excluded.set_at",
    vec![key.into(), at.into(), default_mode(key).into(), account.map(str::to_owned).into()],
  )
  .await?;

  delete_if_empty(db, key).await
}

/// The registration a key runs on at the registry's provider written: one registration, or none to read the
/// tier's cell.
pub async fn set_registration(
  db: &DatabaseConnection,
  key: &str,
  registration: Option<&str>,
  at: i64,
) -> Result<(), Refused> {
  run(
    db,
    "INSERT INTO fallback_routes (key, set_at, route_mode, registration) VALUES ($1, $2, $3, $4)
     ON CONFLICT (key) DO UPDATE SET registration = excluded.registration, set_at = excluded.set_at",
    vec![key.into(), at.into(), default_mode(key).into(), registration.map(str::to_owned).into()],
  )
  .await?;

  delete_if_empty(db, key).await
}

/// Why a key may not name a registration of its own, or nothing where it may: the registry holds it with an
/// address.
pub async fn registration_refusal(db: &DatabaseConnection, name: &str) -> Result<Option<String>, Refused> {
  let held = registry::registration(db, name).await?;

  Ok(
    held
      .is_none_or(|held| held.addresses.is_empty())
      .then(|| format!("{}: {name}", policy::providers::registered::REGISTERED.presence.not_registered)),
  )
}

/// The registration a key names of its own, where it names one.
pub async fn key_registration(db: &DatabaseConnection, key: &str) -> Result<Option<String>, Refused> {
  Ok(route_row(db, key).await?.and_then(|row| row.registration))
}

// A row that says nothing the key's defaults do not is removed, so the key follows its profile again.
async fn delete_if_empty(db: &DatabaseConnection, key: &str) -> Result<(), Refused> {
  run(
    db,
    "DELETE FROM fallback_routes WHERE key = $1 AND codex_model IS NULL AND codex_effort IS NULL
       AND openrouter_model IS NULL AND claude_model IS NULL AND account IS NULL AND registration IS NULL
       AND route_mode = $2",
    vec![key.into(), default_mode(key).into()],
  )
  .await?;

  Ok(())
}
