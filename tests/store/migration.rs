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
    roles[roles.len() - 8..],
    [
      "sessions",
      "description",
      "line",
      "provider_order",
      "fold_ratio",
      "active_provider",
      "active_model",
      "skills"
    ]
  );
  let accounts = columns("claude_accounts").await;
  assert_eq!(
    accounts[accounts.len() - 8..],
    [
      "rowid",
      "user_id",
      "endpoint_url",
      "key_file",
      "endpoint_models",
      "base_prompt",
      "node",
      "lendable"
    ]
  );
  let settings = columns("supervisor_settings").await;
  assert_eq!(settings[settings.len() - 3..], ["saved_settings", "brain_on", "tools_allowed"]);
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
  assert_eq!(row.try_get::<Option<i64>>("", "support_count").unwrap(), None, "auto");
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
         ('people-standing', 'gate', 'gate'), ('pal', 'chat', 'chat'), ('translator', 'gate', 'gate')"#,
    r#"INSERT INTO "companion_settings" ("agent", "persona", "recollector_model")
         VALUES ('pal', 'Plain.', 'local-model'), ('digest', NULL, NULL)"#,
    r#"INSERT INTO "runs" ("agent", "unit", "depth", "started_at") VALUES ('digest', 'u-1', 0, 1),
         ('people-digest', 'u-2', 0, 1), ('pal', 'u-3', 0, 1)"#,
    r#"INSERT INTO "agent_log" ("agent", "run", "at", "kind", "text") VALUES ('people-standing', 'r-1', 1, 'line', 'x'),
         ('pal', 'r-2', 1, 'line', 'y')"#,
    r#"INSERT INTO "run_clocks" ("run", "work_key", "agent", "what", "started_at", "bound_ms")
         VALUES ('clocked-digest', 'digest', 'pal', 'w', 1, 1), ('clocked-voice', 'voice-reading', 'pal', 'w', 1, 1)"#,
    r#"INSERT INTO "clock_asks" ("run", "facts", "asked_at", "answer_by") VALUES ('clocked-digest', '{}', 1, 2),
         ('clocked-voice', '{}', 1, 2)"#,
    r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('digest', 'Reliable'),
         ('people-standing', 'Simple'), ('session-summary', 'Reliable'), ('translator', 'Reliable')"#,
    r#"INSERT INTO "fallback_routes" ("key", "set_at") VALUES ('people-digest', 1), ('translator', 1)"#,
    r#"INSERT INTO "settings" ("key", "value") VALUES ('agent_gate:digest:sanitizer', 'off'),
         ('agent_sampling:people-digest', '{}'), ('reset_point:session-summary', '1'), ('reset_point:pal', '2')"#,
  ] {
    scratch.run(statement, vec![]).await;
  }

  crate::support::scratch::up_to_two_axes(scratch.db()).await;

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
  assert_eq!(count(r#"SELECT count(*) FROM "agents" WHERE "name" IN ('pal', 'translator')"#).await, 2);
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
  assert_eq!(count(r#"SELECT count(*) FROM "settings" WHERE "key" = 'reset_point:pal'"#).await, 1);
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

#[tokio::test]
async fn every_supervisor_s_settings_stand_in_the_supervisors_settings_table_with_its_rows_kept() {
  let scratch = Scratch::blank().await;
  // Every migration before the one that names the settings table for the supervisors.
  let before = Migrator::migrations()
    .iter()
    .position(|migration| migration.name() == "m20261127_000001_supervisor_settings")
    .map(|place| u32::try_from(place).unwrap())
    .unwrap();

  Migrator::up(scratch.db(), Some(before)).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "companion_settings" ("agent", "persona", "generation") VALUES ('pal', 'Plain.', '{}')"#,
      vec![],
    )
    .await;
  Migrator::up(scratch.db(), None).await.unwrap();

  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "supervisor_settings" WHERE "agent" = 'pal' AND "persona" = 'Plain.'"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM pg_constraint WHERE conname IN ('supervisor_settings_pkey', 'supervisor_settings_rowid_key')"#,
        vec![]
      )
      .await,
    2
  );
  scratch
    .run(r#"INSERT INTO "supervisor_settings" ("agent") VALUES ('another')"#, vec![])
    .await;
}

#[tokio::test]
async fn a_job_s_lease_bound_to_a_turn_s_run_is_unbound_and_every_other_binding_kept() {
  let scratch = Scratch::blank().await;
  // Every migration before the one that unbinds a job's seat from its turns.
  let before = Migrator::migrations()
    .iter()
    .position(|migration| migration.name() == "m20261215_000001_job_seats_unbound")
    .map(|place| u32::try_from(place).unwrap())
    .unwrap();

  Migrator::up(scratch.db(), Some(before)).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "role_slot_requests" ("token", "request_key", "owner", "source", "role", "path", "provider",
           "model", "effort", "capacity", "state", "enqueued_at", "class", "seat", "child_id", "child_seen",
           "session")
         VALUES
           ('turn', 'turn', 'job-1', 'job', 'coder', '/c', 'p', 'm', '', 1, 'active', 0, 'foreground', 'coder-01',
            'job-1', 1, NULL),
           ('session', 'session', 'job-2', 'job', 'coder', '/c', 'p', 'm', '', 1, 'active', 0, 'foreground',
            'coder-02', 's-2', 1, 's-2'),
           ('unit', 'unit', 'owner:x', 'cowork', 'coder', '/c', 'p', 'm', '', 1, 'active', 0, 'foreground',
            'coder-03', 'unit-3', 1, NULL)"#,
      vec![],
    )
    .await;
  Migrator::up(scratch.db(), None).await.unwrap();

  let bound = |token: &'static str| {
    scratch.count(
      r#"SELECT count(*) FROM "role_slot_requests" WHERE "token" = $1 AND "child_id" IS NOT NULL AND "child_seen" = 1"#,
      vec![token.into()],
    )
  };

  assert_eq!(bound("turn").await, 0, "a job's lease bound to its turn's run is unbound");
  assert_eq!(bound("session").await, 1, "a lease bound to a role session is kept");
  assert_eq!(bound("unit").await, 1, "another source's lease is kept");
}

