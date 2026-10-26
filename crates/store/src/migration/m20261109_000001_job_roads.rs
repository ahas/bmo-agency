//! The roads a job's turns went out on and why each was left, and the reason a job waits where no road can
//! answer (docs/api.md, 「assignments and jobs」; docs/cache-fallback.md, 「One road resolution」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261109_000001_job_roads";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("job_roads.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
