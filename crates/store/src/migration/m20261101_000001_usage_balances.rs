//! The paid accounts' balances (docs/store.md, 「Its tables」): one row a slot's balance, each reading
//! replacing it (docs/usage-windows.md, 「Balances」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261101_000001_usage_balances";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("usage_balances.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
