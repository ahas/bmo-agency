//! The settings the agency declares, as the service in front resolved them per person and unit and wrote
//! them whole, which the agency reads in place of any delegation chain of its own (docs/api.md, 「Settings the agency
//! declares」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270127_000002_subject_settings";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("subject_settings.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
