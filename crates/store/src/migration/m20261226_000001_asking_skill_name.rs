//! The roles' and their agents' preloaded asking skill under the name the worker bundle gives it, a job asking
//! its supervisor first (docs/launch-lines.md, the skills handed over as a plugin directory).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261226_000001_asking_skill_name";

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
      .execute_unprepared(include_str!("asking_skill_name.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
