//! A role says whether its runs keep their sessions, and a run apart whose role keeps them leaves its
//! session for the next run of the same command (docs/agent-roles.md, 「A role's sessions」; docs/sessions.md,
//! 「A run apart's kept session」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261122_000001_role_sessions";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("role_sessions.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
