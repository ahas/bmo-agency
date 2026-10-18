//! The context a worker's session record keeps of its last answered turn (docs/store.md, 「Its tables」):
//! the tokens it carried and the share of the window they filled, columns of agency's own beside the
//! record's archive shape.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261020_000001_worker_context";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("worker_context.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
