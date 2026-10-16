//! The migrations make every table of the store in an empty database, and record what ran in the
//! store's own record.

use sea_orm_migration::MigratorTrait;
use store::migration::{MIGRATIONS_TABLE, Migrator, TABLES};

use crate::scratch::Scratch;

// Every table the store keeps (docs/store.md, 「Its tables」).
const STORE_TABLES: [&str; 52] = [
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

async fn tables(scratch: &Scratch) -> Vec<String> {
  let rows = store::all(
    scratch.db(),
    "SELECT table_name::text AS name FROM information_schema.tables \
     WHERE table_schema = current_schema() AND table_type = 'BASE TABLE' ORDER BY table_name",
    vec![],
  )
  .await
  .unwrap();

  rows.iter().map(|row| row.try_get::<String>("", "name").unwrap()).collect()
}

#[tokio::test]
async fn the_migrations_make_every_table_of_the_store_and_nothing_else() {
  let scratch = Scratch::blank().await;

  Migrator::up(scratch.db(), None).await.unwrap();

  let mut expected: Vec<String> = STORE_TABLES.iter().map(|name| (*name).to_owned()).collect();
  expected.push(MIGRATIONS_TABLE.to_owned());
  expected.sort();
  assert_eq!(tables(&scratch).await, expected);
}

#[tokio::test]
async fn what_ran_is_recorded_in_the_stores_own_record_and_nothing_is_left_pending() {
  let scratch = Scratch::new().await;

  let ran = scratch
    .count(
      &format!(r#"SELECT COUNT(*) FROM "{MIGRATIONS_TABLE}" WHERE "version" = $1"#),
      vec![TABLES.into()],
    )
    .await;

  assert_eq!(ran, 1);
  assert!(Migrator::get_pending_migrations(scratch.db()).await.unwrap().is_empty());
}

#[tokio::test]
async fn the_columns_carried_beyond_a_tables_first_shape_stand_after_its_row_id() {
  let scratch = Scratch::new().await;
  let columns = |table: &'static str| {
    let db = scratch.db();
    async move {
      store::all(
        db,
        "SELECT column_name::text AS name FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = $1 ORDER BY ordinal_position",
        vec![table.into()],
      )
      .await
      .unwrap()
      .iter()
      .map(|row| row.try_get::<String>("", "name").unwrap())
      .collect::<Vec<_>>()
    }
  };

  let agents = columns("agents").await;
  assert_eq!(
    agents[agents.len() - 6..],
    ["rowid", "node", "role", "session_scope", "turn_shape", "default_answerer"]
  );
  assert_eq!(columns("claude_accounts").await.last().unwrap(), "user_id");
  assert_eq!(columns("companion_settings").await.last().unwrap(), "saved_settings");
  let room_sessions = columns("room_sessions").await;
  assert_eq!(room_sessions[room_sessions.len() - 3..], ["rowid", "spent_calls", "spent_ms"]);
}

#[tokio::test]
async fn a_row_written_with_only_its_required_columns_takes_the_defaults_the_store_declares() {
  let scratch = Scratch::new().await;

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind") VALUES ($1, $2)"#,
      vec!["worker-one".into(), "worker".into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ($1, $2)"#,
      vec!["rest_kind".into(), "day".into()],
    )
    .await;

  let row = store::one(
    scratch.db(),
    r#"SELECT "default_answerer", "runs", "rowid" FROM "agents" WHERE "name" = $1"#,
    vec!["worker-one".into()],
  )
  .await
  .unwrap()
  .unwrap();

  assert_eq!(row.try_get::<i64>("", "default_answerer").unwrap(), 0);
  assert_eq!(row.try_get::<i64>("", "runs").unwrap(), 0);
  assert!(row.try_get::<i64>("", "rowid").unwrap() > 0);
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "settings""#, vec![]).await, 1);
}
