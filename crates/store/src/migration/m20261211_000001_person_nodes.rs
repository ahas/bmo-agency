//! An account's node, where its supervisors' sessions and its role agents' runs stand (docs/person-nodes.md,
//! 「Which node is a person's」): one row an account.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261211_000001_person_nodes";

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
      .execute_unprepared(
        r#"CREATE TABLE IF NOT EXISTS "person_nodes" (
             "owner_account" text PRIMARY KEY,
             "node" text NOT NULL,
             "set_at" bigint NOT NULL
           );"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
