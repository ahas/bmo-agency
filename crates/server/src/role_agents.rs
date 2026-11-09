//! A role's agents as the store keeps them (docs/role-agents.md): one row a seat of a role, with the one road
//! its runs go out on, read at every use so a change the owner makes holds from the next admission.

use sea_orm::{ConnectionTrait, QueryResult};
use serde::Serialize;
use store::{Fault, all, one, run};

use policy::roles::Sessions;

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
  /// Whether its runs keep their sessions (docs/agent-roles.md, 「A role's sessions」).
  #[serde(serialize_with = "sessions_word")]
  pub sessions: Sessions,
  /// Whether a run of another role may call it as a co-worker.
  pub callable: bool,
  /// Its compaction threshold, none where its role's stands.
  pub fold_ratio: Option<f64>,
  /// What it lays over its role's preset, each none where the role's own stands.
  pub prompt: Option<String>,
  pub bundles: Option<Vec<String>>,
  pub skills: Option<Vec<String>>,
  pub added_at: i64,
}

// The sessions setting as the routes write it.
fn sessions_word<S: serde::Serializer>(sessions: &Sessions, serializer: S) -> Result<S::Ok, S::Error> {
  serializer.serialize_str(sessions.word())
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
      sessions: Sessions::of(&row.try_get::<String>("", "sessions")?).unwrap_or_default(),
      callable: row.try_get::<i64>("", "callable")? != 0,
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
          "keeps_cache", "fold_ratio", "prompt", "bundles", "skills", "added_at", "sessions", "callable")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
       ON CONFLICT ("name") DO UPDATE SET
         "role" = excluded."role", "provider" = excluded."provider", "model" = excluded."model",
         "effort" = excluded."effort", "thinking" = excluded."thinking",
         "answer_tokens" = excluded."answer_tokens", "enabled" = excluded."enabled",
         "position" = excluded."position", "keeps_cache" = excluded."keeps_cache",
         "fold_ratio" = excluded."fold_ratio", "prompt" = excluded."prompt", "bundles" = excluded."bundles",
         "skills" = excluded."skills", "sessions" = excluded."sessions", "callable" = excluded."callable""#,
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
      agent.sessions.word().into(),
      i64::from(agent.callable).into(),
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

/// The agent a run is let in on (docs/role-agents.md, 「What a run reads off its agent」): the seat of the newest
/// slot request that stands active at the path the run works at, of the role named where one is; and, for a
/// run that names no path, the seat of its role's only active request. None where neither tells one, and the
/// run then stands on its role's preset alone.
pub async fn seated_at<C: ConnectionTrait>(
  db: &C,
  role: Option<&str>,
  path: Option<&str>,
) -> Result<Option<RoleAgent>, Fault> {
  // A composition made as an agent outside any run of it reads that agent, where it is of the role named.
  if let Ok(agent) = COMPOSED_AS.try_with(Clone::clone) {
    return Ok(named(db, &agent).await?.filter(|agent| role.is_none_or(|role| agent.role == role)));
  }

  let path = path.map(|path| path.trim().trim_end_matches('/')).filter(|path| !path.is_empty());
  let seat = match (path, role) {
    (Some(path), Some(role)) => one(
      db,
      r#"SELECT "seat" FROM "role_slot_requests"
         WHERE rtrim("path", '/') = $1 AND "role" = $2 AND "state" = 'active' AND "seat" IS NOT NULL
         ORDER BY "sequence" DESC LIMIT 1"#,
      vec![path.into(), role.into()],
    )
    .await?
    .map(|row| row.try_get::<String>("", "seat"))
    .transpose()?,
    (Some(path), None) => one(
      db,
      r#"SELECT "seat" FROM "role_slot_requests"
         WHERE rtrim("path", '/') = $1 AND "state" = 'active' AND "seat" IS NOT NULL
         ORDER BY "sequence" DESC LIMIT 1"#,
      vec![path.into()],
    )
    .await?
    .map(|row| row.try_get::<String>("", "seat"))
    .transpose()?,
    (None, Some(role)) => match carrying(db, role).await?.as_slice() {
      [only] => Some(only.clone()),
      _ => None,
    },
    (None, None) => None,
  };

  match seat {
    Some(seat) => named(db, &seat).await,
    None => Ok(None),
  }
}

/// The prompt a run of a role opens with: its agent's own where it lays one over its role's, else its role's;
/// none where neither carries one.
pub async fn prompt_at<C: ConnectionTrait>(
  db: &C,
  role: &str,
  path: Option<&str>,
) -> Result<Option<String>, Fault> {
  let own = seated_at(db, Some(role), path)
    .await?
    .and_then(|agent| agent.prompt)
    .filter(|prompt| !prompt.trim().is_empty());

  match own {
    Some(own) => Ok(Some(own)),
    None => crate::agent_roles::prompt_of(db, role).await,
  }
}

/// The bundles a run of a role is given: its agent's own where it lays them over its role's, else its role's.
pub async fn bundles_at<C: ConnectionTrait>(
  db: &C,
  role: &str,
  path: Option<&str>,
) -> Result<Vec<String>, Fault> {
  match seated_at(db, Some(role), path).await?.and_then(|agent| agent.bundles) {
    Some(own) => Ok(own),
    None => crate::agent_roles::bundles_of(db, role).await,
  }
}

tokio::task_local! {
  // The agent a composition is made as, where it is made outside any run of that agent.
  static COMPOSED_AS: String;
}

/// A piece of work done as an agent that holds no slot while it goes — the warm ping of its kept session
/// (docs/keepalive-schedule.md, 「An agent's kept session」): whatever it composes reads that agent as its
/// seat, so the command it goes out on is the one the agent's run went out on.
pub async fn composed_as<F: Future>(agent: String, work: F) -> F::Output {
  COMPOSED_AS.scope(agent, work).await
}

/// Whether a run works in a folder now: a slot request stands active at it.
pub async fn worked_in<C: ConnectionTrait>(db: &C, path: &str) -> Result<bool, Fault> {
  let path = path.trim().trim_end_matches('/');

  if path.is_empty() {
    return Ok(false);
  }

  Ok(
    one(
      db,
      r#"SELECT 1 AS "works" FROM "role_slot_requests"
         WHERE rtrim("path", '/') = $1 AND "state" = 'active' LIMIT 1"#,
      vec![path.into()],
    )
    .await?
    .is_some(),
  )
}

/// Whether a run of a role keeps its session (docs/agent-roles.md, 「A role's sessions」): the setting of the
/// agent it is let in on, by the folder it works in; a run whose agent cannot be told keeps none.
pub async fn sessions_at<C: ConnectionTrait>(
  db: &C,
  role: &str,
  path: Option<&str>,
) -> Result<Sessions, Fault> {
  Ok(seated_at(db, Some(role), path).await?.map_or(Sessions::Fresh, |agent| agent.sessions))
}
