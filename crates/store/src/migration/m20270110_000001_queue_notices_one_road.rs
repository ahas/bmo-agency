//! Every notice of the queue is a quiet instruction to the orderer, and an idle agent's notice stands in for the
//! launch notice and the idle queue's alarm (docs/agent-control.md, 「An idle agent is told to the orderers of the
//! items it could take」): a row still owed by the receiver's owner road goes, a launch notice told is withdrawn,
//! and the alarm's settings rows go.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270110_000001_queue_notices_one_road";

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
      .execute_unprepared(include_str!("queue_notices_one_road.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
