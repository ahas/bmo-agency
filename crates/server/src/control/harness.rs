//! The harness providers as the routes ask them (docs/provider-presence.md): the owner's order and on/off,
//! whether each provider can answer on this machine, which roads a work key is offered, and what each
//! offers a tier mapping. The machine's answers are the harness-providers crate's; what the store holds —
//! the order, the registrations — is read here.

use harness_providers::{ARCHIVE_PROVIDERS, Machine, Offered, OfferedModel, Provider, Reason, provider};
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::reasoning::REASONING;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::Value;

use store::{all, run};

use super::claude_accounts::{self, Judging};
use super::{Refused, set_setting, setting};
use crate::now_ms;

// Where the owner's order is kept.
const ORDER_KEY: &str = "provider_order";

/// One provider of the owner's order, on or off.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OrderEntry {
  pub name: String,
  pub enabled: bool,
}

/// The scale's levels a run of this model on this provider takes, in the scale's order: the model's own
/// efforts where the provider's catalogue names them, and the provider's words otherwise.
pub async fn levels(
  judging: Judging<'_>,
  machine: &Machine,
  provider: &dyn Provider,
  model: &str,
) -> Vec<String> {
  let dir = if provider.catalogue_under_login() {
    claude_accounts::catalogue_dir(judging, machine).await.ok()
  } else {
    None
  };
  let words = match provider.model_efforts(machine, dir.as_deref(), model).await {
    Some(words) => words,
    None => provider.declared().effort.words.clone(),
  };

  REASONING.levels.iter().filter(|level| words.contains(level)).cloned().collect()
}

/// A provider this build carries.
pub fn named(name: &str) -> Result<&'static dyn Provider, Refused> {
  provider(name).ok_or_else(|| Refused::failed(format!("no provider named {name} in this build")))
}

/// A saved order as it reads under these providers: its own order and on/off kept, and every provider
/// it does not name appended at the end, on. A value naming a provider outside the list, naming one
/// twice, of any other shape, or leaving none on, reads as the default.
pub fn stored_order(stored: Option<&str>) -> Vec<OrderEntry> {
  let default = || {
    ARCHIVE_PROVIDERS
      .iter()
      .map(|name| OrderEntry {
        name: (*name).into(),
        enabled: true,
      })
      .collect()
  };
  let Some(Value::Array(entries)) = stored.and_then(|text| serde_json::from_str::<Value>(text).ok()) else {
    return default();
  };
  let named: Option<Vec<OrderEntry>> = if entries.iter().all(Value::is_string) {
    entries
      .iter()
      .map(|one| {
        Some(OrderEntry {
          name: one.as_str()?.into(),
          enabled: true,
        })
      })
      .collect()
  } else {
    entries
      .iter()
      .map(|one| {
        Some(OrderEntry {
          name: one.get("name")?.as_str()?.into(),
          enabled: one.get("enabled")?.as_bool()?,
        })
      })
      .collect()
  };
  let Some(mut read) = named else {
    return default();
  };
  let unique = read
    .iter()
    .enumerate()
    .all(|(at, one)| read[..at].iter().all(|before| before.name != one.name));

  if !unique || !read.iter().all(|one| ARCHIVE_PROVIDERS.contains(&one.name.as_str())) {
    return default();
  }

  for name in ARCHIVE_PROVIDERS {
    if !read.iter().any(|one| one.name == name) {
      read.push(OrderEntry {
        name: name.into(),
        enabled: true,
      });
    }
  }

  if read.iter().any(|one| one.enabled) { read } else { default() }
}

/// The owner's provider order as every reader reads it.
pub async fn provider_order(db: &DatabaseConnection) -> Result<Vec<OrderEntry>, Refused> {
  Ok(stored_order(setting(db, ORDER_KEY).await?.as_deref()))
}

/// Whether the owner has a provider on.
pub async fn provider_enabled(db: &DatabaseConnection, name: &str) -> Result<bool, Refused> {
  Ok(provider_order(db).await?.iter().any(|one| one.name == name && one.enabled))
}

/// The providers the owner has on, in the owner's order.
pub async fn enabled_providers(db: &DatabaseConnection) -> Result<Vec<String>, Refused> {
  Ok(
    provider_order(db)
      .await?
      .into_iter()
      .filter(|one| one.enabled)
      .map(|one| one.name)
      .collect(),
  )
}

/// The first provider the owner has on whose program carries a line, as a runner line's agent is shown
/// beginning on one.
pub async fn line_provider(db: &DatabaseConnection, line: &str) -> Result<String, String> {
  for entry in provider_order(db).await.map_err(|refused| refused.error)? {
    if entry.enabled && named(&entry.name).map_err(|refused| refused.error)?.declared().carries(line) {
      return Ok(entry.name);
    }
  }

  Err(format!("No enabled provider carries the {line} line"))
}

/// What an order to save is refused with where it does not name every provider once.
pub fn order_shape_reason() -> String {
  let labels: Vec<&str> =
    harness_providers::providers().iter().map(|one| one.declared().label.as_str()).collect();

  fill_marks(&CONTROL_WORDS.fallback.order, &[("providers", &labels.join(", "))])
}

