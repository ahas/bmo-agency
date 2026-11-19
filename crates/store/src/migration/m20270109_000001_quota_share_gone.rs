//! A road holds no share of its provider's quota back: a provider has quota while anything of it is left, so a
//! role's row keeps no share (docs/provider-order.md, 「Quota: what a provider has left」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270109_000001_quota_share_gone";

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
      .execute_unprepared(include_str!("quota_share_gone.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
