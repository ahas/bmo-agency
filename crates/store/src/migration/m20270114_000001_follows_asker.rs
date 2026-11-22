//! A role and an agent each say whether a run of it follows the supervisor that asks it while it names a road of
//! its own, the road then standing where no supervisor's does (docs/role-agents.md, 「An agent that follows its
//! asker」): the role's setting is its agents' default and an agent's own stands over it. The transliterator,
//! whose runs the voice unit asks for a supervisor's narration, follows.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270114_000001_follows_asker";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("follows_asker.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
