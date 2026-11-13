//! A registration names the languages a run on it writes a result for a person in, none for every language
//! (docs/own-llm.md, 「What a registration holds」): none for every registration that stands.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261219_000001_registration_languages";

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
      .execute_unprepared(include_str!("registration_languages.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
