//! What the brain reads of a running server over its control routes (docs/api.md, 「Agents」, 「Sessions」,
//! 「Workers」): every agent with whether it is awake and its brain folder, and nothing of a recollector;
//! the shared rulebooks by name; a work key's road; a verdict on an ask that is not there; and a reload
//! naming the agent and the rooms carried as published — on a store the server settled itself.

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
async fn a_running_server_answers_what_the_brain_reads() {
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

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;
  let (status, listed) = get(port, token, "/agents").await;

  assert_eq!(status, 200, "{listed}");

  let agent = |name: &str| {
    listed["items"]
      .as_array()
      .unwrap()
      .iter()
      .find(|agent| agent["name"] == name)
      .cloned()
      .unwrap_or_else(|| panic!("{name} is listed"))
  };
  let read = |agent: Value| (agent["awake"].clone(), agent["brainFolder"].clone());

  assert_eq!(read(agent("chat-agent")), (json!(true), json!("")));
  assert_eq!(read(agent("companion")), (json!(true), json!("agents/companion")));
  assert_eq!(read(agent("worker-one")), (json!(false), Value::Null));

  for agent in listed["items"].as_array().unwrap() {
    let fields = agent.as_object().unwrap();

    assert!(!fields.keys().any(|field| field.to_lowercase().contains("recollector")), "{agent}");
    assert_ne!(agent["kind"], json!("recollector"), "{agent}");
  }

  let (status, document) = get(port, token, "/prompt-documents/plain-writing").await;

  assert_eq!(status, 200, "{document}");
  assert_eq!(document["name"], json!("plain-writing"));
  assert_eq!(document["kind"], json!("rulebook"));
  assert_eq!(get(port, token, "/prompt-documents/no-such-rulebook").await.0, 404);

  assert_eq!(
    get(port, token, "/overtime-requests").await,
    (200, json!({ "items": [], "more": false }))
  );
  assert_eq!(
    patch(port, token, "/overtime-requests/1", json!({ "verdict": "cut", "why": "stuck" }))
      .await
      .0,
    404
  );
  assert_eq!(patch(port, token, "/overtime-requests/1", json!({ "verdict": "wait" })).await.0, 400);
  assert_eq!(get(port, token, "/work-keys/translator?agentName=companion").await.0, 200);

  let moved =
    json!({ "paths": ["agents/companion/people/a.md"], "now": false, "agent": "companion", "rooms": [-7] });
  let (status, answered) = post(port, token, "/document-moves", moved).await;

  assert_eq!(status, 201, "{answered}");
  assert_eq!(answered["paths"], json!(["agents/companion/people/a.md"]));
  assert_eq!(answered["agentName"], json!("companion"));
  assert_eq!(answered["roomIds"], json!([-7]));
  assert_eq!(post(port, token, "/document-moves", json!({ "paths": ["a.md"] })).await.0, 201);
}
