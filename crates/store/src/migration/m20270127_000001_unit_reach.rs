//! The units each account reaches, written whole by the service in front of the agency, which the agency
//! reads for a unit's reach in place of any organisation of its own (docs/api.md, 「Who a caller acts for」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270127_000001_unit_reach";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("unit_reach.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
