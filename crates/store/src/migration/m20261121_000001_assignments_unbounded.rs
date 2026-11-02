//! A worker's assignment has no moment it runs out (docs/api.md, 「assignments and jobs」): a hold stands
//! until its job starts and ends, or its harness releases it. An assignment in the state `expired` is
//! released.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261121_000001_assignments_unbounded";

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
      .execute_unprepared(include_str!("assignments_unbounded.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
