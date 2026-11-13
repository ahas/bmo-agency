//! A running job's words are kept until a turn of it takes them (docs/agent-control.md, 「Words for a running
//! job」): every session a turn of a job starts is recorded against the job, every word for it waits in a row of
//! its own, and the job keeps the newest of its item's notes its prompt carried.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261222_000001_job_words";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("job_words.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
