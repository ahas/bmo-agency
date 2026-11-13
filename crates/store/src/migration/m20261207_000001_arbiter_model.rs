//! The word an operator's own arbiter lists a registration's model under (docs/own-llm.md, 「What a registration
//! holds」): none on every registration that stands, whose health stays the engine's own answer.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261207_000001_arbiter_model";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("arbiter_model.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
