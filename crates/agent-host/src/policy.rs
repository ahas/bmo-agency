//! The store's word on whether a child may start, read again at the moment it would: the provider
//! order and the cancellation markers, read as the launcher reads them.

use std::collections::HashSet;

use serde_json::Value;
use sqlx::Row;
use store::{open_read, retrying};

use crate::envelope::Policy;

/// Why a child must not start.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
  /// A cancellation stands over one of its names: the stop reaching this run.
  Cancelled,
  /// Its provider is switched off.
  Disabled,
}

const ORDER_KEY: &str = "provider_order";
const CANCEL_PREFIX: &str = "execution_cancel:";

/// Whether `provider` is on under a stored order. A provider the order does not name — one the build
/// gained after it was saved — is on. Where the build's providers are known, an order naming one
/// outside them, naming one twice, of any other shape, or leaving none on reads as every provider on.
pub fn provider_on(stored: Option<&str>, provider: &str, providers: Option<&[String]>) -> bool {
  let Some(entries) = stored
    .and_then(|text| serde_json::from_str::<Value>(text).ok())
    .and_then(|value| read_order(&value))
  else {
    return true;
  };
  let names: HashSet<&str> = entries.iter().map(|(name, _)| name.as_str()).collect();
  let known = providers.is_none_or(|known| names.iter().all(|name| known.iter().any(|one| one == name)));
  let missing_on = providers.is_some_and(|known| known.iter().any(|one| !names.contains(one.as_str())));

  if names.len() != entries.len() || !known || !(entries.iter().any(|(_, on)| *on) || missing_on) {
    return true;
  }

  entries.iter().find(|(name, _)| name == provider).is_none_or(|(_, on)| *on)
}

// An order's entries: all names, or all entries with an on/off.
fn read_order(value: &Value) -> Option<Vec<(String, bool)>> {
  let list = value.as_array()?;

  if list.iter().all(Value::is_string) {
    return Some(list.iter().filter_map(|name| Some((name.as_str()?.to_owned(), true))).collect());
  }

  list
    .iter()
    .map(|entry| Some((entry.get("name")?.as_str()?.to_owned(), entry.get("enabled")?.as_bool()?)))
    .collect()
}

// The settings the policy reads: the provider order, and every cancellation marker standing over one
// of the run's names. The store is read at the address the policy names, on a runtime of this call's
// own, since the host waits on its children from plain threads.
fn read_settings(address: &str, names: &[String]) -> Result<(Option<String>, bool), String> {
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .map_err(|why| format!("Error: {why}"))?;

  // Read again whole where the database went away under the reading (docs/store.md, 「Connecting」).
  runtime
    .block_on(retrying(|| async move {
      let mut db = open_read(address).await?;
      let order: Option<Option<String>> = sqlx::query("SELECT value FROM settings WHERE key = $1")
        .bind(ORDER_KEY)
        .fetch_optional(&mut db)
        .await?
        .map(|row| row.try_get(0))
        .transpose()?;
      let mut cancelled = false;

      for name in names {
        let marked = sqlx::query("SELECT 1 FROM settings WHERE key = $1")
          .bind(format!("{CANCEL_PREFIX}{name}"))
          .fetch_optional(&mut db)
          .await?;

        cancelled |= marked.is_some();
      }

      Ok::<_, sqlx::Error>((order.flatten(), cancelled))
    }))
    .map_err(|why| format!("Error: {why}"))
}

/// The store's answer for this policy, or why it could not be read.
pub fn refusal(policy: &Policy) -> Result<Option<Refusal>, String> {
  let (order, cancelled) = read_settings(&policy.address, &policy.names)?;

  if cancelled {
    return Ok(Some(Refusal::Cancelled));
  }

  if !provider_on(order.as_deref(), &policy.provider, policy.providers.as_deref()) {
    return Ok(Some(Refusal::Disabled));
  }

  Ok(None)
}
