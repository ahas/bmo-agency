//! The run log's prune indexes (docs/run-records.md, 「Agent log」): partial on the lines no job wrote,
//! which are the only lines the prune weighs and ages out.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261113_000001_agent_log_prune";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("agent_log_prune.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
