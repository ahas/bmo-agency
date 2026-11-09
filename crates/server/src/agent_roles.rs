//! The roles as the store keeps them (docs/agent-roles.md, 「What a role row holds」): every field of a role's
//! row is read off it at every use, so a change the owner makes holds from the next read with no restart.

use policy::roles::{ROLES, Sessions};
use sea_orm::{ConnectionTrait, QueryResult};
use store::{Fault, all, one};

use crate::order::{Entry, Order};
use crate::role_agents::{self, RoleAgent};

/// One role as its row holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentRole {
  pub name: String,
  pub label: String,
  /// The bundles it gives, each one the policy declares.
  pub bundles: Vec<String>,
  /// The skills its runs carry on the worker line, where it names them; none leaves its runs the whole plugin
  /// the release places for it.
  pub skills: Option<Vec<String>>,
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
  /// Its agents, in the order a run looks for a free one (docs/role-agents.md).
  pub agents: Vec<RoleAgent>,
}

impl AgentRole {
  pub fn of(row: &QueryResult) -> Result<Self, Fault> {
    let bundles: String = row.try_get("", "bundles")?;
    let skills: Option<String> = row.try_get("", "skills")?;
    let order: String = row.try_get("", "provider_order")?;

    Ok(Self {
      name: row.try_get("", "name")?,
      label: row.try_get("", "label")?,
      bundles: serde_json::from_str(&bundles).map_err(|error| Fault::new(error.to_string()))?,
      skills: skills
        .map(|held| serde_json::from_str(&held).map_err(|error| Fault::new(error.to_string())))
        .transpose()?,
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
      agents: vec![],
    })
  }

  /// The role with its agents: how many of its runs go at once is the number of them switched on, and the
  /// roads its runs go out on are theirs, each once, in the order a run looks for a free agent — the agents
  /// that carry no run first, `carrying` naming the ones that do, so a run reading the role's roads finds
  /// the first free agent's. A role whose agents name no road follows the supervisor that asks it.
  pub fn seated(self, agents: Vec<RoleAgent>, carrying: &[String]) -> Self {
    let on: Vec<&RoleAgent> = agents.iter().filter(|agent| agent.enabled).collect();
    let (free, busy): (Vec<&RoleAgent>, Vec<&RoleAgent>) =
      on.iter().copied().partition(|agent| !carrying.contains(&agent.name));
    let mut entries: Vec<Entry> = vec![];

    for entry in free.iter().chain(busy.iter()).filter_map(|agent| agent.entry()) {
      if !entries.iter().any(|held| held.provider == entry.provider && held.model == entry.model) {
        entries.push(entry);
      }
    }

    Self {
      max_concurrent: on.len() as i64,
      order: Order {
        entries,
        forced: None,
        ..self.order
      },
      agents,
      ..self
    }
  }

  /// Whether a run of another role may call it: an agent of it is switched on and marked callable.
  pub fn called(&self) -> bool {
    self.agents.iter().any(|agent| agent.enabled && agent.callable)
  }

  /// The road a native call of it goes out on from a program standing on `provider`: that of its first
  /// callable agent, by position, on that provider.
  pub fn called_on(&self, provider: &str) -> Option<Entry> {
    self
      .agents
      .iter()
      .filter(|agent| agent.enabled && agent.callable)
      .filter_map(RoleAgent::entry)
      .find(|entry| entry.provider == provider)
  }

  /// The agent of this name among the role's, where it holds one.
  pub fn agent(&self, name: &str) -> Option<&RoleAgent> {
    self.agents.iter().find(|agent| agent.name == name)
  }
}

/// Every role the store holds, in the order a screen lists them.
pub async fn every<C: ConnectionTrait>(db: &C) -> Result<Vec<AgentRole>, Fault> {
  let mut agents = role_agents::every(db).await?;
  let carrying = role_agents::carrying_any(db).await?;
  let roles: Vec<AgentRole> = all(db, r#"SELECT * FROM "roles" ORDER BY "position", "name""#, vec![])
    .await?
    .iter()
    .map(AgentRole::of)
    .collect::<Result<_, _>>()?;

  Ok(
    roles
      .into_iter()
      .map(|role| {
        let (own, rest): (Vec<RoleAgent>, Vec<RoleAgent>) =
          std::mem::take(&mut agents).into_iter().partition(|agent| agent.role == role.name);

        agents = rest;
        role.seated(own, &carrying)
      })
      .collect(),
  )
}

/// The role of this name, where the store holds one.
pub async fn named<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<AgentRole>, Fault> {
  let Some(role) = one(db, r#"SELECT * FROM "roles" WHERE "name" = $1"#, vec![name.into()])
    .await?
    .as_ref()
    .map(AgentRole::of)
    .transpose()?
  else {
    return Ok(None);
  };

  let carrying = role_agents::carrying(db, name).await?;

  Ok(Some(role.seated(role_agents::of_role(db, name).await?, &carrying)))
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

/// The roles a run may call as co-workers, in the order a screen lists them: those with an agent that is
/// switched on and marked callable (docs/agent-roles.md, 「Co-workers」).
pub async fn callable<C: ConnectionTrait>(db: &C) -> Result<Vec<AgentRole>, Fault> {
  Ok(every(db).await?.into_iter().filter(AgentRole::called).collect())
}

/// The sentence an ask naming a role the store does not hold is refused with.
pub fn unknown(name: &str) -> String {
  format!("No role named {name} is held; a run names one of the store's roles.")
}

/// The role a job runs as where its item names none.
pub fn job_role() -> &'static str {
  &ROLES.job
}
