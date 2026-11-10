//! A role's agents and its slots belong to an account (docs/role-agents.md, 「What an agent is」; docs/role-slots.md,
//! 「Accounts」): the account whose seat an agent is, and the account a slot request belongs to, none for the
//! deployment's own.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261209_000001_account_seats";

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
        r#"ALTER TABLE "role_agents" ADD COLUMN IF NOT EXISTS "owner_account" text;
           ALTER TABLE "role_slot_requests" ADD COLUMN IF NOT EXISTS "account" text;"#,
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
