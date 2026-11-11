//! The co-worker roles and their marks gone (docs/agent-roles.md, 「The roles the policy seeds」): the
//! verifier, the scout, the screener and the builder taken away with their agents and sessions, no role or
//! agent marked as one another run may call, and the native call's slot records dropped; and the text a
//! role's run is given beside its prompt by the provider its program stands on kept in a table of its own,
//! each given once by the role and the provider (「A role's provider prompts」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261212_000001_coworkers_gone";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("coworkers_gone.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
