//! The one agent a slot request asks to be let in on and the agent a job's item names (docs/store.md, 「Its
//! tables」; docs/role-agents.md, 「Which agent a run takes」; docs/repositories.md, 「An agent an item names」):
//! none on every row that stands, which asked for no agent by name.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261206_000001_agent_wanted";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    let db = manager.get_connection();

    db.execute_unprepared(r#"ALTER TABLE "role_slot_requests" ADD COLUMN "agent_wanted" text;"#)
      .await?;
    db.execute_unprepared(r#"ALTER TABLE "jobs" ADD COLUMN "item_agent" text;"#).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
