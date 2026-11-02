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
mod m20261026_000001_pool_session;
mod m20261027_000001_deepseek_provider;
mod m20261028_000001_mapping_thinking;
mod m20261029_000001_session_summaries_retired;
mod m20261029_000002_fold_ratios;
mod m20261030_000001_agent_owner;
mod m20261031_000001_recollectors_retired;
mod m20261101_000001_usage_balances;
mod m20261101_000002_run_rounds;
mod m20261102_000001_route_registration;
mod m20261103_000001_run_usage;
mod m20261104_000001_job_usage;
mod m20261105_000001_prefix_documents;
mod m20261106_000001_job_instruction;
mod m20261107_000001_run_tools;
mod m20261108_000001_gate_kinds;
mod m20261109_000001_job_roads;
mod m20261109_000002_worker_skill_names;
mod m20261110_000001_translator_runner;
mod m20261110_000002_pickers_retired;
mod m20261111_000001_moments_in_milliseconds;
mod m20261111_000002_picker_keys_retired;
mod m20261112_000001_role_files;
mod m20261113_000001_agent_log_prune;
mod m20261114_000001_chat_documents;
mod m20261114_000002_registration_last_answered;
mod m20261115_000001_agent_roles;
mod m20261116_000001_agent_log_run_id;
mod m20261117_000001_card_services;
mod m20261118_000001_pool_asks;
mod m20261119_000001_registration_wait_retired;
mod m20261120_000001_clock_asks_unbounded;
mod m20261121_000001_assignments_unbounded;
mod m20261122_000001_role_sessions;
mod m20261122_000002_delegation_titles;
mod m20261122_000003_job_close_lease;
mod m20261123_000001_turn_liveness;
mod m20261124_000001_renewal;
mod m20261124_000001_stream_events;
mod m20261125_000001_delegator_follows_supervisor;

use sea_orm_migration::prelude::*;

/// The table the store's migrations keep the record of what ran in.
pub const MIGRATIONS_TABLE: &str = "agency_migrations";

/// The migration that makes every table; a seed given once is given as it runs (`crate::seed`).
pub const TABLES: &str = m20261016_000001_store::NAME;

/// The migration that makes the supervisors, the roles and the repositories tables of their own and carries
/// the agent rows over (docs/agents.md, 「Two axes」): what writes rows in the shape before it, the archive's
/// import, runs the migrations up to it, writes, and then runs it.
pub const TWO_AXES: &str = m20261124_000001_renewal::NAME;

/// Every table the store's migrations make, beside the record of what ran: what reads every table of the
/// store reads these and no other service's beside them (docs/store.md, 「Its tables」).
pub const OWN_TABLES: [&str; 51] = [
  "supervisors",
  "roles",
  "role_seeded",
  "repositories",
  "repository_node_checkouts",
  "role_sessions",
  "role_slot_requests",
  "role_slot_reservations",
  "role_slot_native",
  "role_slot_native_reservations",
  "role_slot_native_stops",
  "place_writes",
  "gate_kinds",
  "chat_documents",
  "agent_log",
  "agent_executions",
  "execution_attempts",
  "agent_base_prompts",
  "base_prompts",
  "base_prompt_seeded",
  "branch_agents",
  "session_resets",
  "claude_accounts",
  "usage_windows",
  "usage_balances",
  "model_catalogue",
  "fallback_sessions",
  "model_fallbacks",
  "model_uses",
  "registered_models",
  "registered_model_addresses",
  "card_demands",
  "node_workplaces",
  "node_rotation",
  "node_assignments",
  "live_claude_openings",
  "live_claude_runs",
  "prefix_moves",
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
  "turn_liveness",
  "stream_events",
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
      Box::new(m20261026_000001_pool_session::Migration),
      Box::new(m20261027_000001_deepseek_provider::Migration),
      Box::new(m20261028_000001_mapping_thinking::Migration),
      Box::new(m20261029_000001_session_summaries_retired::Migration),
      Box::new(m20261029_000002_fold_ratios::Migration),
      Box::new(m20261030_000001_agent_owner::Migration),
      Box::new(m20261031_000001_recollectors_retired::Migration),
      Box::new(m20261101_000001_usage_balances::Migration),
      Box::new(m20261101_000002_run_rounds::Migration),
      Box::new(m20261102_000001_route_registration::Migration),
      Box::new(m20261103_000001_run_usage::Migration),
      Box::new(m20261104_000001_job_usage::Migration),
      Box::new(m20261105_000001_prefix_documents::Migration),
      Box::new(m20261106_000001_job_instruction::Migration),
      Box::new(m20261107_000001_run_tools::Migration),
      Box::new(m20261108_000001_gate_kinds::Migration),
      Box::new(m20261109_000001_job_roads::Migration),
      Box::new(m20261109_000002_worker_skill_names::Migration),
      Box::new(m20261110_000001_translator_runner::Migration),
      Box::new(m20261110_000002_pickers_retired::Migration),
      Box::new(m20261111_000001_moments_in_milliseconds::Migration),
      Box::new(m20261111_000002_picker_keys_retired::Migration),
      Box::new(m20261112_000001_role_files::Migration),
      Box::new(m20261113_000001_agent_log_prune::Migration),
      Box::new(m20261114_000001_chat_documents::Migration),
      Box::new(m20261114_000002_registration_last_answered::Migration),
      Box::new(m20261115_000001_agent_roles::Migration),
      Box::new(m20261116_000001_agent_log_run_id::Migration),
      Box::new(m20261117_000001_card_services::Migration),
      Box::new(m20261118_000001_pool_asks::Migration),
      Box::new(m20261119_000001_registration_wait_retired::Migration),
      Box::new(m20261120_000001_clock_asks_unbounded::Migration),
      Box::new(m20261121_000001_assignments_unbounded::Migration),
      Box::new(m20261122_000001_role_sessions::Migration),
      Box::new(m20261122_000002_delegation_titles::Migration),
      Box::new(m20261122_000003_job_close_lease::Migration),
      Box::new(m20261123_000001_turn_liveness::Migration),
      Box::new(m20261124_000001_stream_events::Migration),
      Box::new(m20261124_000001_renewal::Migration),
      Box::new(m20261125_000001_delegator_follows_supervisor::Migration),
    ]
  }

  fn migration_table_name() -> DynIden {
    Alias::new(MIGRATIONS_TABLE).into_iden()
  }
}
