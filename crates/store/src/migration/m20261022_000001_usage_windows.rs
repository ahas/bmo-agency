//! The accounts' usage windows (docs/store.md, 「Its tables」): one row a window of a slot, each reading
//! replacing its slot's whole set (docs/usage-windows.md).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261022_000001_usage_windows";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("usage_windows.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
