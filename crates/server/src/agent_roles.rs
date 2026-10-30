//! The roles an agent may hold as the store keeps them (docs/agent-roles.md, 「The roles」): a role's label,
//! bundles, bound and prompt are read off its row at every use, so a change the owner makes holds from the
//! next read with no restart; and the places a bounded role's turns take.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use sea_orm::{ConnectionTrait, QueryResult};
use store::{Fault, all, one};
use tokio::sync::{AcquireError, OwnedSemaphorePermit, Semaphore};

/// One role as its row holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRole {
  pub name: String,
  pub label: String,
  /// The bundles it gives, each one the policy declares.
  pub bundles: Vec<String>,
  /// The most of its agents that run a turn at once, where it is bounded.
  pub max_concurrent: Option<i64>,
  /// The prompt its agents are given before their own, where it carries one.
  pub prompt: Option<String>,
}

impl AgentRole {
  pub fn of(row: &QueryResult) -> Result<Self, Fault> {
    let bundles: String = row.try_get("", "bundles")?;

    Ok(Self {
      name: row.try_get("", "name")?,
      label: row.try_get("", "label")?,
      bundles: serde_json::from_str(&bundles).map_err(|error| Fault::new(error.to_string()))?,
      max_concurrent: row.try_get("", "max_concurrent")?,
      prompt: row.try_get("", "prompt")?,
    })
  }
}

/// Every role the store holds, in the order a screen lists them.
pub async fn every<C: ConnectionTrait>(db: &C) -> Result<Vec<AgentRole>, Fault> {
  all(db, r#"SELECT * FROM "agent_roles" ORDER BY "position", "name""#, vec![])
    .await?
    .iter()
    .map(AgentRole::of)
    .collect()
}

/// The role of this name, where the store holds one.
pub async fn named<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<AgentRole>, Fault> {
  one(db, r#"SELECT * FROM "agent_roles" WHERE "name" = $1"#, vec![name.into()])
    .await?
    .as_ref()
    .map(AgentRole::of)
    .transpose()
}

/// Whether the store holds a role of this name.
pub async fn holds<C: ConnectionTrait>(db: &C, name: &str) -> Result<bool, Fault> {
  Ok(named(db, name).await?.is_some())
}

/// Every role's label, in the order a screen lists them, as a refusal lists them.
pub async fn labels<C: ConnectionTrait>(db: &C) -> Result<Vec<String>, Fault> {
  Ok(every(db).await?.into_iter().map(|role| role.label).collect())
}

/// The bundles a role gives; none for a role the store does not hold.
pub async fn bundles_of<C: ConnectionTrait>(db: &C, name: &str) -> Result<Vec<String>, Fault> {
  Ok(named(db, name).await?.map(|role| role.bundles).unwrap_or_default())
}

/// The prompt a role's agents are given before their own; none for a role carrying none or one the store
/// does not hold.
pub async fn prompt_of<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<String>, Fault> {
  Ok(
    named(db, name)
      .await?
      .and_then(|role| role.prompt)
      .filter(|prompt| !prompt.trim().is_empty()),
  )
}

/// The bound of the role an agent holds, where its row names a role the store bounds.
pub async fn bound_of_agent<C: ConnectionTrait>(
  db: &C,
  agent: &str,
) -> Result<Option<(String, usize)>, Fault> {
  let row = one(
    db,
    r#"SELECT "agent_roles"."name", "agent_roles"."max_concurrent" FROM "agents"
       JOIN "agent_roles" ON "agent_roles"."name" = "agents"."role" WHERE "agents"."name" = $1"#,
    vec![agent.into()],
  )
  .await?;
  let Some(row) = row else {
    return Ok(None);
  };
  let Some(bound) = row.try_get::<Option<i64>>("", "max_concurrent")? else {
    return Ok(None);
  };

  Ok(Some((row.try_get("", "name")?, bound.max(1) as usize)))
}

/// The places the turns of each bounded role take: one semaphore a role, made for the bound its row held
/// when it was made.
#[derive(Default)]
pub struct RolePlaces(Mutex<BTreeMap<String, (usize, Arc<Semaphore>)>>);

impl RolePlaces {
  /// A place for one turn of a role under its bound as read now. Where the bound changed since the role's
  /// places were made, they are made again for the new one: a turn already running keeps the place it holds
  /// under the old bound until it ends, and none is cut. A turn beyond the bound waits for a place.
  pub async fn take(&self, role: &str, bound: usize) -> Result<OwnedSemaphorePermit, AcquireError> {
    let places = {
      let mut held = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
      let entry = held.entry(role.to_owned()).or_insert_with(|| (bound, Arc::new(Semaphore::new(bound))));

      if entry.0 != bound {
        *entry = (bound, Arc::new(Semaphore::new(bound)));
      }

      entry.1.clone()
    };

    places.acquire_owned().await
  }
}
