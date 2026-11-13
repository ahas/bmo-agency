//! A job keeps whether the notice of its end is owed yet, and the words its due review check is told with and
//! when it came due, so agency's sender owes a notice a stop cut off before its row was written
//! (docs/agent-control.md, 「An item's change is told to its orderer」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261220_000001_job_notices_owed";

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
      .execute_unprepared(include_str!("job_notices_owed.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
