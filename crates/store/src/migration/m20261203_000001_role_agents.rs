//! A role's agents (docs/store.md, 「Its tables」; docs/role-agents.md): one row a seat of a role, with the road
//! its runs go out on, and the agent a slot request was let in on, its `seat`. Every role the store holds is given its
//! agents: as many as its cap, each on the road the role's runs went out on, and the role a job runs as one more
//! on each road a repository pins.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261203_000001_role_agents";

/// The agents of every role that holds none, as many as the role's cap, each on the provider the owner forced
/// on the role where one stands and else on the first of its order whose program carries the role's line, as
/// the providers declare their lines at this migration; none of a road where its order holds no such entry.
pub const AGENTS_OF_ROLES: &str = include_str!("role_agents_of_roles.sql");

// The role a job runs as where its item names none, as the roles this store was seeded with name it.
const JOB_ROLE: &str = "worker";

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

    db.execute_unprepared(include_str!("role_agents.sql")).await?;
    db.execute_unprepared(AGENTS_OF_ROLES).await?;

    // A repository's pin names the road its jobs go out on, and a job is let in only on an agent of that road:
    // the role a job runs as where its item names none is given one agent on each pinned road it holds none on.
    db.execute(sea_orm::Statement::from_sql_and_values(
      db.get_database_backend(),
      include_str!("role_agents_of_pins.sql"),
      [JOB_ROLE.into()],
    ))
    .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Err(DbErr::Migration(
      "a migration of the store is not undone; its database is dropped instead".into(),
    ))
  }
}
