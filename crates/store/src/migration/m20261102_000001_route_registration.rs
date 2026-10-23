//! A work key's route row may name the registration the key runs on at the registry's provider
//! (docs/store.md, 「Its tables」, docs/own-llm.md, 「A key's own registration」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261102_000001_route_registration";

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
      .execute_unprepared(include_str!("route_registration.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
