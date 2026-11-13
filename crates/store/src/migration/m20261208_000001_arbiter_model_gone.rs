//! A registration reads no operator's own status: the column that named an operator's word for the model goes
//! (docs/own-llm.md, 「What a registration holds」). A registered model is a service registered by its settings, and
//! nothing of another operator's shape stands in this store.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261208_000001_arbiter_model_gone";

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
      .execute_unprepared(include_str!("arbiter_model_gone.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
