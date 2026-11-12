//! A job's seat is bound to none of its turns (docs/role-slots.md, 「The line」): a job's lease still let in and
//! bound to the run name of one of its turns is unbound, so the line's pass no longer reads that turn's end as
//! the job's and the job keeps its agent until it ends.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261215_000001_job_seats_unbound";

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
      .execute_unprepared(include_str!("job_seats_unbound.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
