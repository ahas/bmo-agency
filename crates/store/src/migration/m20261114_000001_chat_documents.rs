//! A supervisor's own documents the owner keeps a text of (docs/agent-documents.md, 「A supervisor of the window shape」): the
//! opening, the absolute rules and the manual, each standing in place of its file where the store holds one.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261114_000001_chat_documents";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("chat_documents.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
