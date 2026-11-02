//! The boundary's resource answers a running server gives: a road block listed by its id and opened
//! again, the launch room's counts and the launcher's notice, usage grouped by what the query names,
//! the standing branch places, and the edits to a provider and to a registered model.

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

async fn send(port: u16, token: &str, method: reqwest::Method, path: &str, body: Value) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .request(method, format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_road_block_is_listed_by_its_id_and_opened_again() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let until = chrono::Utc::now().timestamp_millis() + 60_000;

  let (status, block) = send(
    port,
    token,
    reqwest::Method::POST,
    "/road-blocks",
    json!({ "selection": "claude-code::opus", "until": until, "reason": "the road is down" }),
  )
  .await;

  assert_eq!(status, 201, "{block}");
  assert_eq!(
    (&block["selection"], &block["reason"], block["id"].as_i64().is_some()),
    (&json!("claude-code::opus"), &json!("the road is down"), true)
  );

  let (_, listed) = get(port, token, "/road-blocks").await;

  assert_eq!(listed["items"].as_array().unwrap().len(), 1, "{listed}");
  assert_eq!(listed["items"][0]["id"], block["id"]);

  let id = block["id"].as_i64().unwrap();

  assert_eq!(
    send(port, token, reqwest::Method::DELETE, &format!("/road-blocks/{id}"), Value::Null)
      .await
      .0,
    204
  );
  assert_eq!(get(port, token, "/road-blocks").await.1["items"], json!([]));
  assert_eq!(
    send(port, token, reqwest::Method::DELETE, &format!("/road-blocks/{id}"), Value::Null)
      .await
      .0,
    404
  );

  // A block names a provider's model and a moment it opens again.
  assert_eq!(
    send(
      port,
      token,
      reqwest::Method::POST,
      "/road-blocks",
      json!({ "selection": "claude-code::opus" })
    )
    .await
    .0,
    400
  );
  assert_eq!(
    send(
      port,
      token,
      reqwest::Method::POST,
      "/road-blocks",
      json!({ "selection": "nobody", "until": until })
    )
    .await
    .0,
    400
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_launch_room_answers_its_counts_and_keeps_the_launchers_notice() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  crate::support::rows::repository(scratch.db(), "worker-a", "/checkouts/worker-a", &[]).await;
  scratch
    .run(
      r#"INSERT INTO "agent_executions" ("id", "body") VALUES ('e-1', '{"agent":"worker-a"}')"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  let (status, room) = get(port, token, "/launch-room").await;

  assert_eq!(status, 200, "{room}");
  assert_eq!((&room["held"], &room["jobCount"]), (&json!(false), &json!(1)), "{room}");

  let (_, held) = send(
    port,
    token,
    reqwest::Method::PATCH,
    "/launch-room",
    json!({ "held": true, "reasons": ["a launch is already going"] }),
  )
  .await;

  assert_eq!(
    (&held["held"], &held["reasons"]),
    (&json!(true), &json!(["a launch is already going"])),
    "{held}"
  );
  assert_eq!(get(port, token, "/launch-room").await.1["held"], json!(true));

  let (_, withdrawn) =
    send(port, token, reqwest::Method::PATCH, "/launch-room", json!({ "held": false })).await;

  assert_eq!((&withdrawn["held"], &withdrawn["reasons"]), (&json!(false), &json!([])));
  assert_eq!(
    send(
      port,
      token,
      reqwest::Method::PATCH,
      "/launch-room",
      json!({ "held": true, "reasons": [] })
    )
    .await
    .0,
    400
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn usage_is_grouped_by_what_the_query_names_and_carries_the_sessions_context() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  let now = chrono::Utc::now().timestamp_millis();

  for (worker, holder, room, model, tokens) in [
    ("worker-a", "holder-1", 7, "opus", 100),
    ("worker-a", "holder-2", 7, "opus", 50),
    ("worker-b", "holder-1", 8, "sonnet", 30),
  ] {
    scratch
      .run(
        &format!(
          r#"INSERT INTO "jobs" ("item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
                "room_id", "state", "token_count", "started_at", "prompt", "model")
             VALUES (1, 1, '{holder}', 1, '{worker}', {room}, 'ended', {tokens}, {now}, 'p', '{model}')"#
        ),
        vec![],
      )
      .await;
  }

  scratch
    .run(
      &format!(
        r#"INSERT INTO "role_sessions" ("role", "path", "provider", "model", "account", "command_key", "session",
             "opened_at", "answered_at", "context_tokens")
           VALUES ('worker', '/checkouts/worker-a', 'claude-code', 'opus', 'default', 'key', 's-1', {now}, {now}, 4_000)"#
      ),
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let span = format!("/usage?after={}&before={}", now - 60_000, now + 60_000);

  let (status, by_model) = get(port, token, &format!("{span}&groupBy=model")).await;

  assert_eq!(status, 200, "{by_model}");
  assert_eq!(
    (&by_model["groups"][0]["key"], &by_model["groups"][0]["tokens"]),
    (&json!("opus"), &json!(150)),
    "{by_model}"
  );
  assert_eq!(
    (&by_model["groups"][1]["key"], &by_model["groups"][1]["tokens"]),
    (&json!("sonnet"), &json!(30))
  );
  assert_eq!(by_model["groups"][0]["contextTokens"], json!(4_000));

  let (_, by_agent) = get(port, token, &format!("{span}&groupBy=agent")).await;

  assert_eq!(
    (&by_agent["groups"][0]["key"], &by_agent["groups"][0]["tokens"]),
    (&json!("worker-a"), &json!(150))
  );

  let (_, by_user) = get(port, token, &format!("{span}&groupBy=user")).await;

  assert_eq!(
    (&by_user["groups"][0]["key"], &by_user["groups"][0]["tokens"]),
    (&json!("holder-1"), &json!(130))
  );

  let (_, by_room) = get(port, token, &format!("{span}&groupBy=room")).await;

  assert_eq!(
    (&by_room["groups"][0]["key"], &by_room["groups"][0]["tokens"]),
    (&json!("7"), &json!(150))
  );
  assert_eq!(get(port, token, &format!("{span}&groupBy=nothing")).await.0, 400);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_place_list_reads_the_standing_checkouts_and_their_last_write() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";
  let slot = data.join("branch-agents").join("repo-abcd123456").join("1");

  std::fs::create_dir_all(&slot).unwrap();
  scratch
    .run(
      r#"INSERT INTO "branch_agents" ("pool", "slot", "mode", "model", "role", "upstream", "prefix_digest", "last_reply_at")
         VALUES ('repo-abcd123456', 1, 'claude-code', 'opus', 'tracer', 'up-1', 'd', 1_700_000_000)"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let path = slot.to_string_lossy().into_owned();
  let named = path.replace('/', "%2F");

  let (status, listed) = get(port, token, "/places").await;
  let item = &listed["items"][0];

  assert_eq!(status, 200, "{listed}");
  assert_eq!(listed["items"].as_array().unwrap().len(), 1, "{listed}");
  assert_eq!(
    (&item["id"], &item["slot"], &item["role"], &item["state"]),
    (&json!(path), &json!(1), &json!("tracer"), &json!("remembered")),
    "{listed}"
  );
  assert_eq!(item["lastWrite"]["provider"], json!("claude-code"));
  assert_eq!(get(port, token, &format!("/places/{named}")).await.1["role"], json!("tracer"));

  // A pool request holding the place marks it taken, and a write it took is its last write.
  scratch
    .run(
      &format!(
        r#"INSERT INTO "role_slot_requests"
             ("token", "request_key", "owner", "source", "role", "provider", "model", "effort",
              "capacity", "state", "enqueued_at", "place_key")
           VALUES ('t-1', 'k-1', 'order:1', 'cowork', 'tracer', 'claude-code', 'opus', 'low',
              4, 'active', 100, '{path}')"#
      ),
      vec![],
    )
    .await;
  assert_eq!(get(port, token, "/places").await.1["items"][0]["state"], json!("taken"));
  assert_eq!(
    send(
      port,
      token,
      reqwest::Method::PATCH,
      &format!("/places/{named}"),
      json!({ "provider": "codex", "model": "m", "effort": "low" })
    )
    .await
    .0,
    200
  );
  assert_eq!(get(port, token, "/places").await.1["items"][0]["lastWrite"]["provider"], json!("codex"));

  // A checkout on a node is held here as the shadow of it.
  let (_, shadow) = get(port, token, "/places?nodeName=node-1&checkout=/srv/a").await;

  assert_eq!(shadow["items"][0]["nodeName"], json!("node-1"));
  assert!(
    shadow["items"][0]["path"]
      .as_str()
      .is_some_and(|path| path.ends_with("nodes/node-1/shadow/-srv-a")),
    "{shadow}"
  );
  assert_eq!(get(port, token, "/places?nodeName=node-1").await.0, 400);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_provider_s_switch_and_a_registered_model_s_name_are_changed_as_their_resources_say() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value")
         VALUES ('provider_order', '[{"name":"codex","enabled":true},{"name":"claude-code","enabled":false},
           {"name":"openrouter","enabled":false},{"name":"registered","enabled":false}]')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "registered_models" ("name", "label", "model", "context_tokens", "answer_tokens",
            "thinking", "walked", "added_at", "keeps_research")
         VALUES ('mine', 'Mine', 'old-model', 100000, 4000, 0, 1, 1, 0)"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  let (status, on) = send(
    port,
    token,
    reqwest::Method::PATCH,
    "/providers/claude-code",
    json!({ "name": "claude-code", "enabled": true }),
  )
  .await;

  assert_eq!((status, &on["enabled"]), (200, &json!(true)), "{on}");

  let (status, off) = send(
    port,
    token,
    reqwest::Method::PATCH,
    "/providers/codex",
    json!({ "name": "codex", "enabled": false }),
  )
  .await;

  assert_eq!((status, &off["enabled"]), (200, &json!(false)), "{off}");
  assert_eq!(
    send(port, token, reqwest::Method::PATCH, "/providers/nobody", json!({ "enabled": true }))
      .await
      .0,
    404
  );

  // The last provider on is not turned off, since nothing would answer.
  for other in ["deepseek", "openrouter", "registered"] {
    assert_eq!(
      send(
        port,
        token,
        reqwest::Method::PATCH,
        &format!("/providers/{other}"),
        json!({ "enabled": false })
      )
      .await
      .0,
      200,
      "{other}"
    );
  }
  assert_eq!(
    send(
      port,
      token,
      reqwest::Method::PATCH,
      "/providers/claude-code",
      json!({ "enabled": false })
    )
    .await
    .0,
    409
  );

  let (status, changed) = send(
    port,
    token,
    reqwest::Method::PATCH,
    "/models/registered::mine",
    json!({ "model": "new-model" }),
  )
  .await;

  assert_eq!(status, 200, "{changed}");
  assert_eq!(
    (&changed["provider"], &changed["model"], &changed["key"]),
    (&json!("registered"), &json!("new-model"), &json!("registered::mine"))
  );
  assert_eq!(get(port, token, "/models/registered::mine").await.1["model"], json!("new-model"));
  assert_eq!(
    send(
      port,
      token,
      reqwest::Method::PATCH,
      "/models/claude-code::opus",
      json!({ "model": "x" })
    )
    .await
    .0,
    404
  );
  assert_eq!(
    send(port, token, reqwest::Method::DELETE, "/models/registered::mine", Value::Null)
      .await
      .0,
    204
  );
  assert_eq!(get(port, token, "/models/registered::mine").await.0, 404);
}

// The model resource is one record: what the list shows is what a create makes and what an edit and a
// delete act on. The catalogue of model files is a resource of its own and never appears in that list.
#[tokio::test(flavor = "multi_thread")]
async fn the_model_resource_is_one_record() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(
      r#"INSERT INTO "registered_models" ("name", "label", "model", "context_tokens", "answer_tokens",
            "thinking", "walked", "added_at", "keeps_research")
         VALUES ('mine', 'Mine', 'old-model', 100000, 4000, 0, 1, 1, 0)"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  // The list is the registrations the store holds.
  let (status, page) = get(port, token, "/models").await;

  assert_eq!(status, 200, "{page}");
  assert_eq!(page["more"], json!(false));
  assert_eq!(
    page["items"],
    json!([{
      "key": "registered::mine",
      "name": "mine",
      "provider": "registered",
      "model": "old-model",
      "cardService": Value::Null,
      "roleFiles": false,
      "usedBy": [],
      "healthy": false,
      "lastAnsweredAt": Value::Null,
    }])
  );

  // A provider that is not the registry's has no models here.
  assert_eq!(get(port, token, "/models?provider=registered").await.0, 200);
  assert_eq!(get(port, token, "/models?provider=claude-code").await.0, 404);

  // A create makes a registration, and the list then shows the same record.
  let (status, made) = send(
    port,
    token,
    reqwest::Method::POST,
    "/models",
    json!({ "key": "registered::made", "provider": "registered", "model": "a-model" }),
  )
  .await;

  assert_eq!(status, 201, "{made}");
  assert_eq!(made["key"], json!("registered::made"));
  assert_eq!(made["model"], json!("a-model"));

  let (_, page) = get(port, token, "/models").await;
  let keys: Vec<&str> =
    page["items"].as_array().unwrap().iter().filter_map(|one| one["key"].as_str()).collect();

  assert_eq!(keys, ["registered::mine", "registered::made"], "{page}");

  // The edit and the delete act on that record.
  let (status, changed) = send(
    port,
    token,
    reqwest::Method::PATCH,
    "/models/registered::made",
    json!({ "model": "changed-model" }),
  )
  .await;

  assert_eq!(status, 200, "{changed}");
  assert_eq!(changed["model"], json!("changed-model"));
  assert_eq!(get(port, token, "/models").await.1["items"][1]["model"], json!("changed-model"));

  // A change may name the card service its engine is alone, and none takes it away again.
  let (status, carded) = send(
    port,
    token,
    reqwest::Method::PATCH,
    "/models/registered::made",
    json!({ "cardService": "lab-arena", "roleFiles": true }),
  )
  .await;

  assert_eq!(status, 200, "{carded}");
  assert_eq!((&carded["cardService"], &carded["roleFiles"]), (&json!("lab-arena"), &json!(true)));
  assert_eq!(carded["model"], json!("changed-model"));

  let (status, _) = send(
    port,
    token,
    reqwest::Method::PATCH,
    "/models/registered::made",
    json!({ "cardService": "none" }),
  )
  .await;

  assert_eq!(status, 200);
  assert_eq!(get(port, token, "/models/registered::made").await.1["cardService"], Value::Null);
  assert_eq!(
    send(port, token, reqwest::Method::PATCH, "/models/registered::made", json!({})).await.0,
    400
  );

  assert_eq!(
    send(port, token, reqwest::Method::DELETE, "/models/registered::made", Value::Null)
      .await
      .0,
    204
  );
  assert_eq!(get(port, token, "/models/registered::made").await.0, 404);
  assert_eq!(get(port, token, "/models").await.1["items"].as_array().unwrap().len(), 1);

  // The catalogue is a resource of its own: an entry is made and read there, and the model route neither
  // lists it nor edits or deletes it.
  let (status, catalogue) = send(
    port,
    token,
    reqwest::Method::POST,
    "/model-catalogue",
    json!({
      "action": "save",
      "name": "rvn",
      "label": "RVN",
      "file": "RVN-Q4_K_M.gguf",
      "bytes": 1234,
      "sha256": "ab".repeat(32),
    }),
  )
  .await;

  assert_eq!(status, 200, "{catalogue}");
  assert_eq!(catalogue["entries"][0]["name"], json!("rvn"));
  assert_eq!(get(port, token, "/model-catalogue").await.1["entries"][0]["name"], json!("rvn"));

  let (_, page) = get(port, token, "/models").await;
  let keys: Vec<&str> =
    page["items"].as_array().unwrap().iter().filter_map(|one| one["key"].as_str()).collect();

  assert_eq!(keys, ["registered::mine"], "{page}");
  assert_eq!(
    send(port, token, reqwest::Method::PATCH, "/models/registered::rvn", json!({ "model": "x" }))
      .await
      .0,
    404
  );
  assert_eq!(
    send(port, token, reqwest::Method::DELETE, "/models/registered::rvn", Value::Null)
      .await
      .0,
    404
  );
}
