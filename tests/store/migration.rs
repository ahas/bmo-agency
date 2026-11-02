//! The migrations make every table of the store in an empty database, record what ran in the store's own
//! record, and leave no recollector standing.

use sea_orm_migration::MigratorTrait;
use store::migration::{MIGRATIONS_TABLE, Migrator, OWN_TABLES, TABLES};

use crate::support::scratch::Scratch;

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

  let mut expected: Vec<String> = OWN_TABLES.iter().map(|name| (*name).to_owned()).collect();
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

  let roles = columns("roles").await;
  assert_eq!(
    roles[roles.len() - 7..],
    ["sessions", "description", "line", "provider_order", "quota_share", "fold_ratio", "callable"]
  );
  let accounts = columns("claude_accounts").await;
  assert_eq!(
    accounts[accounts.len() - 6..],
    ["rowid", "user_id", "endpoint_url", "key_file", "endpoint_models", "base_prompt"]
  );
  assert_eq!(columns("companion_settings").await.last().unwrap(), "saved_settings");
  let room_sessions = columns("room_sessions").await;
  assert_eq!(room_sessions[room_sessions.len() - 3..], ["rowid", "spent_calls", "spent_ms"]);
}

#[tokio::test]
async fn a_row_written_with_only_its_required_columns_takes_the_defaults_the_store_declares() {
  let scratch = Scratch::new().await;

  scratch
    .run(
      r#"INSERT INTO "repositories" ("name", "path", "created_at", "updated_at") VALUES ($1, $2, 0, 0)"#,
      vec!["worker-one".into(), "/checkout".into()],
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
    r#"SELECT "downgrade_heal", "support_count", "rowid" FROM "repositories" WHERE "name" = $1"#,
    vec!["worker-one".into()],
  )
  .await
  .unwrap()
  .unwrap();

  assert_eq!(row.try_get::<i64>("", "downgrade_heal").unwrap(), 1);
  assert_eq!(row.try_get::<i64>("", "support_count").unwrap(), 0);
  assert!(row.try_get::<i64>("", "rowid").unwrap() > 0);
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "settings""#, vec![]).await, 1);
}

