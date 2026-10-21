//! The DeepSeek models move from the registry to the deepseek provider (docs/cache-fallback.md, 「The
//! DeepSeek road」): the pins, tier mappings, roads and kept sessions that named the registrations, and
//! the co-worker pool's limits on the new provider.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261027_000001_deepseek_provider";

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
      .execute_unprepared(include_str!("deepseek_provider.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
