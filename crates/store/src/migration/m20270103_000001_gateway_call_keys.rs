//! Every account but the deployment's own holds a call key of the gateway's, the store keeping its digest alone, and
//! every request the gateway carries is recorded with its account, model, run and cost (docs/store.md, 「Its
//! tables」, docs/own-llm.md, 「The keys」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270103_000001_gateway_call_keys";

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
      .execute_unprepared(include_str!("gateway_call_keys.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
