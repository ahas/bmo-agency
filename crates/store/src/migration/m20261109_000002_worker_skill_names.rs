//! The co-worker roles' preloaded skills under the names the skills repository's worker bundle gives them
//! (docs/launch-lines.md, the skills handed over as a plugin directory).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261109_000002_worker_skill_names";

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
      .execute_unprepared(include_str!("worker_skill_names.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
