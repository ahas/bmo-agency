//! A road names the advisor its program may consult, none where it consults none (docs/provider-order.md,
//! 「What a road is」): a supervisor's road and an agent's road each keep it beside their model.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270113_000001_road_advisor";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("road_advisor.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
