//! The fold ratio of each provider at each tier (docs/sessions.md, 「Session compaction」): the share of a
//! model's window a session may fill before it is folded, and the providers whose ratios were given.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261029_000002_fold_ratios";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("fold_ratios.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
