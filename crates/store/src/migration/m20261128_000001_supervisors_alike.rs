//! Every supervisor of one kind (docs/supervisors.md, 「One runner」): the column marking one as answering the
//! rooms that name none goes, and the supervisor it marked becomes agency's research supervisor setting.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261128_000001_supervisors_alike";

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
      .execute_unprepared(include_str!("supervisors_alike.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
