//! The move of the DeepSeek models from the registry to the deepseek provider (docs/cache-fallback.md,
//! 「The DeepSeek road」): the pins, tier mappings, roads, kept sessions and pool limits the migration
//! carries over, and the registrations it takes away.

use sea_orm::ConnectionTrait;

use crate::support::scratch::Scratch;

const MOVE: &str = include_str!("../../crates/store/src/migration/deepseek_provider.sql");

async fn settled() -> Scratch {
  let scratch = Scratch::before_two_axes().await;

  scratch
    .run(
      r#"INSERT INTO "registered_models" ("name", "label", "model", "context_tokens", "answer_tokens",
           "thinking", "walked", "added_at")
         VALUES ('deepseek-flash', 'DeepSeek V4.1 Flash', 'deepseek-flash', 1048576, 32768, 1, 0, 1),
                ('deepseek-v4-pro', 'DeepSeek V4 Pro', 'deepseek-v4-pro', 1048576, 32768, 1, 0, 1),
                ('engine', 'Engine', 'engine-model', 32768, 4096, 0, 1, 1)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "registered_model_addresses" ("registration", "node", "position", "url", "key_file")
         VALUES ('deepseek-flash', 'deepseek', 1, 'https://api.deepseek.com/v1/chat/completions', 'registered/deepseek-flash.key'),
                ('deepseek-v4-pro', 'deepseek', 1, 'https://api.deepseek.com/v1/chat/completions', 'registered/deepseek-v4-pro.key'),
                ('engine', 'node', 1, 'http://127.0.0.1:18020/v1/chat/completions', 'engine.key')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort")
         VALUES ('registered', 'BMO', 'deepseek-flash', NULL),
                ('registered', 'Light', 'deepseek-flash', NULL),
                ('registered', 'Fine', 'engine', NULL),
                ('claude-code', 'BMO', 'claude-opus-5-5', 'medium')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "pinned_provider", "pinned_model")
         VALUES ('worker-one', 'worker', 'registered', 'deepseek-flash'),
                ('worker-two', 'worker', 'registered', 'deepseek-v4-pro'),
                ('worker-three', 'worker', 'registered', 'engine'),
                ('chat', 'chat', NULL, NULL)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "fallback_routes" ("key", "set_at", "route_mode")
         VALUES ('bmo', 1, 'registered'), ('voice-reading', 1, 'registered'), ('translator', 1, 'claude-code')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value")
         VALUES ('agent_session:job-worker:worker-one', '{"mode":"registered","model":"deepseek-flash","effort":""}'),
                ('agent_session:job-worker:worker-three', '{"mode":"registered","model":"engine","effort":""}')"#,
      vec![],
    )
    .await;

  scratch
}

#[tokio::test]
async fn the_pins_mappings_and_roads_on_the_deepseek_models_move_to_the_provider() {
  let scratch = settled().await;
  scratch.db().execute_unprepared(MOVE).await.unwrap();

  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "agents" WHERE "pinned_provider" = 'deepseek'
           AND "pinned_model" IN ('deepseek-flash', 'deepseek-v4-pro')"#,
        vec![]
      )
      .await,
    2
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "agents" WHERE "pinned_provider" = 'registered' AND "pinned_model" = 'engine'"#,
        vec![]
      )
      .await,
    1
  );

  // The deepseek mappings are the seed's to write; a registration's mapping is gone.
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "model_tier_mappings" WHERE "provider" = 'registered' AND "model" LIKE 'deepseek-%'"#,
        vec![]
      )
      .await,
    0
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "model_tier_mappings" WHERE "provider" = 'registered' AND "model" = 'engine'"#,
        vec![]
      )
      .await,
    1
  );

  // The chat's road moves; a gate's walks, since this provider carries no gate.
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "fallback_routes" WHERE "key" = 'bmo' AND "route_mode" = 'deepseek'"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "fallback_routes" WHERE "key" = 'voice-reading' AND "route_mode" = 'auto'"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "fallback_routes" WHERE "key" = 'translator' AND "route_mode" = 'claude-code'"#,
        vec![]
      )
      .await,
    1
  );

  // A worker's kept session names the provider it answered on.
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "settings" WHERE "key" = 'agent_session:job-worker:worker-one'
           AND "value" LIKE '%"mode":"deepseek"%'"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "settings" WHERE "key" = 'agent_session:job-worker:worker-three'
           AND "value" LIKE '%"mode":"registered"%'"#,
        vec![]
      )
      .await,
    1
  );

  // The two registrations leave the registry and the other stays.
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "registered_models""#, vec![]).await, 1);
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "registered_model_addresses""#, vec![]).await, 1);

  // The co-worker limits are the seed's to write with the provider's mappings.
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "coworker_pool_tier_limits" WHERE "provider" = 'deepseek'"#,
        vec![]
      )
      .await,
    0
  );
}

#[tokio::test]
async fn the_move_is_one_way_and_a_second_reading_leaves_it_as_it_stands() {
  let scratch = settled().await;
  scratch.db().execute_unprepared(MOVE).await.unwrap();
  scratch.db().execute_unprepared(MOVE).await.unwrap();

  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "registered_models""#, vec![]).await, 1);
}
