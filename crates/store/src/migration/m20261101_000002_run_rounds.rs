//! A run of the ledger knows its round and the message its answer went out as (docs/store.md, 「Its
//! tables」, docs/run-records.md, 「The run ledger」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261101_000002_run_rounds";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("run_rounds.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
