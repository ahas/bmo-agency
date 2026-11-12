//! Agency's outbox, the notices owed to an orderer or the owner (docs/agent-control.md, 「An item's change is told
//! to its orderer」): one row a notice, sent until the receiver takes it, standing while its condition holds.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261217_000001_notices";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("notices.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
