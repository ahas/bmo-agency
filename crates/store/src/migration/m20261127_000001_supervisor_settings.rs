//! Every supervisor's settings under the supervisors' name (docs/supervisors.md, 「Its settings」): the table
//! that holds them, its constraints and its row numbering are renamed, every row kept as it stood.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261127_000001_supervisor_settings";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(include_str!("supervisor_settings.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
