//! A job's lines put under the agent its item names (docs/run-records.md, 「Agent log」): the lines already kept
//! under the checkout a job ran in are moved to that agent's name.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261214_000001_job_lines_by_agent";

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
      .execute_unprepared(include_str!("job_lines_by_agent.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