/// An order to save as it reads against the order held: every provider once, all named alone — each
/// keeping its own on/off — or all as entries with their on/off. Nothing for any other value.
pub fn order_to_save(value: &Value, held: &[OrderEntry]) -> Option<Vec<OrderEntry>> {
  let entries = value.as_array().filter(|entries| entries.len() == ARCHIVE_PROVIDERS.len())?;
  let read: Vec<OrderEntry> = if entries.iter().all(Value::is_string) {
    entries
      .iter()
      .map(|one| {
        let name = one.as_str()?;

        Some(OrderEntry {
          name: name.into(),
          enabled: held.iter().find(|held| held.name == name)?.enabled,
        })
      })
      .collect::<Option<_>>()?
  } else {
    entries
      .iter()
      .map(|one| {
        Some(OrderEntry {
          name: one.get("name")?.as_str()?.into(),
          enabled: one.get("enabled")?.as_bool()?,
        })
      })
      .collect::<Option<_>>()?
  };
  let unique = read
    .iter()
    .enumerate()
    .all(|(at, one)| read[..at].iter().all(|before| before.name != one.name));

  (unique && read.iter().all(|one| ARCHIVE_PROVIDERS.contains(&one.name.as_str()))).then_some(read)
}

/// Whether an order is the default: every provider on, in the providers' own order.
pub fn is_default_order(order: &[OrderEntry]) -> bool {
  order.len() == ARCHIVE_PROVIDERS.len()
    && order.iter().zip(ARCHIVE_PROVIDERS).all(|(one, name)| one.enabled && one.name == name)
}

/// The owner's order saved, or the refusal of one leaving every provider off. The default is kept as
/// no setting at all, so a provider a later build gains joins it.
pub async fn set_provider_order(db: &DatabaseConnection, order: &[OrderEntry]) -> Result<(), Refused> {
  if !order.iter().any(|one| one.enabled) {
    return Err(Refused::failed(CONTROL_WORDS.fallback.all_off.as_str()));
  }

  if is_default_order(order) {
    run(db, r#"DELETE FROM "settings" WHERE "key" = $1"#, vec![ORDER_KEY.into()]).await?;
  } else {
    set_setting(db, ORDER_KEY, &serde_json::to_string(order).map_err(Refused::failed)?).await?;
  }

  Ok(())
}

/// The registrations with an address, in the order they were added.
pub async fn addressed_registrations(db: &DatabaseConnection) -> Result<Vec<String>, Refused> {
  all(
    db,
    r#"SELECT name FROM registered_models WHERE EXISTS
         (SELECT 1 FROM registered_model_addresses WHERE registration = registered_models.name)
       ORDER BY added_at, name"#,
    vec![],
  )
  .await?
  .iter()
  .map(|row| Ok(row.try_get("", "name")?))
  .collect()
}

/// The code of the reason a provider whose program does not carry a line gives.
pub const NOT_CARRIED: &str = "line-not-carried";

/// Why this machine cannot use a provider now, or nothing where it can: the machine's answer, then the
/// registry's where the provider's models are registrations.
pub async fn missing(
  db: &DatabaseConnection,
  machine: &Machine,
  provider: &dyn Provider,
) -> Result<Option<Reason>, Refused> {
  if let Some(missing) = provider.missing(machine) {
    return Ok(Some(missing));
  }

  match provider.unregistered() {
    Some(reason) if addressed_registrations(db).await?.is_empty() => Ok(Some(reason)),
    _ => Ok(None),
  }
}

/// Why a work key on this line may not be put on a provider, or nothing where it may: the provider
/// cannot answer here, or its program does not carry the line.
pub async fn offered_reason(
  db: &DatabaseConnection,
  machine: &Machine,
  provider: &dyn Provider,
  line: &str,
) -> Result<Option<Reason>, Refused> {
  if let Some(missing) = missing(db, machine, provider).await? {
    return Ok(Some(missing));
  }

  let declared = provider.declared();

  Ok((!declared.carries(line)).then(|| Reason {
    code: NOT_CARRIED.to_owned(),
    text: fill_marks(&CONTROL_WORDS.fallback.not_carried, &[("provider", &declared.label)]),
  }))
}

/// What a provider offers a tier mapping, read afresh: the registrations with an address, each taking
/// the provider's whole effort vocabulary, for a provider whose models they are; otherwise the
/// provider's own catalogue, under the walking login's directory where it keeps it there. A catalogue
/// that cannot be read is the refusal it gave.
pub async fn offered(
  judging: Judging<'_>,
  machine: &Machine,
  provider: &dyn Provider,
) -> Result<Offered, String> {
  if provider.unregistered().is_some() {
    let efforts: Vec<Option<String>> = provider
      .declared()
      .effort
      .vocabulary()
      .into_iter()
      .map(|word| word.map(str::to_owned))
      .collect();
    let models = addressed_registrations(judging.db).await.map_err(|refused| refused.error)?;

    return Ok(Offered {
      models: models
        .into_iter()
        .map(|id| OfferedModel {
          id,
          efforts: efforts.clone(),
          free: None,
          in_service: None,
        })
        .collect(),
      stale_read_at: None,
    });
  }

  let dir = if provider.catalogue_under_login() {
    Some(
      claude_accounts::catalogue_dir(judging, machine)
        .await
        .map_err(|refused| refused.error)?,
    )
  } else {
    None
  };

  provider.offered(machine, dir.as_deref(), now_ms() as f64).await
}
