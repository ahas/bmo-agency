//! A role's agents in no order of priority (docs/role-agents.md, 「Which agent a run takes」): their place among
//! their role's dropped, since a job's agent is named by the supervisor that orders it and a run that names none
//! takes any free agent of its role; and the agents a seeded role is given written with no place.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261213_000001_agent_order_gone";

/// The agents a role is seeded with, as many as its declaration's count, each on the road it names
/// (docs/role-agents.md, 「The roles the policy seeds」), for a role that holds none yet.
pub const AGENTS_OF_ROLES: &str = include_str!("role_agents_of_roles_unordered.sql");

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(include_str!("agent_order_gone.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
