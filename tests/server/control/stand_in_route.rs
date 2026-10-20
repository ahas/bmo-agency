//! Whether a stand-in answers, and an editor's session, asked of a running server over its control routes:
//! `GET /stand-in` answers for the agent named and refuses a call naming none, and an editor's session is
//! started, read and refused as the session contract says, on a store the server settled itself.

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
async fn a_running_server_answers_whether_a_stand_in_answers_and_carries_an_editors_session() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(r#"INSERT INTO "agents" ("name", "kind") VALUES ('chat-agent', 'chat')"#, vec![])
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  // Claude Code stands open and no stand-in is pinned, so none answers for the agent.
  assert_eq!(get(port, token, "/agents/chat-agent").await.1["standInAvailable"], json!(false));

  let (status, started) =
    post(port, token, "/sessions", json!({ "agentName": "digest", "scope": "editor" })).await;

  assert_eq!(status, 201, "{started}");
  assert_eq!(started["id"], "digest@editor");
  assert_eq!(started["scope"], json!("editor"));

  let (status, refused) =
    post(port, token, "/sessions", json!({ "agentName": "worker", "scope": "editor" })).await;

  assert_eq!((status, refused["code"].as_str()), (422, Some("backend-refused")));

  // The brain under the server's data root is no repository, so the session's state says why.
  let (status, state) = get(port, token, "/sessions/digest@editor").await;

  assert_eq!(status, 422, "{state}");
  assert!(state["error"].as_str().unwrap().contains("no repository"), "{state}");

  std::fs::create_dir_all(data.join("brain").join(".git")).unwrap();

  let (status, state) = get(port, token, "/sessions/digest@editor").await;

  assert_eq!(status, 200, "{state}");
  assert_eq!(state["provider"], "claude-code");
  assert_eq!(state["place"], data.join("brain").to_string_lossy().as_ref());
}
