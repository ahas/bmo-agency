//! The migrations that make the store's tables (docs/store.md, 「Its tables」). The record of what ran
//! is a table of the store's own, so the store may share a database with another service's tables
//! without either's record standing in the other's way.

mod m20261016_000001_store;

use sea_orm_migration::prelude::*;

/// The table the store's migrations keep the record of what ran in.
pub const MIGRATIONS_TABLE: &str = "agency_migrations";

/// The migration that makes every table; a seed given once is given as it runs (`crate::seed`).
pub const TABLES: &str = m20261016_000001_store::NAME;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261016_000001_store::Migration)]
  }

  fn migration_table_name() -> DynIden {
    Alias::new(MIGRATIONS_TABLE).into_iden()
  }
}
