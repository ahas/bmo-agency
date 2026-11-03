//! The supervisors as the store keeps them (docs/supervisors.md, 「What a supervisor row holds」): every field
//! of a supervisor's row is read off it at every use.

use std::collections::BTreeMap;

use sea_orm::{ConnectionTrait, QueryResult};
use store::{Fault, all, one};

use crate::order::Order;

/// One supervisor as its row holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Supervisor {
  pub name: String,
  pub label: Option<String>,
  pub owner_account: Option<i64>,
  pub aliases: Option<String>,
  pub wake_words: Option<String>,
  pub icon: Option<String>,
  pub prompts: Option<String>,
  pub brain_folder: Option<String>,
  pub voice: Option<String>,
  pub session_scope: String,
  pub turn_shape: String,
  /// The repository its TODO items go to.
  pub repository: Option<String>,
  pub order: Order,
  /// For each provider, whether its prefix is kept warm while idle there.
  pub warming: BTreeMap<String, bool>,
  pub fold_ratio: f64,
  pub builtin: bool,
  /// What its last turn went out on.
  pub last_mode: Option<String>,
  pub last_model: Option<String>,
  pub last_effort: Option<String>,
  pub session_id: Option<String>,
}

impl Supervisor {
  pub fn of(row: &QueryResult) -> Result<Self, Fault> {
    let order: String = row.try_get("", "provider_order")?;
    let warming: String = row.try_get("", "warming")?;

    Ok(Self {
      name: row.try_get("", "name")?,
      label: row.try_get("", "label")?,
      owner_account: row.try_get("", "owner_account")?,
      aliases: row.try_get("", "aliases")?,
      wake_words: row.try_get("", "wake_words")?,
      icon: row.try_get("", "icon")?,
      prompts: row.try_get("", "prompts")?,
      brain_folder: row.try_get("", "brain_folder")?,
      voice: row.try_get("", "voice")?,
      session_scope: row.try_get("", "session_scope")?,
      turn_shape: row.try_get("", "turn_shape")?,
      repository: row.try_get("", "repository")?,
      order: Order::of_row(&order, row.try_get("", "quota_share")?)
        .forcing(row.try_get("", "active_provider")?, row.try_get("", "active_model")?),
      warming: serde_json::from_str(&warming).unwrap_or_default(),
      fold_ratio: row.try_get("", "fold_ratio")?,
      builtin: row.try_get::<i64>("", "builtin")? != 0,
      last_mode: row.try_get("", "last_mode")?,
      last_model: row.try_get("", "last_model")?,
      last_effort: row.try_get("", "last_effort")?,
      session_id: row.try_get("", "session_id")?,
    })
  }

  /// Whether its prefix is kept warm while it stands idle on this provider: the row's switch for it, off
  /// where the row names none.
  pub fn warms_on(&self, provider: &str) -> bool {
    self.warming.get(provider).copied().unwrap_or(false)
  }
}

/// Every supervisor the store holds, by name.
pub async fn every<C: ConnectionTrait>(db: &C) -> Result<Vec<Supervisor>, Fault> {
  all(db, r#"SELECT * FROM "supervisors" ORDER BY "name""#, vec![])
    .await?
    .iter()
    .map(Supervisor::of)
    .collect()
}

/// The supervisor of this name, where the store holds one.
pub async fn named<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<Supervisor>, Fault> {
  one(db, r#"SELECT * FROM "supervisors" WHERE "name" = $1"#, vec![name.into()])
    .await?
    .as_ref()
    .map(Supervisor::of)
    .transpose()
}
