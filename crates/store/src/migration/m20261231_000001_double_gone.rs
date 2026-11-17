//! The supervisor's double gone (docs/agent-roles.md, 「The roles the policy seeds」; docs/role-slots.md, 「Who
//! takes a slot」): every slot asked for a double or of its role that still stands finished as given back by the
//! owner, the `doppelganger` role taken away with its agents, reservations, sessions and provider prompts, and
//! no slot request kept as asked short (docs/store.md, 「Its tables」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261231_000001_double_gone";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("double_gone.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
