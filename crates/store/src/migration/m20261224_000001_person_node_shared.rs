//! Several accounts' records name one node only where the administrator declared it shared for each
//! (docs/person-nodes.md, 「Several accounts on one node」): a record carries whether it is.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261224_000001_person_node_shared";

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
      .execute_unprepared(include_str!("person_node_shared.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
