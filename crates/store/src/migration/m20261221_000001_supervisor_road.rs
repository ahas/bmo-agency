//! A supervisor stands on one road, set on its row: the first entry of the order it held. The marks of a stand-in
//! answering and of a supervisor's walk go with them (docs/provider-order.md, 「What a road is」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261221_000001_supervisor_road";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("supervisor_road.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
