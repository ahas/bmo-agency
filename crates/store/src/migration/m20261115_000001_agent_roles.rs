//! The roles an agent may hold as the owner keeps them (docs/agent-roles.md, 「The roles」): one row a role,
//! with the names of the roles the policy's seed has given, so a role the owner removed stays removed.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261115_000001_agent_roles";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("agent_roles.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
