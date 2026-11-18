//! A person's run's platform reservations, and the supply outbox of the asks owed to the supply hook (docs/store.md,
//! 「Its tables」, docs/supply.md, 「Reservations」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20270106_000001_supply_reservations";

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
      .execute_unprepared(include_str!("supply_reservations.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
