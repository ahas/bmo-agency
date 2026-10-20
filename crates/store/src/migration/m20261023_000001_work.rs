//! The work a harness hands the agency (docs/api.md, 「Assignments and jobs」): a worker held for an item, the
//! item's run, and what a worker's session carries of the item, its documents and its summary.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261023_000001_work";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("work.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
