//! The hash of the system prompt a supervisor's one session across rooms was last sent with
//! (docs/supervisors.md, 「A session in each room」): the column the next turn compares its composed prompt
//! against, so a prompt whose documents moved hands the session over rather than leaving it on the old text.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261202_000001_supervisor_system_hash";

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
      .execute_unprepared(include_str!("supervisor_system_hash.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
