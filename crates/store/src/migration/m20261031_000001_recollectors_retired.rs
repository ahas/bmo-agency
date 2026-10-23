//! No recollector stands (docs/agent-ownership.md, 「Who writes long-term memory」): the companions' column
//! naming a model for their recollectors goes, and so does every row kept under a recollector's name or its
//! work key.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261031_000001_recollectors_retired";

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
      .execute_unprepared(include_str!("recollectors_retired.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
