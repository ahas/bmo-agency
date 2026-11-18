//! The support count `auto` (docs/worker-slots.md, 「The base checkout and the support checkouts」): the support
//! checkouts such a repository has made recorded each by its slot (docs/store.md, 「Its tables」), each one a
//! repository's count stood for recorded; and the count nullable, null standing for `auto`, every repository's
//! default and every repository set to it.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270101_000001_support_auto";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("support_auto.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
