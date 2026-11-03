//! The roles under the names the owner gave them (docs/agent-roles.md, 「The roles the policy seeds」): the
//! supervisor's double is `doppelganger`, the search line's role `voyager`, the browse line's `seeker` and the
//! video role `producer`, and every row keyed on a role's name is keyed on its new one; `digest` and `helper`
//! go with the slots, reservations and sessions they held. A run's name or a request key holding an old name
//! as text stays as it was written.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261126_000001_role_names";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("role_names.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
