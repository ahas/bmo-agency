//! The translator as a runner with a role of its own, and its work key `translate` started from the
//! translator's road and tier (docs/agent-roles.md, docs/launch-lines.md, 「Translate runner」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261110_000001_translator_runner";

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
      .execute_unprepared(include_str!("translator_runner.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
