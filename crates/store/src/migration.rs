//! The migrations that make the store's tables (docs/store.md, 「Its tables」). The record of what ran
//! is a table of the store's own, so the store may share a database with another service's tables
//! without either's record standing in the other's way.

mod m20261016_000001_store;

use sea_orm_migration::prelude::*;

/// The table the store's migrations keep the record of what ran in.
pub const MIGRATIONS_TABLE: &str = "agency_migrations";

/// The migration that makes every table; a seed given once is given as it runs (`crate::seed`).
pub const TABLES: &str = m20261016_000001_store::NAME;

/// Every table the store's migrations make, beside the record of what ran: what reads every table of the
/// store reads these and no other service's beside them (docs/store.md, 「Its tables」).
pub const OWN_TABLES: [&str; 52] = [
  "agents",
  "agent_log",
  "agent_executions",
  "execution_attempts",
  "agent_base_prompts",
  "base_prompts",
  "base_prompt_changes",
  "branch_agents",
  "worker_sessions",
  "session_resets",
  "claude_accounts",
  "coworker_pool_limits",
  "coworker_pool_meta",
  "coworker_pool_native",
  "coworker_pool_native_reservations",
  "coworker_pool_native_stops",
  "coworker_pool_pings",
  "coworker_pool_place_writes",
  "coworker_pool_requests",
  "coworker_pool_reservation_meta",
  "coworker_pool_reservations",
  "coworker_pool_roles",
  "coworker_pool_tier_limits",
  "coworker_pool_tier_meta",
  "model_catalogue",
  "model_fallbacks",
  "fallback_routes",
  "fallback_sessions",
  "model_tier_assignments",
  "model_tier_backfilled",
  "model_tier_conflicts",
  "model_tier_legacy",
  "model_tier_mappings",
  "model_tier_meta",
  "model_tier_seeded",
  "model_uses",
  "registered_models",
  "registered_model_addresses",
  "node_workplaces",
  "node_rotation",
  "node_assignments",
  "node_hubs",
  "live_claude_openings",
  "live_claude_runs",
  "prefix_moves",
  "run_records",
  "room_sessions",
  "room_session_turns",
  "room_windows",
  "companion_settings",
  "companion_setting_changes",
  "settings",
];

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
