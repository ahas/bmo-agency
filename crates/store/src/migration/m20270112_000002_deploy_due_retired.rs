//! The deploy queue's hourly flush retires each repository's due clock (docs/agent-control.md, 「The deploy queue
//! is flushed every hour」): its rows go, and a due notice still owed is let go, the copy told withdrawn.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270112_000002_deploy_due_retired";

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
      .execute_unprepared(include_str!("deploy_due_retired.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
