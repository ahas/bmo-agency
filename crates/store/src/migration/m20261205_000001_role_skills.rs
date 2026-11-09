//! A role's skills (docs/store.md, 「Its tables」; docs/agent-roles.md, 「What a role row holds」): the names of
//! the skills its runs carry as a JSON list, none on every role the store holds, whose runs go on carrying
//! the whole plugin the release places for the role.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261205_000001_role_skills";

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
      .execute_unprepared(r#"ALTER TABLE "roles" ADD COLUMN "skills" text;"#)
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
