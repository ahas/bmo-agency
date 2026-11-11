//! Where a job's review stands (docs/job-review.md): its stages, the stage, round and check under way, its
//! rejections, the owner's question its work waits on, whether it gave its checkout up meanwhile, and the words
//! its orderer is to examine now.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261210_000001_job_review";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("job_review.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
