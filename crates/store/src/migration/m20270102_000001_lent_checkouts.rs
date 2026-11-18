//! Checkouts lent to a job (docs/worker-slots.md, 「A checkout lent to a job」, 「An umbrella repository」): the
//! umbrella mark on a repository row, the repositories a job's item named to be lent, and when a loan's
//! give-back began (docs/store.md, 「Its tables」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270102_000001_lent_checkouts";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("lent_checkouts.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
