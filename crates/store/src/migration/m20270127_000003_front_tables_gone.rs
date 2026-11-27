//! The tables of the units each account reached and of the settings per subject, which the agency reads no more:
//! it asks the authority instead (docs/api.md, 「Calls agency makes」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270127_000003_front_tables_gone";

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
      .execute_unprepared(include_str!("front_tables_gone.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
