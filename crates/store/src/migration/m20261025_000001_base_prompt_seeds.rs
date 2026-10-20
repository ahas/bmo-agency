//! The base prompts a store has been given (docs/base-prompts.md, 「The seeds」): each seed's name, kept
//! once it is given, so a seed the owner removed is not given again.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261025_000001_base_prompt_seeds";

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
      .execute_unprepared(include_str!("base_prompt_seeds.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
