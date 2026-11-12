//! A job carries whether it stands in the machine's turn (docs/role-agents.md, 「When an agent is busy」): testing,
//! waiting to test, deploying or waiting for its deployment, as the last reading of the testbed's lanes and of its
//! item's close found it; such a job leaves its agent free for another job.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261216_000001_job_machine_turn";

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
      .execute_unprepared(include_str!("job_machine_turn.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
