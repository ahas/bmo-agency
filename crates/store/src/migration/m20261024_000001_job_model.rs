//! The model each job's turn went out on (docs/api.md, 「Accounts, rest and usage」): a job's token spend
//! and the session it ran on are grouped by model as well as by worker, holder and room.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261024_000001_job_model";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("job_model.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