#[tokio::test]
async fn a_standing_deploy_notice_is_kept_due_no_more_once_told_and_every_other_notice_left_as_it_was() {
  let scratch = Scratch::blank().await;
  // Every migration before the one that keeps a commit waiting for its deployment.
  let before = Migrator::migrations()
    .iter()
    .position(|migration| migration.name() == "m20261223_000001_deploy_notices_kept")
    .map(|place| u32::try_from(place).unwrap())
    .unwrap();

  Migrator::up(scratch.db(), Some(before)).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "notices" ("key", "route", "body", "keep", "mark", "owed_at", "told_at", "due_at")
         VALUES
           ('item-awaits-deploy:1', 'instruction', '{}', 'standing', 'c1', 1, 2, 300),
           ('item-awaits-deploy:2', 'instruction', '{}', 'standing', 'c2', 1, NULL, 5),
           ('item-awaits-launch:3', 'instruction', '{}', 'standing', '', 1, 2, 300)"#,
      vec![],
    )
    .await;
  Migrator::up(scratch.db(), Some(1)).await.unwrap();

  let rows = |keep: &'static str, due: &'static str| {
    scratch.count(
      r#"SELECT count(*) FROM "notices" WHERE "keep" = $1 AND "key" = ANY(string_to_array($2, ','))"#,
      vec![keep.into(), due.into()],
    )
  };
  let due_no_more = scratch.count(
    r#"SELECT count(*) FROM "notices" WHERE "key" = 'item-awaits-deploy:1' AND "due_at" IS NULL"#,
    vec![],
  );
  let still_sent = scratch.count(
    r#"SELECT count(*) FROM "notices" WHERE "key" = 'item-awaits-deploy:2' AND "due_at" = 5"#,
    vec![],
  );

  assert_eq!(
    rows("kept", "item-awaits-deploy:1,item-awaits-deploy:2").await,
    2,
    "both deploy rows kept"
  );
  assert_eq!(due_no_more.await, 1, "a row told is due no more");
  assert_eq!(still_sent.await, 1, "a row not yet told is still sent");
  assert_eq!(rows("standing", "item-awaits-launch:3").await, 1, "another standing notice stands");
}

