//! The endpoint accounts and an account's own base prompt (docs/store.md, 「Its tables」): an account of
//! the Claude Code road may be reached at an Anthropic-compatible endpoint, by its base URL, its key file
//! and its map from the family words to the endpoint's model ids, and may name a kept base prompt, which
//! cannot then be removed from under it.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261025_000002_endpoint_accounts";

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
      .execute_unprepared(include_str!("endpoint_accounts.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
