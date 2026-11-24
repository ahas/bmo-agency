//! A registration may be switched off, and an address may be reached through its owner's PC at a port of this
//! machine's loopback (docs/own-llm.md, 「A person's local AI」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270111_000001_local_ai";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("local_ai.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
