//! A seat request carries the title and the asking agent its asker names, and the agent its child runs on
//! (docs/api.md, 「pool, seat-requests, reservations and roles」), which a delegation is listed by.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261122_000002_delegation_titles";

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
      .execute_unprepared(include_str!("delegation_titles.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
