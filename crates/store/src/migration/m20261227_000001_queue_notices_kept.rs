//! An item awaiting a launch is told once for each stretch it waits, and an ended job's item left leased once for
//! each lease (docs/agent-control.md, 「An item's change is told to its orderer」): a row of either notice still
//! standing is kept instead, one already told due no more and one not yet told still sent, and a close notice's
//! row marked with the lease its ended job keeps.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261227_000001_queue_notices_kept";

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
      .execute_unprepared(include_str!("queue_notices_kept.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
