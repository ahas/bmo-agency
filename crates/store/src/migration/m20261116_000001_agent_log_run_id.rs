//! The run log's index by run (docs/run-records.md, 「Agent log」): a page of one run's lines after an id is
//! read by the run alone, so the run stands first in the index.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261116_000001_agent_log_run_id";

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
      .execute_unprepared(include_str!("agent_log_run_id.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