#[tokio::test]
async fn a_standing_launch_or_close_notice_is_kept_due_no_more_once_told_and_marked_with_its_lease() {
  let scratch = Scratch::blank().await;
  // Every migration before the one that keeps the launch and close notices.
  let before = Migrator::migrations()
    .iter()
    .position(|migration| migration.name() == "m20261227_000001_queue_notices_kept")
    .map(|place| u32::try_from(place).unwrap())
    .unwrap();

  Migrator::up(scratch.db(), Some(before)).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "notices" ("key", "route", "body", "keep", "mark", "owed_at", "told_at", "due_at")
         VALUES
           ('item-awaits-launch:1', 'instruction', '{}', 'standing', '', 1, 2, 300),
           ('job-awaits-close:2', 'instruction', '{}', 'standing', '', 1, 2, 300),
           ('job-awaits-close:3', 'instruction', '{}', 'standing', '', 1, NULL, 5),
           ('job-review:4', 'instruction', '{}', 'standing', '', 1, 2, 300)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "jobs" ("id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "state",
                             "started_at", "prompt", "lease_kept")
         VALUES (1, 2, 15, 'harness', 1, 'w1', 'ended', 1, 'work', true)"#,
      vec![],
    )
    .await;
  // Every migration after it up to the one that lets the launch notice go.
  let launch_gone = Migrator::migrations()
    .iter()
    .position(|migration| migration.name() == "m20270110_000001_queue_notices_one_road")
    .map(|place| u32::try_from(place).unwrap())
    .unwrap();

  Migrator::up(scratch.db(), Some(launch_gone - before)).await.unwrap();

  let told = scratch.count(
    r#"SELECT count(*) FROM "notices"
        WHERE "keep" = 'kept' AND "due_at" IS NULL AND "key" IN ('item-awaits-launch:1', 'job-awaits-close:2')"#,
    vec![],
  );
  let still_sent = scratch.count(
    r#"SELECT count(*) FROM "notices" WHERE "keep" = 'kept' AND "due_at" = 5 AND "key" = 'job-awaits-close:3'"#,
    vec![],
  );
  let review = scratch.count(
    r#"SELECT count(*) FROM "notices" WHERE "keep" = 'standing' AND "key" = 'job-review:4'"#,
    vec![],
  );

  assert_eq!(told.await, 2, "a row told is kept, due no more");
  assert_eq!(still_sent.await, 1, "a row not yet told is kept and still sent");
  assert_eq!(review.await, 1, "a review's check stands");

  let marked = scratch.count(
    r#"SELECT count(*) FROM "notices" WHERE "key" = 'job-awaits-close:2' AND "mark" = '15'"#,
    vec![],
  );

  assert_eq!(
    marked.await,
    1,
    "a close notice told before is marked with the lease its ended job keeps"
  );
}

