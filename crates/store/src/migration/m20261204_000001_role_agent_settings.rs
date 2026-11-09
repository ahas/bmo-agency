//! An agent's own settings (docs/store.md, 「Its tables」; docs/role-agents.md, 「What an agent row holds」):
//! whether its runs keep their sessions and whether a run of another role may call it, beside the compaction
//! threshold it already held. Every agent the store holds is given what its role's row held of the three.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261204_000001_role_agent_settings";

/// Every agent given its role's row's sessions setting and callability, and its compaction threshold where
/// the agent sets none: what a role's declaration gives the agents it is seeded with.
pub const SETTINGS_OF_ROLES: &str = include_str!("role_agent_settings_of_roles.sql");

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    let db = manager.get_connection();

    db.execute_unprepared(include_str!("role_agent_settings.sql")).await?;
    db.execute_unprepared(SETTINGS_OF_ROLES).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
