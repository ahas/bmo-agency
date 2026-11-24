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
mod m20261109_000003_usage_balance_requests;
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
mod m20261126_000001_role_names;
mod m20261127_000001_supervisor_settings;
mod m20261128_000001_supervisors_alike;
mod m20261129_000001_chosen_roads;
mod m20261130_000001_supervisor_card_rank;
mod m20261201_000001_relays;
mod m20261202_000001_supervisor_system_hash;
pub mod m20261203_000001_role_agents;
pub mod m20261204_000001_role_agent_settings;
mod m20261205_000001_role_skills;
mod m20261206_000001_agent_wanted;
mod m20261207_000001_arbiter_model;
mod m20261208_000001_arbiter_model_gone;
mod m20261209_000001_account_seats;
mod m20261210_000001_job_review;
mod m20261211_000001_person_nodes;
mod m20261212_000001_coworkers_gone;
pub mod m20261213_000001_agent_order_gone;
mod m20261214_000001_job_lines_by_agent;
mod m20261215_000001_job_seats_unbound;
mod m20261216_000001_job_machine_turn;
mod m20261217_000001_notices;
mod m20261218_000001_job_turn_words;
mod m20261219_000001_registration_languages;
mod m20261220_000001_job_notices_owed;
mod m20261221_000001_supervisor_road;
mod m20261222_000001_job_words;
mod m20261223_000001_deploy_notices_kept;
mod m20261224_000001_person_node_shared;
mod m20261225_000001_supervisor_limits;
mod m20261226_000001_asking_skill_name;
mod m20261227_000001_queue_notices_kept;
mod m20261228_000001_call_names;
mod m20261229_000001_short_slots;
mod m20261230_000001_run_turns;
mod m20261231_000001_double_gone;
mod m20270101_000001_support_auto;
mod m20270102_000001_lent_checkouts;
mod m20270103_000001_gateway_call_keys;
mod m20270104_000001_personal_supply;
mod m20270105_000001_owned_accounts;
mod m20270106_000001_supply_reservations;
mod m20270107_000001_run_cost;
mod m20270108_000001_gate_keys_retired;
mod m20270109_000001_quota_share_gone;
mod m20270110_000001_queue_notices_one_road;
mod m20270111_000001_registration_owner;
mod m20270112_000002_deploy_due_retired;
mod m20270113_000001_road_advisor;
mod m20270114_000001_follows_asker;
mod m20270115_000001_organisation_supply;
mod m20270116_000001_session_reset_clean;
mod m20270120_000001_supervisor_sleep;
mod m20270121_000001_note_assistant_plain;

use sea_orm_migration::prelude::*;

/// The table the store's migrations keep the record of what ran in.
pub const MIGRATIONS_TABLE: &str = "agency_migrations";

/// The migration that makes every table; a seed given once is given as it runs (`crate::seed`).
pub const TABLES: &str = m20261016_000001_store::NAME;

/// The migration that makes the supervisors, the roles and the repositories tables of their own and carries
/// the agent rows over (docs/agents.md, 「Two axes」): what writes rows in the shape before it, the archive's
/// import, runs the migrations up to it, writes, and then runs it.
pub const TWO_AXES: &str = m20261124_000001_renewal::NAME;
/// The migration that leaves every supervisor on one road, the first entry of the order it held.
pub const SUPERVISOR_ROAD: &str = m20261221_000001_supervisor_road::NAME;

/// The migration that gives the roles the names the owner chose and retires the roles no one calls
/// (docs/agent-roles.md, 「The roles the policy seeds」).
pub const ROLE_NAMES: &str = m20261126_000001_role_names::NAME;

/// The settings a migration writes on its own, from what the rows it carries held.
pub const SETTINGS_WRITTEN: [&str; 1] = ["research_supervisor"];

