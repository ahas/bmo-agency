//! What chooses where a run goes (docs/provider-order.md, 「What a road is」; docs/repositories.md,
//! 「A model an item names」): a role's and a supervisor's forced provider, a job's item model, and what chose a
//! job's road.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261129_000001_chosen_roads";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("chosen_roads.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
