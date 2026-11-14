//! A commit waiting for its repository's batch deployment is told once for each commit and kept until the item
//! no longer waits (docs/agent-control.md, 「An item's change is told to its orderer」): a row of that notice
//! still standing is kept instead, one already told due no more and one not yet told still sent.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261223_000001_deploy_notices_kept";

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
      .execute_unprepared(include_str!("deploy_notices_kept.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
