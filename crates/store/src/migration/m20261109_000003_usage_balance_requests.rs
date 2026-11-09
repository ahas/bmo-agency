//! The request allowance a balance carries (docs/store.md, 「Its tables」): how many requests of the day are
//! used, the day's ceiling and how many are left, none where the provider states none
//! (docs/usage-windows.md, 「Balances」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261109_000003_usage_balance_requests";

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
      .execute_unprepared(include_str!("usage_balance_requests.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
