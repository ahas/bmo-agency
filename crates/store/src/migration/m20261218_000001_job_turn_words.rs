//! A job keeps the words of the turn it carries until that turn comes back from its session (docs/api.md,
//! 「assignments and jobs」), so a server that starts launches a job whose turn had not gone out again from them.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261218_000001_job_turn_words";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("job_turn_words.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
