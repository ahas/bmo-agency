//! An ask on a run's clock has no moment it is due (docs/agent-control.md, 「A clock asks, it never cuts」):
//! it stands until the supervisor's verdict or the run's end.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261120_000001_clock_asks_unbounded";

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
      .execute_unprepared(include_str!("clock_asks_unbounded.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
