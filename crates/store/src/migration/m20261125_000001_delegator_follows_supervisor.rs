//! The supervisor's double follows its supervisor: the delegator role's order is empty where it still holds the
//! order the roles policy gave it, so its runs go out on the asking supervisor's road; an order the owner set
//! on the role stands (docs/role-slots.md, 「A delegated task」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261125_000001_delegator_follows_supervisor";

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
      .execute_unprepared(include_str!("delegator_follows_supervisor.sql"))
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
