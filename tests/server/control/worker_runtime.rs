//! What a job's run asks of its worker's runtime, asked of a running server in the shapes the job unit sends:
//! a run noted on the row, the bench made ready with a job before's dev-server request taken back, the
//! servers swept, the job documents read with the trailer's marks filled and a support worker's handoff
//! instruction its base's, the files of a change that touch the prefix, and a session handed over — the
//! kept one on the model with its record dropped where none is named, nothing where nothing is kept.

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
async fn a_running_server_answers_what_a_job_s_run_asks_of_its_worker() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";
  let bench = data.join("workbench").join("helper");

  std::fs::create_dir_all(&bench).unwrap();
  std::fs::write(bench.join("dev-server.wanted"), "").unwrap();
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "support_of")
         VALUES ('helper', 'worker', NULL), ('helper-2', 'worker', 'companion')"#,
      vec![],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  // The bench made ready: the workbench stands, and a job before's request for its dev server is taken back.
  assert_eq!(patch(port, token, "/workers/helper", json!({ "bench": true })).await.0, 200);
  assert!(bench.is_dir() && !bench.join("dev-server.wanted").exists());

  // Nothing listening is swept.
  assert_eq!(delete(port, token, "/workers/helper/servers").await, (200, json!({ "lines": [] })));

  // The job documents: the graph text, the trailer with it and its paths filled in, and a handoff
  // instruction only where the worker's own folder, or its base's, holds one.
  let (status, documents) = get(port, token, "/workers/helper/definition").await;
  let graph = documents["graph"].as_str().unwrap_or_default();
  let trailer = documents["trailer"].as_str().unwrap_or_default();

  assert_eq!(status, 200, "{documents}");
  assert!(!graph.is_empty() && trailer.contains(graph), "{documents}");
  assert!(!trailer.contains("{{"), "every mark of the trailer is filled: {trailer}");
  assert!(trailer.contains("plain-writing.md"), "{trailer}");
  assert_eq!(documents["handoff"], Value::Null);

  let (_, support) = get(port, token, "/workers/helper-2/definition").await;

  assert!(support["handoff"].as_str().is_some_and(|text| !text.is_empty()), "{support}");

  // Which of a change's files a worker session's prefix is built from, in the order given.
  assert_eq!(
    post(
      port,
      token,
      "/prefix-checks",
      json!({ "workerName": "helper", "files": ["src/main.rs", "prompts/agents/worker/plugin/skills/a.md", "CLAUDE.md"] }),
    )
    .await,
    (200, json!({ "touched": ["prompts/agents/worker/plugin/skills/a.md", "CLAUDE.md"] }))
  );

  // A session handed to another holder answers the session it then has.
  let (status, session) =
    patch(port, token, "/sessions/helper@worker", json!({ "holder": "runner" })).await;

  assert_eq!(status, 200, "{session}");
  assert_eq!(session["id"], json!("helper@worker"));
  assert_eq!(session["holder"], json!("runner"));
}
