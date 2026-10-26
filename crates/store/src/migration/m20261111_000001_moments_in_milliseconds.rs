//! Every run log line's moment and every ledger run's start and finish held in epoch milliseconds, a moment
//! kept in epoch seconds given its milliseconds (docs/run-records.md).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261111_000001_moments_in_milliseconds";

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
      .execute_unprepared(include_str!("moments_in_milliseconds.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
