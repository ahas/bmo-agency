//! A supervisor's brain switch and tool limit beside its other settings (docs/supervisors.md, 「Its settings」):
//! a row that holds neither reads its brain as on and every tool as held.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261225_000001_supervisor_limits";

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
      .execute_unprepared(include_str!("supervisor_limits.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
