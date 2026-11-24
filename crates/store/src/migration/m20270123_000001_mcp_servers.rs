//! The MCP servers agency put at home for an account, one row a server, so what a record withholds is known without
//! the tool host (docs/person-nodes.md, 「MCP servers」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270123_000001_mcp_servers";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("mcp_servers.sql")).await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
