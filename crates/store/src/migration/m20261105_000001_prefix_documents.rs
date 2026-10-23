//! A worker names the documents its session's prefix carries (docs/worker-bench.md, 「Prefix documents」,
//! docs/store.md, 「Its tables」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261105_000001_prefix_documents";

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
      .execute_unprepared(include_str!("prefix_documents.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
