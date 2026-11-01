//! The ask a waiting pool request's owner made last (docs/coworker-pool.md, 「Requests and leases」): every
//! ask numbered in one increasing order of the pool's own, so the line passes over a request whose owner
//! has not asked since the asker last did.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261118_000001_pool_asks";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("pool_asks.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