/// Every table the store's migrations make, beside the record of what ran: what reads every table of the
/// store reads these and no other service's beside them (docs/store.md, 「Its tables」).
pub const OWN_TABLES: [&str; 63] = [
  "supervisors",
  "roles",
  "role_seeded",
  "repositories",
  "repository_node_checkouts",
  "support_checkouts",
  "role_sessions",
  "role_slot_requests",
  "role_agents",
  "role_slot_reservations",
  "role_provider_prompts",
  "role_provider_prompt_seeded",
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
  "person_nodes",
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
  "supervisor_settings",
  "supervisor_sleep",
  "settings",
  "assignments",
  "jobs",
  "job_sessions",
  "job_words",
  "session_work",
  "session_loads",
  "turn_liveness",
  "stream_events",
  "relays",
  "notices",
  "gateway_call_keys",
  "gateway_requests",
  "personal_supply",
  "supply_reservations",
  "supply_settlements",
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
      Box::new(m20261109_000003_usage_balance_requests::Migration),
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
      Box::new(m20261126_000001_role_names::Migration),
      Box::new(m20261127_000001_supervisor_settings::Migration),
      Box::new(m20261128_000001_supervisors_alike::Migration),
      Box::new(m20261129_000001_chosen_roads::Migration),
      Box::new(m20261130_000001_supervisor_card_rank::Migration),
      Box::new(m20261201_000001_relays::Migration),
      Box::new(m20261202_000001_supervisor_system_hash::Migration),
      Box::new(m20261203_000001_role_agents::Migration),
      Box::new(m20261204_000001_role_agent_settings::Migration),
      Box::new(m20261205_000001_role_skills::Migration),
      Box::new(m20261206_000001_agent_wanted::Migration),
      Box::new(m20261207_000001_arbiter_model::Migration),
      Box::new(m20261208_000001_arbiter_model_gone::Migration),
      Box::new(m20261209_000001_account_seats::Migration),
      Box::new(m20261210_000001_job_review::Migration),
      Box::new(m20261211_000001_person_nodes::Migration),
      Box::new(m20261212_000001_coworkers_gone::Migration),
      Box::new(m20261213_000001_agent_order_gone::Migration),
      Box::new(m20261214_000001_job_lines_by_agent::Migration),
      Box::new(m20261215_000001_job_seats_unbound::Migration),
      Box::new(m20261216_000001_job_machine_turn::Migration),
      Box::new(m20261217_000001_notices::Migration),
      Box::new(m20261218_000001_job_turn_words::Migration),
      Box::new(m20261219_000001_registration_languages::Migration),
      Box::new(m20261220_000001_job_notices_owed::Migration),
      Box::new(m20261221_000001_supervisor_road::Migration),
      Box::new(m20261222_000001_job_words::Migration),
      Box::new(m20261223_000001_deploy_notices_kept::Migration),
      Box::new(m20261224_000001_person_node_shared::Migration),
      Box::new(m20261225_000001_supervisor_limits::Migration),
      Box::new(m20261226_000001_asking_skill_name::Migration),
      Box::new(m20261227_000001_queue_notices_kept::Migration),
      Box::new(m20261228_000001_call_names::Migration),
      Box::new(m20261229_000001_short_slots::Migration),
      Box::new(m20261230_000001_run_turns::Migration),
      Box::new(m20261231_000001_double_gone::Migration),
      Box::new(m20270101_000001_support_auto::Migration),
      Box::new(m20270102_000001_lent_checkouts::Migration),
      Box::new(m20270103_000001_gateway_call_keys::Migration),
      Box::new(m20270104_000001_personal_supply::Migration),
      Box::new(m20270105_000001_owned_accounts::Migration),
      Box::new(m20270106_000001_supply_reservations::Migration),
      Box::new(m20270107_000001_run_cost::Migration),
      Box::new(m20270108_000001_gate_keys_retired::Migration),
      Box::new(m20270109_000001_quota_share_gone::Migration),
      Box::new(m20270110_000001_queue_notices_one_road::Migration),
      Box::new(m20270111_000001_registration_owner::Migration),
      Box::new(m20270112_000002_deploy_due_retired::Migration),
      Box::new(m20270113_000001_road_advisor::Migration),
      Box::new(m20270114_000001_follows_asker::Migration),
      Box::new(m20270115_000001_organisation_supply::Migration),
      Box::new(m20270116_000001_session_reset_clean::Migration),
      Box::new(m20270120_000001_supervisor_sleep::Migration),
      Box::new(m20270121_000001_note_assistant_plain::Migration),
    ]
  }

  fn migration_table_name() -> DynIden {
    Alias::new(MIGRATIONS_TABLE).into_iden()
  }
}
