//! The session a pool request carries (docs/coworker-pool.md, 「A project's own session」): the session a
//! worker's own turn continues where the pool judged one it still holds warm and within its model's
//! context, or none where the turn opens a fresh one.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261026_000001_pool_session";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("pool_session.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
