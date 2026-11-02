//! The pool as the job unit reads it and writes a place's line, of a running server: one request by its token
//! whatever its state, the open ones by owner, the roles with their tiers, and a place's write kept with the
//! latest moment.

use serde_json::{Value, json};

use crate::support::scratch::Scratch;
use crate::support::standing;

async fn get(port: u16, token: &str, path: &str) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .get(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

async fn put(port: u16, token: &str, path: &str, body: Value) -> u16 {
  reqwest::Client::new()
    .patch(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .unwrap()
    .status()
    .as_u16()
}

fn tokens(listed: &Value) -> Vec<&str> {
  listed.as_array().unwrap().iter().map(|one| one["token"].as_str().unwrap()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_server_reads_the_pool_s_requests_and_roles_and_keeps_a_place_s_write() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  scratch
    .run(
      r#"INSERT INTO "coworker_pool_requests"
           ("token", "request_key", "owner", "source", "role", "work_key", "provider", "model", "effort", "tier",
            "capacity", "generation", "state", "enqueued_at", "leased_at", "child_id")
         VALUES ('t-1', 'k-1', 'order:1', 'cowork', 'tracer', 'role:tracer', 'codex', 'm', 'low', 'Light', 4, 1, 'waiting', 100, NULL, NULL),
                ('t-2', 'k-2', 'order:1', 'cowork', 'tracer', 'role:tracer', 'codex', 'm', 'low', 'Light', 4, 1, 'active', 110, $1, 'unit-2'),
                ('t-3', 'k-3', 'order:2', 'cowork', 'tracer', 'role:tracer', 'codex', 'm', 'low', 'Light', 4, 1, 'active', 130, $1, 'unit-3'),
                ('t-4', 'k-4', 'order:1', 'cowork', 'tracer', 'role:tracer', 'codex', 'm', 'low', 'Light', 4, 1, 'released', 90, 95, 'unit-4')"#,
      // The active leases were leased just now, so the server's pass over bound leases leaves them to the
      // launch's grace rather than reading their made-up children as ended.
      vec![agency_server::now_ms().into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "coworker_pool_roles" ("name", "description", "prompt", "tools", "skills", "enabled", "pooled", "set_at")
         VALUES ('tracer', 'Traces a road', '', '', '', 1, 1, 0), ('drafter', 'Drafts', '', '', '', 0, 0, 0)
         ON CONFLICT DO NOTHING"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('role:tracer', 'Light')
         ON CONFLICT ("key") DO UPDATE SET "tier" = excluded."tier""#,
      vec![],
    )
    .await;

  // One request by its token, ended or not, and nothing held for a token no request holds.
  let (status, request) = get(port, token, "/seat-requests/t-2").await;

  assert_eq!(status, 200, "{request}");
  assert_eq!(
    (
      &request["requestKey"],
      &request["state"],
      &request["workKey"],
      &request["childId"],
      &request["turn"]
    ),
    (&json!("k-2"), &json!("active"), &json!("role:tracer"), &json!("unit-2"), &json!(false))
  );
  assert_eq!(get(port, token, "/seat-requests/t-4").await.1["state"], json!("released"));

  // The seat reads whole as the schema declares it, with what its work key resolved to.
  let declared: bmo_agency_client::SeatRequest = serde_json::from_value(request.clone()).unwrap();

  assert_eq!(declared.state, bmo_agency_client::SeatRequestState::Active);
  assert_eq!(
    (
      declared.provider.as_str(),
      declared.model.as_str(),
      declared.effort.as_deref(),
      declared.tier.as_str()
    ),
    ("codex", "m", Some("low"), "Light")
  );

  let (status, absent) = get(port, token, "/seat-requests/nobody").await;

  assert_eq!((status, absent["code"].as_str()), (404, Some("not-found")), "{absent}");

  // The open requests in the order they came, an owner's alone where one is named.
  let (_, open) = get(port, token, "/seat-requests").await;

  assert_eq!(tokens(&open["items"]), ["t-1", "t-2", "t-3"]);
  assert_eq!(
    tokens(&get(port, token, "/seat-requests?owner=order:1").await.1["items"]),
    ["t-1", "t-2"]
  );

  // The roles with the tier their assignment names, empty where none does.
  let (_, roles) = get(port, token, "/roles").await;
  let roles = &roles["items"];
  let role = |name: &str| roles.as_array().unwrap().iter().find(|one| one["name"] == name).cloned();

  assert_eq!(
    role("tracer"),
    Some(
      json!({ "name": "tracer", "description": "Traces a road", "enabled": true, "pooled": true, "tier": "Light" })
    )
  );
  assert_eq!(role("drafter").map(|one| one["tier"].clone()), Some(json!("")));

  // A place's write kept, a later one replacing what it was written with.
  let place = "/places/branch-line:codex:m:low";

  assert_eq!(
    put(port, token, place, json!({ "provider": "codex", "model": "m", "effort": "low" })).await,
    200
  );
  assert_eq!(
    put(
      port,
      token,
      place,
      json!({ "provider": "codex", "model": "m", "effort": null, "upstream": "up-1" })
    )
    .await,
    200
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "coworker_pool_place_writes" WHERE "place" = 'branch-line:codex:m:low'
           AND "provider" = 'codex' AND "effort" = '' AND "upstream" = 'up-1' AND "written_at" > 0"#,
        vec![],
      )
      .await,
    1
  );
  assert_eq!(put(port, token, place, json!({ "model": "m" })).await, 400);
}
