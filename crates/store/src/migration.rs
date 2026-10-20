//! The migrations that make the store's tables (docs/store.md, 「Its tables」). The record of what ran
//! is a table of the store's own, so the store may share a database with another service's tables
//! without either's record standing in the other's way.

mod m20261016_000001_store;
mod m20261017_000001_clocks;
mod m20261019_000001_runs;
mod m20261020_000001_worker_context;
mod m20261021_000001_worker_words;
mod m20261022_000001_usage_windows;
mod m20261023_000001_work;
mod m20261024_000001_job_model;
mod m20261025_000001_base_prompt_seeds;
mod m20261025_000002_endpoint_accounts;

use sea_orm_migration::prelude::*;

/// The table the store's migrations keep the record of what ran in.
pub const MIGRATIONS_TABLE: &str = "agency_migrations";

/// The migration that makes every table; a seed given once is given as it runs (`crate::seed`).
pub const TABLES: &str = m20261016_000001_store::NAME;

/// Every table the store's migrations make, beside the record of what ran: what reads every table of the
/// store reads these and no other service's beside them (docs/store.md, 「Its tables」).
pub const OWN_TABLES: [&str; 59] = [
  "agents",
  "agent_log",
  "agent_executions",
  "execution_attempts",
  "agent_base_prompts",
  "base_prompts",
  "base_prompt_seeded",
  "branch_agents",
  "worker_sessions",
  "worker_words",
  "session_resets",
  "claude_accounts",
  "usage_windows",
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
  "live_claude_openings",
  "live_claude_runs",
  "prefix_moves",
  "run_records",
  "runs",
  "run_clocks",
  "clock_asks",
  "room_sessions",
  "room_session_turns",
  "companion_settings",
  "settings",
  "assignments",
  "jobs",
  "session_work",
  "session_loads",
  "session_summaries",
];

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
      Box::new(m20261016_000001_store::Migration),
      Box::new(m20261017_000001_clocks::Migration),
      Box::new(m20261019_000001_runs::Migration),
      Box::new(m20261020_000001_worker_context::Migration),
      Box::new(m20261021_000001_worker_words::Migration),
      Box::new(m20261022_000001_usage_windows::Migration),
      Box::new(m20261023_000001_work::Migration),
      Box::new(m20261024_000001_job_model::Migration),
      Box::new(m20261025_000001_base_prompt_seeds::Migration),
      Box::new(m20261025_000002_endpoint_accounts::Migration),
    ]
  }

  fn migration_table_name() -> DynIden {
    Alias::new(MIGRATIONS_TABLE).into_iden()
  }
}
