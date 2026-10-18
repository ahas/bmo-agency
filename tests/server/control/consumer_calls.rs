//! The routes a consumer holding the rooms asks beside a turn, asked of a running server in the shapes that
//! consumer sends: a companion's agent row stood up and taken out, an agent's reset point, why a rest that has
//! just passed silenced the deployment, and words pushed into a session's working turn, on a store the server
//! settled itself.

use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

async fn get(port: u16, token: &str, path: &str) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .get(format!("http://127.0.0.1:{port}{path}"))
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
  assert_eq!(post(port, token, "/companions/add", added.clone()).await, (200, json!({ "ok": true })));
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

  let (status, taken) = post(port, token, "/companions/add", added.clone()).await;

  assert_eq!((status, taken["field"].as_str()), (400, Some("name")), "{taken}");

  let mut workerless = added.clone();

  workerless["name"] = json!("companion-b");
  workerless["worker"] = json!("chat-agent");

  let (status, refused) = post(port, token, "/companions/add", workerless).await;

  assert_eq!((status, refused["field"].as_str()), (400, Some("worker")), "{refused}");

  // Its reset point: none until a reset moves it, then the moment kept.
  assert_eq!(
    get(port, token, "/reset-point?agent=companion-a").await,
    (200, json!({ "point": null }))
  );
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('reset_point:companion-a', '2000')"#,
      vec![],
    )
    .await;
  assert_eq!(
    get(port, token, "/reset-point?agent=companion-a").await,
    (200, json!({ "point": 2000 }))
  );
  assert_eq!(get(port, token, "/reset-point").await.0, 400);

  // Words pushed into a session carrying no turn did not go in.
  assert_eq!(
    post(port, token, "/session/push", json!({ "session": "companion-a@-7", "words": "more" })).await,
    (200, json!({ "said": "refused" }))
  );
  assert_eq!(
    post(port, token, "/session/push", json!({ "session": "nobody@-7", "words": "more" }))
      .await
      .1["code"],
    "unknown-session"
  );

  // Taken out again with everything of its own; one the deployment seeds, or no companion, is refused.
  let (status, removed) = post(
    port,
    token,
    "/companions/remove",
    json!({ "agent": "companion-a", "rooms": [-7], "by": "owner" }),
  )
  .await;

  assert_eq!((status, &removed), (200, &json!({ "ok": true })));
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

  let (status, refused) = post(port, token, "/companions/remove", json!({ "agent": "helper" })).await;

  assert_eq!((status, refused["field"].as_str()), (400, Some("agent")), "{refused}");

  // The service's outage rest past its end: cleared as it is read, and told once.
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('rest_until', '1000'), ('rest_kind', 'outage')"#,
      vec![],
    )
    .await;
  assert_eq!(get(port, token, "/rest/passed").await, (200, json!({ "kind": "outage" })));
  assert_eq!(get(port, token, "/rest/passed").await, (200, json!({ "kind": null })));
  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "settings" WHERE "key" LIKE 'rest\_%'"#, vec![])
      .await,
    0
  );
}
