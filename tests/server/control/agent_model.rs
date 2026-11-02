//! An agent's own model (docs/model-tiers.md, 「An agent's own model」): a chat agent's model kept on a
//! registered model, its key's road stood on the pin's clearing, a companion returned to the same route, an
//! agent that is no supervisor refused the route, and any agent kept on a provider's model its catalogue
//! names, refused one its provider does not hold or a provider agency does not carry.

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
            "thinking", "walked", "added_at", "keeps_research")
         VALUES ('local-model', 'Local', 'served', 100000, 4000, 1, 1, 1, 0)"#,
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
  assert_eq!(refused["field"], "model.method", "{refused}");
  assert!(
    refused["error"].as_str().unwrap_or_default().contains("no supervisor route"),
    "{refused}"
  );

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
            "thinking", "walked", "added_at", "keeps_research")
         VALUES ('local-model', 'Local', 'served', 100000, 4000, 1, 1, 1, 0)"#,
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

// A catalogue the Claude CLI would keep under the machine's configuration directory, naming the models given.
fn claude_catalogue(home: &std::path::Path, models: &[&str]) {
  let place = home.join(".claude/cache/model-catalog");
  let models: Vec<Value> = models
    .iter()
    .map(|id| json!({ "id": id, "section": "main", "thinking": { "type": "none" } }))
    .collect();
  let file = json!({ "version": 2, "fetchedAt": 1_000, "staleAt": 3_601_000, "catalog": { "surface": "cc",
    "config": { "id": "cc", "models": models } } });

  std::fs::create_dir_all(&place).unwrap();
  std::fs::write(place.join("test-cc.json"), file.to_string()).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn any_agent_keeps_a_providers_model_its_catalogue_names_and_is_refused_one_it_does_not_hold() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  claude_catalogue(home.path(), &["claude-opus-5-5", "claude-fable-5-1"]);

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "label", "builtin", "session_scope", "turn_shape", "default_answerer")
         VALUES ('helper', 'worker', 'Helper', 0, NULL, NULL, 0),
                ('search', 'runner', 'Search', 0, NULL, NULL, 0)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "registered_models" ("name", "label", "model", "context_tokens", "answer_tokens",
            "thinking", "walked", "added_at", "keeps_research")
         VALUES ('local-model', 'Local', 'served', 100000, 4000, 1, 1, 1, 0)"#,
      vec![],
    )
    .await;

  // A worker is kept on a Claude model its CLI's catalogue names, and a runner's agent likewise.
  for agent in ["helper", "search"] {
    let (status, fixed) = patch(
      port,
      token,
      &format!("/agents/{agent}"),
      json!({ "model": { "method": "fixed", "provider": "claude-code", "model": "claude-fable-5-1" } }),
    )
    .await;

    assert_eq!(status, 200, "{agent}: {fixed}");
    assert_eq!(fixed["pinnedProvider"], "claude-code", "{fixed}");
    assert_eq!(fixed["pinnedModel"], "claude-fable-5-1", "{fixed}");
  }

  // The registration form is taken by a worker as well.
  let (status, fixed) = patch(
    port,
    token,
    "/agents/helper",
    json!({ "model": { "method": "fixed", "registration": "local-model" } }),
  )
  .await;

  assert_eq!(status, 200, "{fixed}");
  assert_eq!(fixed["pinnedProvider"], "registered", "{fixed}");

  // A provider agency does not carry, a model the catalogue reads without naming, and a registration with no
  // address are each refused by their field, the pin left as it stood.
  let refusals = [
    (
      json!({ "provider": "nowhere", "model": "claude-fable-5-1" }),
      "model.provider",
      "nowhere",
    ),
    (
      json!({ "provider": "claude-code", "model": "claude-nowhere-1" }),
      "model.model",
      "claude-nowhere-1",
    ),
    (
      json!({ "provider": "registered", "model": "local-model" }),
      "model.model",
      "local-model",
    ),
  ];

  for (asked, field, named) in refusals {
    let mut model = asked.clone();

    model["method"] = json!("fixed");

    let (status, refused) = patch(port, token, "/agents/helper", json!({ "model": model })).await;

    assert_eq!(status, 400, "{asked}: {refused}");
    assert_eq!(refused["code"], "bad-request", "{refused}");
    assert_eq!(refused["field"], field, "{refused}");

    if field == "model.model" {
      assert!(refused["error"].as_str().unwrap_or_default().contains(named), "{refused}");
    }
  }

  let (_, agent) = get(port, token, "/agents/helper").await;

  assert_eq!(agent["pinnedProvider"], "registered", "{agent}");
  assert_eq!(agent["pinnedModel"], "local-model", "{agent}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pin_to_a_claude_model_is_kept_where_the_catalogue_cannot_be_read() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "label", "builtin") VALUES ('helper', 'worker', 'Helper', 0)"#,
      vec![],
    )
    .await;

  let (status, fixed) = patch(
    port,
    token,
    "/agents/helper",
    json!({ "model": { "method": "fixed", "provider": "claude-code", "model": "claude-fable-5-1" } }),
  )
  .await;

  assert_eq!(status, 200, "{fixed}");
  assert_eq!(fixed["pinnedModel"], "claude-fable-5-1", "{fixed}");
}
