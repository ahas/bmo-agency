//! The roles as the store keeps them (docs/agent-roles.md, 「What a role row holds」): every field of a role's
//! row is read off it at every use, so a change the owner makes holds from the next read with no restart.

use policy::roles::{ROLES, Sessions};
use sea_orm::{ConnectionTrait, QueryResult};
use store::{Fault, all, one};

use crate::order::Order;

/// One role as its row holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentRole {
  pub name: String,
  pub label: String,
  /// The bundles it gives, each one the policy declares.
  pub bundles: Vec<String>,
  /// The most of its runs that go at once.
  pub max_concurrent: i64,
  /// The prompt its runs open with after the harness opening, where it carries one.
  pub prompt: Option<String>,
  /// What it is for and when to call it, which a run that may call co-workers is shown.
  pub description: Option<String>,
  /// Whether its runs keep their sessions (docs/agent-roles.md, 「A role's sessions」).
  pub sessions: Sessions,
  /// The launch line its runs go out on.
  pub line: String,
  /// Its provider order with its quota share.
  pub order: Order,
  /// The share of its model's window a run's session may fill before it is folded.
  pub fold_ratio: f64,
  /// Whether a run of another role may call it as a co-worker.
  pub callable: bool,
  pub position: i64,
}

impl AgentRole {
  pub fn of(row: &QueryResult) -> Result<Self, Fault> {
    let bundles: String = row.try_get("", "bundles")?;
    let order: String = row.try_get("", "provider_order")?;

    Ok(Self {
      name: row.try_get("", "name")?,
      label: row.try_get("", "label")?,
      bundles: serde_json::from_str(&bundles).map_err(|error| Fault::new(error.to_string()))?,
      max_concurrent: row.try_get("", "max_concurrent")?,
      prompt: row.try_get("", "prompt")?,
      description: row.try_get("", "description")?,
      sessions: Sessions::of(&row.try_get::<String>("", "sessions")?).unwrap_or_default(),
      line: row.try_get("", "line")?,
      order: Order::of_row(&order, row.try_get("", "quota_share")?)
        .forcing(row.try_get("", "active_provider")?, row.try_get("", "active_model")?),
      fold_ratio: row.try_get("", "fold_ratio")?,
      callable: row.try_get::<i64>("", "callable")? != 0,
      position: row.try_get("", "position")?,
    })
  }
}

/// Every role the store holds, in the order a screen lists them.
pub async fn every<C: ConnectionTrait>(db: &C) -> Result<Vec<AgentRole>, Fault> {
  all(db, r#"SELECT * FROM "roles" ORDER BY "position", "name""#, vec![])
    .await?
    .iter()
    .map(AgentRole::of)
    .collect()
}

/// The role of this name, where the store holds one.
pub async fn named<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<AgentRole>, Fault> {
  one(db, r#"SELECT * FROM "roles" WHERE "name" = $1"#, vec![name.into()])
    .await?
    .as_ref()
    .map(AgentRole::of)
    .transpose()
}

/// Whether the store holds a role of this name.
pub async fn holds<C: ConnectionTrait>(db: &C, name: &str) -> Result<bool, Fault> {
  Ok(named(db, name).await?.is_some())
}

/// Every role's name, in the order a screen lists them, as a refusal lists them.
pub async fn names<C: ConnectionTrait>(db: &C) -> Result<Vec<String>, Fault> {
  Ok(every(db).await?.into_iter().map(|role| role.name).collect())
}

/// The bundles a role gives; none for a role the store does not hold.
pub async fn bundles_of<C: ConnectionTrait>(db: &C, name: &str) -> Result<Vec<String>, Fault> {
  Ok(named(db, name).await?.map(|role| role.bundles).unwrap_or_default())
}

/// The prompt a role's runs open with; none for a role carrying none or one the store does not hold.
pub async fn prompt_of<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<String>, Fault> {
  Ok(
    named(db, name)
      .await?
      .and_then(|role| role.prompt)
      .filter(|prompt| !prompt.trim().is_empty()),
  )
}

/// The sessions setting of a role, where the store holds it.
pub async fn sessions_of<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<Sessions>, Fault> {
  Ok(named(db, name).await?.map(|role| role.sessions))
}

/// The roles a run may call as co-workers, in the order a screen lists them.
pub async fn callable<C: ConnectionTrait>(db: &C) -> Result<Vec<AgentRole>, Fault> {
  Ok(every(db).await?.into_iter().filter(|role| role.callable).collect())
}

/// The sentence an ask naming a role the store does not hold is refused with.
pub fn unknown(name: &str) -> String {
  format!("No role named {name} is held; a run names one of the store's roles.")
}

/// The role a job runs as where its item names none.
pub fn job_role() -> &'static str {
  &ROLES.job
}
