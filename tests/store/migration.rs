//! The migrations make every table of the store in an empty database, and record what ran in the
//! store's own record.

use sea_orm_migration::MigratorTrait;
use store::migration::{MIGRATIONS_TABLE, Migrator, OWN_TABLES, TABLES};

use crate::scratch::Scratch;

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

  let agents = columns("agents").await;
  assert_eq!(
    agents[agents.len() - 7..],
    ["rowid", "node", "role", "session_scope", "turn_shape", "default_answerer", "prompts"]
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