#[tokio::test]
async fn the_double_s_role_and_its_open_slots_go_and_no_slot_is_kept_as_asked_short() {
  let scratch = Scratch::blank().await;
  // Every migration before the one that takes the supervisor's double away.
  let before = Migrator::migrations()
    .iter()
    .position(|migration| migration.name() == "m20261231_000001_double_gone")
    .map(|place| u32::try_from(place).unwrap())
    .unwrap();

  Migrator::up(scratch.db(), Some(before)).await.unwrap();

  for role in ["doppelganger", "helper"] {
    scratch
      .run(
        r#"INSERT INTO "roles" ("name", "label", "max_concurrent", "prompt", "position", "created_at", "updated_at")
           VALUES ($1, $1, 2, NULL, 0, 0, 0)"#,
        vec![role.into()],
      )
      .await;
    scratch
      .run(r#"INSERT INTO "role_seeded" ("name") VALUES ($1)"#, vec![role.into()])
      .await;
    scratch
      .run(
        r#"INSERT INTO "role_agents" ("name", "role") VALUES ($1 || '-01', $1)"#,
        vec![role.into()],
      )
      .await;
    scratch
      .run(
        r#"INSERT INTO "role_sessions" ("role", "path", "provider", "model", "command_key", "session", "opened_at")
           VALUES ($1, '/scratch', 'deepseek', 'deepseek-flash', 'k', $1, 0)"#,
        vec![role.into()],
      )
      .await;
  }

  scratch
    .run(
      r#"INSERT INTO "role_provider_prompts" ("role", "provider", "prompt", "updated_at")
         VALUES ('doppelganger', 'claude-code', 'Work as the supervisor.', 0)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "role_slot_reservations" ("token", "reservation_key", "owner", "source", "role", "provider",
           "model", "effort", "wanted", "state", "created_at")
         VALUES ('r1', 'r1', 'r1', 'cowork', 'doppelganger', 'deepseek', 'deepseek-flash', 'low', 1, 'held', 0)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "role_slot_requests" ("token", "request_key", "owner", "source", "role", "provider", "model",
           "effort", "capacity", "state", "enqueued_at", "outcome", "short")
         VALUES
           ('asked', 'asked', 'o', 'delegate', 'helper', '', '', '', 2, 'waiting', 0, NULL, false),
           ('running', 'running', 'o', 'delegate', 'helper', 'p', 'm', '', 2, 'active', 0, NULL, true),
           ('answered', 'answered', 'o', 'delegate', 'helper', 'p', 'm', '', 2, 'released', 0, 'answered', false),
           ('of-the-double', 'of-the-double', 'o', 'cowork', 'doppelganger', '', '', '', 2, 'waiting', 0, NULL,
            false),
           ('a-job', 'a-job', 'job-1', 'job', 'worker', 'p', 'm', '', 2, 'active', 0, NULL, false)"#,
      vec![],
    )
    .await;
  Migrator::up(scratch.db(), Some(1)).await.unwrap();

  for table in ["role_agents", "role_sessions", "role_slot_reservations", "role_provider_prompts"] {
    let left = scratch
      .count(&format!(r#"SELECT count(*) FROM "{table}" WHERE "role" = 'doppelganger'"#), vec![])
      .await;

    assert_eq!(left, 0, "the double's {table} go");
  }

  for table in ["roles", "role_seeded"] {
    let left = scratch
      .count(&format!(r#"SELECT count(*) FROM "{table}" WHERE "name" = 'doppelganger'"#), vec![])
      .await;

    assert_eq!(left, 0, "the double's role goes from {table}");
  }

  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "role_agents" WHERE "role" = 'helper'"#, vec![])
      .await,
    1,
    "another role keeps its agents"
  );

  let given_back = |token: &'static str, state: &'static str| {
    scratch.count(
      r#"SELECT count(*) FROM "role_slot_requests"
          WHERE "token" = $1 AND "state" = $2 AND "ended_at" IS NOT NULL AND "ended_by" = 'owner'
            AND "outcome" LIKE '%double was removed%'"#,
      vec![token.into(), state.into()],
    )
  };

  assert_eq!(given_back("asked", "cancelled").await, 1, "a waiting delegated slot is cancelled");
  assert_eq!(given_back("running", "released").await, 1, "a delegated slot let in is released");
  assert_eq!(
    given_back("of-the-double", "cancelled").await,
    1,
    "a waiting slot of the double's role is cancelled"
  );

  let stands = |token: &'static str, state: &'static str| {
    scratch.count(
      r#"SELECT count(*) FROM "role_slot_requests" WHERE "token" = $1 AND "state" = $2 AND "ended_by" IS NULL"#,
      vec![token.into(), state.into()],
    )
  };

  assert_eq!(stands("answered", "released").await, 1, "an ended delegated slot stays as it ended");
  assert_eq!(stands("a-job", "active").await, 1, "another source's slot stands");

  let short = scratch
    .count(
      r#"SELECT count(*) FROM information_schema.columns
          WHERE table_schema = current_schema() AND table_name = 'role_slot_requests' AND column_name = 'short'"#,
      vec![],
    )
    .await;

  assert_eq!(short, 0, "no slot is kept as asked short");
}

#[tokio::test]
async fn the_retired_sanitiser_and_output_format_gates_kinds_and_switches_go() {
  let scratch = Scratch::blank().await;
  // Every migration before the one that lets the two gates go.
  let before = Migrator::migrations()
    .iter()
    .position(|migration| migration.name() == "m20270108_000001_gate_keys_retired")
    .map(|place| u32::try_from(place).unwrap())
    .unwrap();

  Migrator::up(scratch.db(), Some(before)).await.unwrap();

  scratch
    .run(
      r#"INSERT INTO "gate_kinds" ("key", "part", "step", "position", "declared_at")
         VALUES ('sanitizer', 'pre', 0, 0, 0), ('output-format', 'post', 0, 1, 0), ('translator', 'apart', 0, 2, 0)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value")
         VALUES ('agent_gate:pal:sanitizer', 'off'), ('agent_gate:pal:output-format', 'off'),
           ('agent_gate:pal:hacking-gate', 'off'), ('agent_gate:pal:translator', 'off'), ('reset_point:sanitizer', '1')"#,
      vec![],
    )
    .await;
  Migrator::up(scratch.db(), Some(1)).await.unwrap();

  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "gate_kinds" WHERE "key" <> 'translator'"#, vec![])
      .await,
    0,
    "the two gates' kinds go"
  );
  assert_eq!(
    scratch.count(r#"SELECT count(*) FROM "gate_kinds""#, vec![]).await,
    1,
    "another gate's kind stays"
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "settings" WHERE "key" IN ('agent_gate:pal:translator', 'reset_point:sanitizer')"#,
        vec![]
      )
      .await,
    2,
    "another gate's switch and a key that is no switch stay"
  );
  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "settings" WHERE "key" LIKE 'agent_gate:%'"#, vec![])
      .await,
    1,
    "the retired gates' switches go"
  );
}

#[tokio::test]
async fn the_owner_road_s_notices_the_launch_notices_and_the_idle_queue_s_rows_go_a_told_launch_withdrawn()
{
  let scratch = Scratch::blank().await;
  // Every migration before the one that tells every notice of the queue as an instruction to its orderer.
  let before = Migrator::migrations()
    .iter()
    .position(|migration| migration.name() == "m20270110_000001_queue_notices_one_road")
    .map(|place| u32::try_from(place).unwrap())
    .unwrap();

  Migrator::up(scratch.db(), Some(before)).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "notices" ("key", "route", "body", "keep", "mark", "instruction_id", "owed_at", "told_at",
                                "due_at", "tries", "failure")
         VALUES
           ('job-failed:7', 'owner', '{}', 'once', '', NULL, 1, NULL, 5, 2, 'cut'),
           ('item-awaits-launch:1', 'instruction', '{}', 'kept', '', 41, 1, 2, NULL, 0, NULL),
           ('item-awaits-launch:2', 'instruction', '{}', 'kept', '', NULL, 1, NULL, 5, 3, 'cut'),
           ('job-ended:3', 'instruction', '{}', 'once', '', NULL, 1, NULL, 5, 0, NULL)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value")
         VALUES ('queue_idle_alarm', 'off'), ('queue_idle_told', '1'), ('queue_idle_minutes', '5'),
           ('queue_item_waiting:1', '1'), ('queue_item_waiting:2', '1'), ('queue_draft_told:4', '1')"#,
      vec![],
    )
    .await;
  Migrator::up(scratch.db(), Some(1)).await.unwrap();

  let count = |sql: &'static str| scratch.count(sql, vec![]);

  assert_eq!(
    count(r#"SELECT count(*) FROM "notices" WHERE "route" = 'owner'"#).await,
    0,
    "the owner road's notices go"
  );
  assert_eq!(
    count(
      r#"SELECT count(*) FROM "notices"
          WHERE "key" = 'item-awaits-launch:1' AND "route" = 'withdrawal' AND "body"::jsonb ->> 'id' = '41'
            AND "keep" = 'once' AND "due_at" IS NOT NULL AND "tries" = 0 AND "failure" IS NULL"#
    )
    .await,
    1,
    "a told launch notice turns into the withdrawal of its instruction, due now"
  );
  assert_eq!(
    count(r#"SELECT count(*) FROM "notices" WHERE "key" = 'item-awaits-launch:2'"#).await,
    0,
    "an untold launch notice goes"
  );
  assert_eq!(
    count(r#"SELECT count(*) FROM "notices" WHERE "key" = 'job-ended:3' AND "route" = 'instruction'"#)
      .await,
    1,
    "another instruction stays"
  );
  assert_eq!(
    count(r#"SELECT count(*) FROM "settings""#).await,
    1,
    "the idle queue's rows go, another queue row stays"
  );
  assert_eq!(count(r#"SELECT count(*) FROM "settings" WHERE "key" = 'queue_draft_told:4'"#).await, 1);
}
