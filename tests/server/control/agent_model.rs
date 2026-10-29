//! A supervisor's model method (docs/model-tiers.md, 「A supervisor's own model」): a chat agent's model
//! kept on a registered model, its key's road stood on the pin's clearing, a companion returned to the
//! same route, and an agent that is no supervisor refused one.

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

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_agents_model_is_kept_on_a_registration_and_returned_to_its_key_road() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "label", "builtin", "session_scope", "turn_shape", "default_answerer")
         VALUES ('bmo', 'chat', 'Chat', 1, 'agent', 'window', 1),
                ('kakao', 'chat', 'Kakao', 1, 'agent', 'pieces', 0),
                ('helper', 'worker', 'Helper', 0, NULL, NULL, 0)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "registered_models" ("name", "label", "model", "context_tokens", "answer_tokens",
            "wait_seconds", "thinking", "walked", "added_at", "keeps_research")
         VALUES ('local-model', 'Local', 'served', 100000, 4000, 60, 1, 1, 1, 0)"#,
      vec![],
    )
    .await;

  // The chat agent is kept on a registration, and its pin answered as it stands.
  let (status, fixed) = patch(
    port,
    token,
    "/agents/bmo",
    json!({ "model": { "method": "fixed", "registration": "local-model" } }),
  )
  .await;

  assert_eq!(status, 200, "{fixed}");
  assert_eq!(fixed["pinnedProvider"], "registered");
  assert_eq!(fixed["pinnedModel"], "local-model");

  // Its route clears the pin, and the key answers on its road again.
  let (status, route) = patch(port, token, "/agents/bmo", json!({ "model": { "method": "route" } })).await;

  assert_eq!(status, 200, "{route}");
  assert_eq!(route["pinnedProvider"], Value::Null);
  assert_eq!(route["pinnedModel"], Value::Null);

  // A companion takes the same route, and an agent that is no supervisor takes none; a name the registry
  // holds no registration under is no model.
  let (status, companion) =
    patch(port, token, "/agents/kakao", json!({ "model": { "method": "route" } })).await;

  assert_eq!(status, 200, "{companion}");

  let (status, refused) =
    patch(port, token, "/agents/helper", json!({ "model": { "method": "route" } })).await;

  assert_eq!(status, 400);
  assert!(refused["error"].as_str().unwrap_or_default().contains("no model"), "{refused}");

  let (status, refused) = patch(
    port,
    token,
    "/agents/bmo",
    json!({ "model": { "method": "fixed", "registration": "nowhere" } }),
  )
  .await;

  assert_eq!(status, 400, "{refused}");
  assert!(refused["error"].as_str().unwrap_or_default().contains("nowhere"), "{refused}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_companions_pin_is_cleared_and_it_answers_on_the_supervisor_route_again() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "label", "builtin", "session_scope", "turn_shape", "default_answerer",
            "pinned_provider", "pinned_model")
         VALUES ('bmo', 'chat', 'Chat', 1, 'agent', 'window', 1, NULL, NULL),
                ('companion-a', 'chat', 'Companion A', 0, 'agent', 'pieces', 0, 'registered', 'local-model')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "registered_models" ("name", "label", "model", "context_tokens", "answer_tokens",
            "wait_seconds", "thinking", "walked", "added_at", "keeps_research")
         VALUES ('local-model', 'Local', 'served', 100000, 4000, 60, 1, 1, 1, 0)"#,
      vec![],
    )
    .await;

  let (status, route) =
    patch(port, token, "/companions/companion-a", json!({ "registration": null })).await;

  assert_eq!(status, 200, "{route}");
  assert_eq!(route["method"], "route", "{route}");
  assert_eq!(route["registration"], Value::Null, "{route}");
  assert!(route["provider"].as_str().is_some_and(|provider| !provider.is_empty()), "{route}");

  // The pin is gone from its row, and the model the route answers is what it shows.
  let (status, agent) = get(port, token, "/agents/companion-a").await;

  assert_eq!(status, 200, "{agent}");
  assert_eq!(agent["pinnedProvider"], Value::Null, "{agent}");
}
