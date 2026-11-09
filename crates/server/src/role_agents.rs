//! A role's agents as the store keeps them (docs/role-agents.md): one row a seat of a role, with the one road
//! its runs go out on, read at every use so a change the owner makes holds from the next admission.

use sea_orm::{ConnectionTrait, QueryResult};
use serde::Serialize;
use store::{Fault, all, one, run};

use crate::order::Entry;

/// One agent as its row holds it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleAgent {
  pub name: String,
  pub role: String,
  /// Its road: the provider and model its runs go out on, both or neither. An agent naming neither follows
  /// the supervisor that asks it.
  pub provider: Option<String>,
  pub model: Option<String>,
  pub effort: Option<String>,
  /// `on`, `off`, or none for the provider's own.
  pub thinking: Option<String>,
  pub answer_tokens: Option<i64>,
  pub enabled: bool,
  pub position: i64,
  /// Whether its session's cache is kept warm between runs.
  pub keeps_cache: bool,
  /// Its compaction threshold, none where its provider's declared ratio stands.
  pub fold_ratio: Option<f64>,
  /// What it lays over its role's preset, each none where the role's own stands.
  pub prompt: Option<String>,
  pub bundles: Option<Vec<String>>,
  pub skills: Option<Vec<String>>,
  pub added_at: i64,
}

// A list a row keeps as JSON text, none where the row keeps none or it does not read.
fn list(row: &QueryResult, name: &str) -> Result<Option<Vec<String>>, Fault> {
  Ok(
    row
      .try_get::<Option<String>>("", name)?
      .and_then(|kept| serde_json::from_str(&kept).ok()),
  )
}

impl RoleAgent {
  pub fn of(row: &QueryResult) -> Result<Self, Fault> {
    let text = |name: &str| -> Result<Option<String>, Fault> {
      Ok(row.try_get::<Option<String>>("", name)?.filter(|held| !held.trim().is_empty()))
    };

    Ok(Self {
      name: row.try_get("", "name")?,
      role: row.try_get("", "role")?,
      provider: text("provider")?,
      model: text("model")?,
      effort: text("effort")?,
      thinking: text("thinking")?,
      answer_tokens: row.try_get("", "answer_tokens")?,
      enabled: row.try_get::<i64>("", "enabled")? != 0,
      position: row.try_get("", "position")?,
      keeps_cache: row.try_get::<i64>("", "keeps_cache")? != 0,
      fold_ratio: row.try_get("", "fold_ratio")?,
      prompt: text("prompt")?,
      bundles: list(row, "bundles")?,
      skills: list(row, "skills")?,
      added_at: row.try_get("", "added_at")?,
    })
  }

  /// The road its runs go out on, none where it follows the supervisor that asks it.
  pub fn entry(&self) -> Option<Entry> {
    Some(Entry {
      provider: self.provider.clone()?,
      model: self.model.clone()?,
      effort: self.effort.clone(),
      thinking: self.thinking.clone(),
      answer_tokens: self.answer_tokens,
    })
  }

  /// Whether its road is this provider and model.
  pub fn on(&self, provider: &str, model: &str) -> bool {
    self.provider.as_deref() == Some(provider) && self.model.as_deref() == Some(model)
  }
}

