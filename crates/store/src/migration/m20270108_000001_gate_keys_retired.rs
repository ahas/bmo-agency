//! The retired sanitiser and output-format gates let go: the gate kinds kept under their names and every
//! supervisor's switch of them (docs/api.md, 「gate kinds」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270108_000001_gate_keys_retired";

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
      .execute_unprepared(include_str!("gate_keys_retired.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
