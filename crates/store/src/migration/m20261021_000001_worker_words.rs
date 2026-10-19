//! The words said to a worker (docs/store.md, 「Its tables」): one row a word, kept until a turn has taken it,
//! in the shape the archive holds it.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261021_000001_worker_words";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("worker_words.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
