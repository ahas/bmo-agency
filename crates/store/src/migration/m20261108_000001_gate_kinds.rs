//! The gate kinds the gate declares (docs/api.md, 「Gate kinds」): the order it publishes, kept so a
//! console's agent switches and a gate's reasoning read it without agency holding a copy.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261108_000001_gate_kinds";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("gate_kinds.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
