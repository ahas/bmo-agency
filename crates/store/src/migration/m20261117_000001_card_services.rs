//! The card service a registration's engine is and whether its instances keep role files, and the demands
//! agency holds on the arbiter's master (docs/own-llm.md, 「An engine the arbiter stands」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261117_000001_card_services";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("card_services.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
