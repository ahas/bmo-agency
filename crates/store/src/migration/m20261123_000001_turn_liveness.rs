//! The life of a turn put into a program's pane, one row a turn (docs/agent-control.md, 「Telling that a
//! turn's program is gone」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261123_000001_turn_liveness";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("turn_liveness.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
