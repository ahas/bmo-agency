//! A run of the ledger keeps its title and whether agency carries its turn (docs/store.md, 「Its tables」,
//! docs/run-records.md, 「The run ledger」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261230_000001_run_turns";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("run_turns.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
