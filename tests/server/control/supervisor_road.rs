//! Whether a stand-in answers, asked of a running server over its control routes: the stand-in route answers
//! whether one answers for a supervisor, and a session is started at no scope the contract does not name — on
//! a store the server settled itself.

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
async fn a_running_server_answers_whether_a_stand_in_answers_and_starts_no_editors_session() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  crate::support::rows::supervisor(scratch.db(), "chat-agent", &[]).await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  // Claude Code stands open and no stand-in is pinned, so none answers for the agent.
  assert_eq!(get(port, token, "/stand-in?agent=chat-agent").await.1["answers"], json!(false));

  let (status, refused) =
    post(port, token, "/sessions", json!({ "agentName": "digest", "scope": "editor" })).await;

  assert_eq!(status, 400, "{refused}");
  assert!(refused["error"].as_str().unwrap().contains("not editor"), "{refused}");
  assert_ne!(get(port, token, "/sessions/digest@editor").await.0, 200);
}
