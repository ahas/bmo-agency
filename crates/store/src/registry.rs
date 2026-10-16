//! The model registry as it stands (docs/own-llm.md, 「Registered models」): every registration with
//! its addresses, in the order the registry lists them: registrations by when they were added, then by
//! name; a registration's addresses by their place in it, then by node.

use sqlx::any::AnyRow;
use sqlx::{AnyConnection, Row};

use crate::number;

/// One registered model: what it is called and asked for, and where it is served.
#[derive(Clone, Debug, PartialEq)]
pub struct Registration {
  pub name: String,
  pub label: String,
  pub model: String,
  pub context_tokens: i64,
  pub answer_tokens: i64,
  /// How long a request on it is waited for, in seconds.
  pub wait_seconds: i64,
  pub thinking: bool,
  pub walked: bool,
  pub keeps_research: bool,
  pub addresses: Vec<RegisteredAddress>,
}

/// One node a registration is served on.
#[derive(Clone, Debug, PartialEq)]
pub struct RegisteredAddress {
  pub node: String,
  /// The OpenAI-compatible chat completions address.
  pub url: String,
  /// The key's file under the data root's `env/`, absent where the address takes no key.
  pub key_file: Option<String>,
  /// Whether agency wrote that file.
  pub key_owned: bool,
  /// When the address was last given a new session, in seconds.
  pub placed_at: Option<i64>,
}

// An integer column, however the writer stored it.
fn whole(row: &AnyRow, column: &str) -> Result<Option<i64>, sqlx::Error> {
  Ok(number(row, column)?.map(|value| value as i64))
}

fn flag(row: &AnyRow, column: &str) -> Result<bool, sqlx::Error> {
  Ok(whole(row, column)? == Some(1))
}

/// Every registration with its addresses.
pub async fn registrations(store: &mut AnyConnection) -> Result<Vec<Registration>, sqlx::Error> {
  let models = sqlx::query(
    "SELECT name, label, model, context_tokens, answer_tokens, wait_seconds, thinking, walked, keeps_research \
     FROM registered_models ORDER BY added_at, name",
  )
  .fetch_all(&mut *store)
  .await?;
  let addresses = sqlx::query(
    "SELECT registration, node, url, key_file, key_owned, placed_at \
     FROM registered_model_addresses ORDER BY registration, position, node",
  )
  .fetch_all(&mut *store)
  .await?;

  let mut read = Vec::with_capacity(models.len());
  for row in &models {
    let name: String = row.try_get("name")?;
    let mut served = Vec::new();
    for address in &addresses {
      if address.try_get::<String, _>("registration")? != name {
        continue;
      }
      served.push(RegisteredAddress {
        node: address.try_get("node")?,
        url: address.try_get("url")?,
        key_file: address.try_get("key_file")?,
        key_owned: flag(address, "key_owned")?,
        placed_at: whole(address, "placed_at")?,
      });
    }
    read.push(Registration {
      label: row.try_get("label")?,
      model: row.try_get("model")?,
      context_tokens: whole(row, "context_tokens")?.unwrap_or(0),
      answer_tokens: whole(row, "answer_tokens")?.unwrap_or(0),
      wait_seconds: whole(row, "wait_seconds")?.unwrap_or(0),
      thinking: flag(row, "thinking")?,
      walked: flag(row, "walked")?,
      keeps_research: flag(row, "keeps_research")?,
      addresses: served,
      name,
    });
  }
  Ok(read)
}
