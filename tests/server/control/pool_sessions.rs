//! A project's own turn through the pool continues the session its worker was working in
//! (docs/coworker-pool.md, 「A project's own session」): a warm session within its model's context is
//! carried on the request, a session past the provider's cache lifetime or over the model's context limit
//! is dropped with its record and the turn opens a fresh one, and a reused session changes no tier's
//! number of co-workers.

use agency_server::now_ms;
use serde_json::{Value, json};

use crate::support::scratch::Scratch;
use crate::support::standing;

async fn post(port: u16, token: &str, path: &str, body: Value) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .post(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

async fn patch(port: u16, token: &str, path: &str, body: Value) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .patch(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

// A builder turn's place: the project's checkout, under the role's tier.
fn turn(key: &str, owner: &str, checkout: &str) -> Value {
  json!({
    "requestKey": key,
    "owner": owner,
    "role": "tracer",
    "selection": "Light",
    "tier": "Light",
    "provider": "claude-code",
    "source": "turn",
    "placeKey": checkout,
  })
}

// A co-work branch's seat, counted against the tier.
fn seat(key: &str, owner: &str) -> Value {
  json!({
    "requestKey": key,
    "owner": owner,
    "role": "tracer",
    "selection": "Light",
    "tier": "Light",
    "provider": "claude-code",
    "source": "cowork",
  })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_project_s_turn_continues_the_session_its_worker_holds() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  for sql in [
    r#"INSERT INTO "coworker_pool_roles" ("name", "description", "prompt", "tools", "skills", "enabled", "pooled", "set_at")
       VALUES ('tracer', 'Traces a road', '', '', '', 1, 1, 0) ON CONFLICT DO NOTHING"#,
    r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('role:tracer', 'Light')
       ON CONFLICT ("key") DO UPDATE SET "tier" = excluded."tier""#,
    r#"INSERT INTO "model_tier_mappings" ("provider", "tier", "model", "effort") VALUES ('claude-code', 'Light', 'claude-sonnet-5', 'low')
       ON CONFLICT ("provider", "tier") DO UPDATE SET "model" = excluded."model", "effort" = excluded."effort""#,
    r#"INSERT INTO "coworker_pool_tier_limits" ("provider", "tier", "capacity", "set_at") VALUES ('claude-code', 'Light', 1, 0)
       ON CONFLICT ("provider", "tier") DO UPDATE SET "capacity" = 1"#,
  ] {
    scratch.run(sql, vec![]).await;
  }

  let warm = home.path().join("project-warm");
  let cold = home.path().join("project-cold");
  let full = home.path().join("project-full");
  let tier = home.path().join("project-tier");

  for (name, checkout) in [
    ("worker-warm", &warm),
    ("worker-cold", &cold),
    ("worker-full", &full),
    ("worker-tier", &tier),
  ] {
    scratch
      .run(
        r#"INSERT INTO "agents" ("name", "kind", "cwd") VALUES ($1, 'worker', $2)"#,
        vec![name.into(), checkout.to_string_lossy().into_owned().into()],
      )
      .await;
  }

  let now = now_ms().div_euclid(1000);

  for (worker, session, answered, ratio) in [
    ("worker-warm", "sess-warm", now, Some(0.5)),
    ("worker-cold", "sess-cold", now - 2 * 60 * 60, Some(0.5)),
    ("worker-full", "sess-full", now, Some(1.0)),
    ("worker-tier", "sess-tier", now, Some(0.5)),
  ] {
    scratch
      .run(
        r#"INSERT INTO "worker_sessions" ("worker", "model", "account", "claude_session_id", "last_reply_at", "context_ratio")
           VALUES ($1, 'claude-sonnet-5', 'default', $2, $3, $4)"#,
        vec![worker.into(), session.into(), answered.into(), ratio.into()],
      )
      .await;
  }

  // A warm session within the model's context is the one the turn continues.
  let (status, held) =
    post(port, token, "/seat-requests", turn("order:1:turn", "order:1", &warm.to_string_lossy())).await;

  assert_eq!(status, 201, "{held}");
  assert_eq!(held["session"], json!("sess-warm"), "{held}");
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "worker_sessions" WHERE "worker" = 'worker-warm'"#, vec![])
      .await,
    1
  );

  // A session past the provider's cache lifetime opens a new one and loses its record.
  let (status, stale) =
    post(port, token, "/seat-requests", turn("order:2:turn", "order:2", &cold.to_string_lossy())).await;

  assert_eq!(status, 201, "{stale}");
  assert_eq!(stale["session"], Value::Null, "{stale}");
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "worker_sessions" WHERE "worker" = 'worker-cold'"#, vec![])
      .await,
    0
  );

  // A session whose last answer filled its model's context opens a new one and loses its record.
  let (status, oversized) =
    post(port, token, "/seat-requests", turn("order:3:turn", "order:3", &full.to_string_lossy())).await;

  assert_eq!(status, 201, "{oversized}");
  assert_eq!(oversized["session"], Value::Null, "{oversized}");
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "worker_sessions" WHERE "worker" = 'worker-full'"#, vec![])
      .await,
    0
  );

  // The tier's maximum number of co-workers is untouched by session reuse: one branch holds the only
  // place, a project's own turn with a session goes in beside it as turns always do, and the next branch
  // still waits.
  let (status, first) = post(port, token, "/seat-requests", seat("order:4:0", "order:4")).await;

  assert_eq!(status, 201, "{first}");
  assert_eq!(first["session"], Value::Null, "{first}");

  let (status, active) =
    patch(port, token, &format!("/seat-requests/{}", first["token"].as_str().unwrap()), json!({})).await;

  assert_eq!((status, &active["request"]["state"]), (200, &json!("active")), "{active}");

  let (status, project) =
    post(port, token, "/seat-requests", turn("order:4:turn", "order:4", &tier.to_string_lossy())).await;

  assert_eq!(status, 201, "{project}");
  assert_eq!(project["session"], json!("sess-tier"), "{project}");

  let (status, admitted) = patch(
    port,
    token,
    &format!("/seat-requests/{}", project["token"].as_str().unwrap()),
    json!({}),
  )
  .await;

  assert_eq!((status, &admitted["request"]["state"]), (200, &json!("active")), "{admitted}");

  let (status, next) = post(port, token, "/seat-requests", seat("order:4:1", "order:4")).await;

  assert_eq!(status, 201, "{next}");

  let (status, waited) =
    patch(port, token, &format!("/seat-requests/{}", next["token"].as_str().unwrap()), json!({})).await;

  assert_eq!((status, &waited["request"]), (200, &Value::Null), "{waited}");

  assert_eq!(
    scratch
      .count(
        r#"SELECT "capacity" FROM "coworker_pool_tier_limits" WHERE "provider" = 'claude-code' AND "tier" = 'Light'"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "coworker_pool_requests" WHERE "state" = 'active' AND "turn" = 0"#,
        vec![]
      )
      .await,
    1
  );
}
