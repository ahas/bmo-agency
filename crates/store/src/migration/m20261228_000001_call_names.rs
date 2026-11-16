//! A supervisor's call names stand in one column, `aliases` (docs/supervisors.md, 「Its call names」): the
//! `wake_words` column no route wrote and no reader read goes.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261228_000001_call_names";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("call_names.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