/// Every agent the store holds, by role and then by its place among its role's.
pub async fn every<C: ConnectionTrait>(db: &C) -> Result<Vec<RoleAgent>, Fault> {
  all(db, r#"SELECT * FROM "role_agents" ORDER BY "role", "position", "name""#, vec![])
    .await?
    .iter()
    .map(RoleAgent::of)
    .collect()
}

/// A role's agents, in the order a run looks for a free one.
pub async fn of_role<C: ConnectionTrait>(db: &C, role: &str) -> Result<Vec<RoleAgent>, Fault> {
  all(
    db,
    r#"SELECT * FROM "role_agents" WHERE "role" = $1 ORDER BY "position", "name""#,
    vec![role.into()],
  )
  .await?
  .iter()
  .map(RoleAgent::of)
  .collect()
}

/// The agent of this name, where the store holds one.
pub async fn named<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<RoleAgent>, Fault> {
  one(db, r#"SELECT * FROM "role_agents" WHERE "name" = $1"#, vec![name.into()])
    .await?
    .as_ref()
    .map(RoleAgent::of)
    .transpose()
}

/// The name an agent added with none is given: its role and the first number no agent of the role holds.
pub async fn next_name<C: ConnectionTrait>(db: &C, role: &str) -> Result<String, Fault> {
  let held: Vec<String> = every(db).await?.into_iter().map(|agent| agent.name).collect();

  Ok(
    (1..)
      .map(|number| format!("{role}-{number:02}"))
      .find(|name| !held.contains(name))
      .unwrap_or_else(|| format!("{role}-01")),
  )
}

// A list as a row keeps it, none where the agent sets none.
fn kept(list: &Option<Vec<String>>) -> Option<String> {
  list
    .as_ref()
    .map(|list| serde_json::to_string(list).unwrap_or_else(|_| "[]".to_owned()))
}

/// The agent written whole: added where the store holds none of its name, and every field replaced where it
/// holds one.
pub async fn write<C: ConnectionTrait>(db: &C, agent: &RoleAgent) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "role_agents"
         ("name", "role", "provider", "model", "effort", "thinking", "answer_tokens", "enabled", "position",
          "keeps_cache", "fold_ratio", "prompt", "bundles", "skills", "added_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
       ON CONFLICT ("name") DO UPDATE SET
         "role" = excluded."role", "provider" = excluded."provider", "model" = excluded."model",
         "effort" = excluded."effort", "thinking" = excluded."thinking",
         "answer_tokens" = excluded."answer_tokens", "enabled" = excluded."enabled",
         "position" = excluded."position", "keeps_cache" = excluded."keeps_cache",
         "fold_ratio" = excluded."fold_ratio", "prompt" = excluded."prompt", "bundles" = excluded."bundles",
         "skills" = excluded."skills""#,
    vec![
      agent.name.clone().into(),
      agent.role.clone().into(),
      agent.provider.clone().into(),
      agent.model.clone().into(),
      agent.effort.clone().into(),
      agent.thinking.clone().into(),
      agent.answer_tokens.into(),
      i64::from(agent.enabled).into(),
      agent.position.into(),
      i64::from(agent.keeps_cache).into(),
      agent.fold_ratio.into(),
      agent.prompt.clone().into(),
      kept(&agent.bundles).into(),
      kept(&agent.skills).into(),
      agent.added_at.into(),
    ],
  )
  .await?;

  Ok(())
}

/// The agent removed; whether the store held it.
pub async fn remove<C: ConnectionTrait>(db: &C, name: &str) -> Result<bool, Fault> {
  Ok(run(db, r#"DELETE FROM "role_agents" WHERE "name" = $1"#, vec![name.into()]).await? > 0)
}

/// Every agent, of any role, a slot request stands let in on now.
pub async fn carrying_any<C: ConnectionTrait>(db: &C) -> Result<Vec<String>, Fault> {
  all(
    db,
    r#"SELECT "seat" FROM "role_slot_requests" WHERE "state" = 'active' AND "seat" IS NOT NULL"#,
    vec![],
  )
  .await?
  .iter()
  .map(|row| row.try_get::<String>("", "seat").map_err(Fault::from))
  .collect()
}

/// The agents a slot request stands let in on now, by name: the seats that carry a run.
pub async fn carrying<C: ConnectionTrait>(db: &C, role: &str) -> Result<Vec<String>, Fault> {
  all(
    db,
    r#"SELECT "seat" FROM "role_slot_requests"
       WHERE "role" = $1 AND "state" = 'active' AND "seat" IS NOT NULL"#,
    vec![role.into()],
  )
  .await?
  .iter()
  .map(|row| row.try_get::<String>("", "seat").map_err(Fault::from))
  .collect()
}
