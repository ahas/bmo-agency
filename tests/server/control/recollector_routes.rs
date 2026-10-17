//! What the brain reads of a running server over its control routes (docs/api.md, 「Agents」, 「Sessions」,
//! 「Workers」): every agent with whether it is awake, its brain folder and its recollectors' registration;
//! the shared rulebooks by name; a run's clock and the editor cap; a verdict on an ask that is not there;
//! and a reload naming the agent and the rooms carried as published — on a store the server settled itself.

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
async fn a_running_server_answers_what_the_brains_recollectors_read() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "brain_folder") VALUES ('chat-agent', 'chat', ''),
           ('companion', 'companion', 'agents/companion/'), ('worker-one', 'worker', NULL)"#,
      vec![],
    )
    .await;
  scratch
    .run(r#"UPDATE "agents" SET "resting_since" = 1 WHERE "name" = 'worker-one'"#, vec![])
    .await;
  scratch
    .run(
      r#"INSERT INTO "companion_settings" ("agent", "recollector_model") VALUES ('companion', 'local-model')"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let (status, listed) = get(port, token, "/agents").await;

  assert_eq!(status, 200, "{listed}");

  let agent = |name: &str| {
    listed["agents"]
      .as_array()
      .unwrap()
      .iter()
      .find(|agent| agent["name"] == name)
      .cloned()
      .unwrap_or_else(|| panic!("{name} is listed"))
  };
  let read = |agent: Value| {
    (
      agent["awake"].clone(),
      agent["brainFolder"].clone(),
      agent["recollectorRegistration"].clone(),
    )
  };

  assert_eq!(read(agent("chat-agent")), (json!(true), json!(""), Value::Null));
  assert_eq!(read(agent("companion")), (json!(true), json!("agents/companion"), json!("local-model")));
  assert_eq!(read(agent("worker-one")), (json!(false), Value::Null, Value::Null));

  let (status, rulebooks) = get(port, token, "/rulebooks?names=plain-writing,self-preservation").await;

  assert_eq!(status, 200, "{rulebooks}");
  assert_eq!(rulebooks["rulebooks"][0]["name"], "plain-writing");
  assert_eq!(rulebooks["rulebooks"][1]["name"], "self-preservation");
  assert!(rulebooks["rulebooks"][1]["text"].as_str().unwrap().starts_with("# Self-preservation"));
  assert_eq!(get(port, token, "/rulebooks?names=no-such-rulebook").await.0, 404);
  assert_eq!(get(port, token, "/rulebooks?names=../secrets").await.0, 400);

  assert_eq!(
    get(port, token, "/runs/clock?run=bmo-digest").await,
    (200, json!({ "capEnds": null, "keptUntil": null }))
  );
  assert_eq!(get(port, token, "/runs/overtime").await, (200, json!({ "asks": [] })));
  assert_eq!(
    post(port, token, "/runs/overtime", json!({ "ask": 1, "verdict": "cut", "why": "stuck" }))
      .await
      .0,
    404
  );
  assert_eq!(post(port, token, "/runs/overtime", json!({ "ask": 1, "verdict": "wait" })).await.0, 400);
  assert_eq!(
    get(port, token, "/editors/cap?workKey=digest&agent=companion").await,
    (200, json!({ "capSeconds": policy::run_clock::RUN_CLOCK.editor_cap_seconds }))
  );
  assert_eq!(get(port, token, "/editors/cap?workKey=worker&agent=companion").await.0, 422);

  let moved =
    json!({ "paths": ["agents/companion/people/a.md"], "now": false, "agent": "companion", "rooms": [-7] });

  assert_eq!(post(port, token, "/documents/reload", moved.clone()).await, (200, moved));
  assert_eq!(
    post(port, token, "/documents/reload", json!({ "paths": ["a.md"] })).await,
    (200, json!({ "paths": ["a.md"], "now": false, "agent": null, "rooms": [] }))
  );
}
