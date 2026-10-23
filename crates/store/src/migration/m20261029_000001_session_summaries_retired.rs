//! No session summary is kept anywhere (docs/sessions.md, 「Session replacement」): the table an agent's
//! summary was kept in goes, and so do the notes a closed worker session left for the next run.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261029_000001_session_summaries_retired";

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
      .execute_unprepared(include_str!("session_summaries_retired.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
