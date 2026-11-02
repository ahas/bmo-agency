//! The routes a consumer holding the rooms asks beside a turn, asked of a running server in the shapes that
//! consumer sends: a companion's agent row stood up and taken out, a save of its settings naming a field none
//! takes, an agent's reset point, why a rest that has
//! just passed silenced the deployment, words pushed into a session's working turn, and a run of the ledger
//! read by its id and listed by its search beside its opening and closing, on a store the server settled
//! itself.

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

async fn delete(port: u16, token: &str, path: &str) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .delete(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

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

#[tokio::test(flavor = "multi_thread")]
async fn a_running_server_answers_the_calls_a_room_holder_makes_beside_its_turns() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind") VALUES ('helper', 'worker'), ('chat-agent', 'chat')"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let added = json!({
    "name": "companion-a",
    "label": "Companion A",
    "provider": "claude-code",
    "model": "m",
    "worker": "helper",
    "room": -7,
  });

  // A companion's row stood up from the fields the console's dialog gave, and refused once its name is taken
  // or its worker is none.
  assert_eq!(post(port, token, "/companions", added.clone()).await, (201, json!({ "ok": true })));
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "agents" WHERE "name" = 'companion-a' AND "kind" = 'chat'
           AND "session_scope" = 'room' AND "default_answerer" = 0 AND "pinned_provider" = 'claude-code'
           AND "pinned_model" = 'm' AND "item_worker" = 'helper' AND "brain_folder" = 'agents/companion-a'
           AND "builtin" = 0"#,
        vec![],
      )
      .await,
    1
  );

  let (status, taken) = post(port, token, "/companions", added.clone()).await;

  assert_eq!((status, taken["field"].as_str()), (400, Some("name")), "{taken}");

  let mut workerless = added.clone();

  workerless["name"] = json!("companion-b");
  workerless["worker"] = json!("chat-agent");

  let (status, refused) = post(port, token, "/companions", workerless).await;

  assert_eq!((status, refused["field"].as_str()), (400, Some("worker")), "{refused}");

  // Its reset point: none until a reset moves it, then the moment kept.
  let (_, before) = get(port, token, "/agents/companion-a").await;
  assert_eq!(before["resetAt"], Value::Null);
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('reset_point:companion-a', '2000')"#,
      vec![],
    )
    .await;
  let (_, after) = get(port, token, "/agents/companion-a").await;
  assert_eq!(after["resetAt"], json!(2000));

  // A save naming a field no save takes — the recollectors' model among them — is refused by that field,
  // and nothing is written.
  let (status, refused) =
    patch(port, token, "/companions/companion-a", json!({ "recollectorModel": "local-model" })).await;

  assert_eq!((status, refused["field"].as_str()), (400, Some("recollectorModel")), "{refused}");

  for retired in ["pieces", "ownPieces"] {
    let (status, refused) = patch(port, token, "/companions/companion-a", json!({ retired: [] })).await;

    assert_eq!((status, refused["field"].as_str()), (400, Some(retired)), "{refused}");
  }

  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "companion_settings" WHERE "agent" = 'companion-a'"#, vec![])
      .await,
    0
  );

  // Words name the turn they are pushed into, and a turn the session carries none of is not there.
  assert_eq!(
    post(port, token, "/sessions/companion-a@-7/turns/1/inputs", json!({ "words": "more" })).await,
    (404, json!({ "code": "unknown-turn", "error": "no turn 1 is going in companion-a@-7" }))
  );
  assert_eq!(
    post(port, token, "/sessions/nobody@-7/turns/1/inputs", json!({ "words": "more" }))
      .await
      .1["code"],
    "unknown-session"
  );

  // Taken out again with everything of its own; one the deployment seeds, or no companion, is refused.
  let (status, _) = delete(port, token, "/companions/companion-a").await;

  assert_eq!(status, 204);
  assert_eq!(
    scratch
      .count(
        r#"SELECT (SELECT count(*) FROM "agents" WHERE "name" = 'companion-a')
           + (SELECT count(*) FROM "settings" WHERE "key" = 'reset_point:companion-a')"#,
        vec![],
      )
      .await,
    0
  );

  let (status, refused) = delete(port, token, "/companions/helper").await;

  assert_eq!((status, refused["field"].as_str()), (400, Some("agent")), "{refused}");

  // The service's outage rest past its end: cleared as it is read, and told once.
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('rest_until', '1000'), ('rest_kind', 'outage')"#,
      vec![],
    )
    .await;
  assert_eq!(
    patch(port, token, "/rest", json!({ "clearPassed": true })).await,
    (200, json!({ "resting": false, "until": null, "kind": "outage", "accountId": null }))
  );
  assert_eq!(
    patch(port, token, "/rest", json!({ "clearPassed": true })).await,
    (200, json!({ "resting": false, "until": null, "kind": null, "accountId": null }))
  );
  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "settings" WHERE "key" LIKE 'rest\_%'"#, vec![])
      .await,
    0
  );

  // A run of the ledger, opened and closed in the shapes the job unit sends and read as the room holder reads
  // it; a run with no row is nothing held, and a static path beside the id's is still its own route.
  let (status, opened) = post(
    port,
    token,
    "/runs",
    json!({
      "agent": "helper",
      "chat_id": null,
      "unit": "unit-1",
      "depth": 2,
      "cap_ms": 60_000,
      "started_at": 4_000,
      "started_by": "chat:-7",
      "search_id": "12",
    }),
  )
  .await;

  assert_eq!(status, 201, "{opened}");

  let id = opened["id"].as_i64().unwrap();
  let read = format!("/runs/{id}");

  let (status, read_run) = get(port, token, &read).await;

  assert_eq!(status, 200);
  assert_eq!(read_run["id"], json!(id));
  assert_eq!(read_run["agentName"], json!("helper"));
  assert_eq!(read_run["unitName"], json!("unit-1"));
  assert_eq!(read_run["state"], json!("open"));
  assert_eq!(read_run["startedAt"], json!(4_000));

  let closing = json!({ "finishedAt": 9_000, "outcome": "done", "contextTokens": 30_000 });

  assert_eq!(patch(port, token, &read, closing.clone()).await.0, 200);
  assert_eq!(patch(port, token, &read, json!({ "finishedAt": 9_500, "outcome": "cut" })).await.0, 409);

  let (_, closed) = get(port, token, &read).await;

  assert_eq!(
    (&closed["outcome"], &closed["contextTokens"], &closed["state"]),
    (&json!("done"), &json!(30_000), &json!("closed")),
    "the first closing is kept"
  );

  let (status, numbered) = post(
    port,
    token,
    "/runs",
    json!({ "agent": "helper", "unit": "unit-2", "depth": 0, "started_at": 5_000, "search_id": 12 }),
  )
  .await;

  assert_eq!(status, 201, "{numbered}");
  assert_eq!(
    scratch.count(r#"SELECT count(*) FROM "runs" WHERE "search_id" = '12'"#, vec![]).await,
    2,
    "a search named by its number or its text is kept the same"
  );
  let (_, searched) = get(port, token, "/runs?search=12").await;
  let ids: Vec<i64> = searched["items"]
    .as_array()
    .unwrap()
    .iter()
    .filter_map(|run| run["id"].as_i64())
    .collect();

  assert_eq!(
    ids,
    vec![numbered["id"].as_i64().unwrap(), id],
    "the runs that carried a search, newest first"
  );
  assert_eq!(get(port, token, "/runs?search=13").await.1["items"], json!([]));
  assert_eq!(get(port, token, "/runs").await.0, 200);

  let missing = id + 1_000;

  assert_eq!(
    get(port, token, &format!("/runs/{missing}")).await,
    (
      404,
      json!({ "code": "not-found", "error": format!("the ledger holds no run {missing}") })
    )
  );
  assert_eq!(patch(port, token, &format!("/runs/{missing}"), closing).await.1["code"], "not-found");
  assert_eq!(post(port, token, "/runs", json!({ "agent": "helper" })).await.0, 400);
  assert_eq!(get(port, token, &read).await.0, 200);
}
