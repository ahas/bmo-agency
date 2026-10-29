//! When each registration's engine last answered (docs/own-llm.md, 「The registry」): the moment the health
//! probe last heard from it, so a screen shows when it last answered rather than only that it does not now.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261114_000002_registration_last_answered";

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
      .execute_unprepared(include_str!("registration_last_answered.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
