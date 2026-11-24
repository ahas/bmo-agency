//! Whether a reset of a session starts it clean (docs/store.md, 「Its tables」, docs/session-resets.md, 「What a
//! reset is」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270116_000001_session_reset_clean";

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
      .execute_unprepared(include_str!("session_reset_clean.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
