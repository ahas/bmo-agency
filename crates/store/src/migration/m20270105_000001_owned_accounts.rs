//! A Claude account names the node a person's login stands on, and whether the platform lends it to another
//! person's run (docs/store.md, 「Its tables」, docs/supply.md, 「The lendable accounts」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270105_000001_owned_accounts";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("owned_accounts.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
