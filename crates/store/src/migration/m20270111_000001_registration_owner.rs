//! A registration names the account it belongs to, none for the platform's (docs/own-llm.md, 「What a
//! registration holds」, docs/supply.md, 「Owned supply」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270111_000001_registration_owner";

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
      .execute_unprepared(include_str!("registration_owner.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