#[tokio::test]
async fn a_base_prompt_an_account_names_cannot_be_removed_and_a_missing_one_cannot_be_named() {
  let scratch = Scratch::new().await;
  let account = |id: &'static str, prompt: &'static str| {
    let db = scratch.db();
    async move {
      store::run(
        db,
        r#"INSERT INTO "claude_accounts" ("id", "name", "place", "credential_kind", "credential_path",
             "added_at", "endpoint_url", "key_file", "endpoint_models", "base_prompt")
           VALUES ($1, $1, 0, 'endpoint', '/home', 0, 'https://endpoint.test', '/key', '{"opus":"pro"}', $2)"#,
        vec![id.into(), prompt.into()],
      )
      .await
    }
  };
  let remove_kept =
    || store::run(scratch.db(), r#"DELETE FROM "base_prompts" WHERE "name" = 'kept'"#, vec![]);
  scratch
    .run(
      r#"INSERT INTO "base_prompts" ("name", "label", "text") VALUES ('kept', 'Kept', 'text')"#,
      vec![],
    )
    .await;

  account("endpoint", "kept").await.unwrap();
  assert!(account("stray", "missing").await.is_err());
  assert!(remove_kept().await.is_err());

  scratch.run(r#"UPDATE "claude_accounts" SET "base_prompt" = NULL"#, vec![]).await;
  remove_kept().await.unwrap();
}

#[tokio::test]
async fn no_recollector_is_left_standing_and_nothing_else_goes_with_them() {
  let scratch = Scratch::blank().await;
  // Every migration before the one that retires the recollectors.
  let before = Migrator::migrations()
    .iter()
    .position(|migration| migration.name() == "m20261031_000001_recollectors_retired")
    .map(|place| u32::try_from(place).unwrap())
    .unwrap();

  Migrator::up(scratch.db(), Some(before)).await.unwrap();

  for statement in [
    r#"INSERT INTO "agents" ("name", "kind", "role") VALUES ('digest', 'recollector', 'recollector'),
         ('people-digest', 'recollector', 'recollector'), ('session-summary', 'recollector', 'recollector'),
         ('people-standing', 'gate', 'gate'), ('companion', 'chat', 'chat'), ('translator', 'gate', 'gate')"#,
    r#"INSERT INTO "companion_settings" ("agent", "persona", "recollector_model")
         VALUES ('companion', 'Plain.', 'local-model'), ('digest', NULL, NULL)"#,
    r#"INSERT INTO "runs" ("agent", "unit", "depth", "started_at") VALUES ('digest', 'u-1', 0, 1),
         ('people-digest', 'u-2', 0, 1), ('companion', 'u-3', 0, 1)"#,
    r#"INSERT INTO "agent_log" ("agent", "run", "at", "kind", "text") VALUES ('people-standing', 'r-1', 1, 'line', 'x'),
         ('companion', 'r-2', 1, 'line', 'y')"#,
    r#"INSERT INTO "run_clocks" ("run", "work_key", "agent", "what", "started_at", "bound_ms")
         VALUES ('clocked-digest', 'digest', 'companion', 'w', 1, 1), ('clocked-voice', 'voice-reading', 'companion', 'w', 1, 1)"#,
    r#"INSERT INTO "clock_asks" ("run", "facts", "asked_at", "answer_by") VALUES ('clocked-digest', '{}', 1, 2),
         ('clocked-voice', '{}', 1, 2)"#,
    r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('digest', 'Reliable'),
         ('people-standing', 'Simple'), ('session-summary', 'Reliable'), ('translator', 'Reliable')"#,
    r#"INSERT INTO "fallback_routes" ("key", "set_at") VALUES ('people-digest', 1), ('translator', 1)"#,
    r#"INSERT INTO "settings" ("key", "value") VALUES ('agent_gate:digest:sanitizer', 'off'),
         ('agent_sampling:people-digest', '{}'), ('reset_point:session-summary', '1'), ('reset_point:companion', '2')"#,
  ] {
    scratch.run(statement, vec![]).await;
  }

  Migrator::up(scratch.db(), None).await.unwrap();

  let count = |sql: &'static str| scratch.count(sql, vec![]);

  assert_eq!(
    count(
      r#"SELECT count(*) FROM information_schema.columns
         WHERE table_schema = current_schema() AND table_name = 'companion_settings' AND column_name = 'recollector_model'"#
    )
    .await,
    0
  );
  assert_eq!(count(r#"SELECT count(*) FROM "agents" WHERE "kind" = 'recollector'"#).await, 0);
  assert_eq!(
    count(r#"SELECT count(*) FROM "agents" WHERE "name" IN ('companion', 'translator')"#).await,
    2
  );
  assert_eq!(
    count(r#"SELECT count(*) FROM "agents" WHERE "name" IN ('digest', 'people-digest', 'session-summary', 'people-standing')"#)
      .await,
    0
  );
  assert_eq!(
    count(r#"SELECT count(*) FROM "companion_settings" WHERE "persona" = 'Plain.'"#).await,
    1
  );
  assert_eq!(count(r#"SELECT count(*) FROM "companion_settings" WHERE "agent" = 'digest'"#).await, 0);
  assert_eq!(count(r#"SELECT count(*) FROM "runs""#).await, 1);
  assert_eq!(count(r#"SELECT count(*) FROM "agent_log""#).await, 1);
  assert_eq!(count(r#"SELECT count(*) FROM "run_clocks" WHERE "run" = 'clocked-voice'"#).await, 1);
  assert_eq!(count(r#"SELECT count(*) FROM "run_clocks""#).await, 1);
  assert_eq!(count(r#"SELECT count(*) FROM "clock_asks""#).await, 1);
  assert_eq!(
    count(r#"SELECT count(*) FROM "model_tier_assignments" WHERE "key" = 'translator'"#).await,
    1
  );
  assert_eq!(
    count(r#"SELECT count(*) FROM "model_tier_assignments" WHERE "key" IN ('digest', 'people-standing', 'session-summary')"#)
      .await,
    0
  );
  assert_eq!(count(r#"SELECT count(*) FROM "fallback_routes" WHERE "key" = 'translator'"#).await, 1);
  assert_eq!(
    count(r#"SELECT count(*) FROM "fallback_routes" WHERE "key" = 'people-digest'"#).await,
    0
  );
  assert_eq!(
    count(r#"SELECT count(*) FROM "settings" WHERE "key" IN ('agent_gate:digest:sanitizer', 'agent_sampling:people-digest', 'reset_point:session-summary')"#)
      .await,
    0
  );
  assert_eq!(
    count(r#"SELECT count(*) FROM "settings" WHERE "key" = 'reset_point:companion'"#).await,
    1
  );
}

#[tokio::test]
async fn a_moment_kept_in_seconds_is_given_its_milliseconds_and_one_in_milliseconds_is_left() {
  let scratch = Scratch::blank().await;
  // Every migration before the one that holds the run log's and the ledger's moments in milliseconds.
  let before = Migrator::migrations()
    .iter()
    .position(|migration| migration.name() == "m20261111_000001_moments_in_milliseconds")
    .map(|place| u32::try_from(place).unwrap())
    .unwrap();

  Migrator::up(scratch.db(), Some(before)).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "agent_log" ("agent", "run", "at", "kind", "text")
           VALUES ('pal', 'seconds', 1793003847, 'tool', 'x'), ('pal', 'milliseconds', 1792580336000, 'tool', 'y')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "runs" ("agent", "unit", "depth", "started_at", "finished_at")
           VALUES ('pal', 'seconds', 0, 1793011298, 1793011319), ('pal', 'milliseconds', 0, 1792580336000, NULL)"#,
      vec![],
    )
    .await;
  Migrator::up(scratch.db(), None).await.unwrap();

  let scratch = &scratch;
  let run = |unit: &'static str, column: &'static str| async move {
    let sql = format!(r#"SELECT COALESCE("{column}", 0) FROM "runs" WHERE "unit" = '{unit}'"#);

    scratch.count(&sql, vec![]).await
  };

  assert_eq!(run("seconds", "started_at").await, 1_793_011_298_000);
  assert_eq!(run("seconds", "finished_at").await, 1_793_011_319_000);
  assert_eq!(run("milliseconds", "started_at").await, 1_792_580_336_000);
  assert_eq!(run("milliseconds", "finished_at").await, 0, "an open run stays open");

  let at =
    |run: &'static str| scratch.count(r#"SELECT "at" FROM "agent_log" WHERE "run" = $1"#, vec![run.into()]);

  assert_eq!(at("seconds").await, 1_793_003_847_000);
  assert_eq!(at("milliseconds").await, 1_792_580_336_000);
}
