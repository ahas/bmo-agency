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

  // A run noted on the worker's row.
  assert_eq!(post(port, token, "/workers/helper/runs", json!({})).await.0, 204);
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "agents" WHERE "name" = 'helper' AND "runs" = 1 AND "last_run_at" IS NOT NULL"#,
        vec![],
      )
      .await,
    1
  );

  // The bench made ready: the workbench stands, and a job before's request for its dev server is taken back.
  assert_eq!(post(port, token, "/workers/helper/bench", json!({})).await.0, 204);
  assert!(bench.is_dir() && !bench.join("dev-server.wanted").exists());
  assert_eq!(post(port, token, "/workers/nobody/bench", json!({})).await.0, 204);

  // Nothing listening is swept.
  assert_eq!(
    post(port, token, "/workers/helper/servers/sweep", json!({})).await,
    (200, json!({ "lines": [] }))
  );

  // The job documents: the graph text, the trailer with it and its paths filled in, and a handoff
  // instruction only where the worker's own folder, or its base's, holds one.
  let (status, documents) = get(port, token, "/workers/helper/job-documents").await;
  let graph = documents["graph"].as_str().unwrap_or_default();
  let trailer = documents["trailer"].as_str().unwrap_or_default();

  assert_eq!(status, 200, "{documents}");
  assert!(!graph.is_empty() && trailer.contains(graph), "{documents}");
  assert!(!trailer.contains("{{"), "every mark of the trailer is filled: {trailer}");
  assert!(trailer.contains("plain-writing.md"), "{trailer}");
  assert_eq!(documents["handoff"], Value::Null);

  let (_, support) = get(port, token, "/workers/helper-2/job-documents").await;

  assert!(support["handoff"].as_str().is_some_and(|text| !text.is_empty()), "{support}");

  // Which of a change's files a worker session's prefix is built from, in the order given.
  assert_eq!(
    post(
      port,
      token,
      "/workers/helper/prefix-touched",
      json!({ "files": ["src/main.rs", "prompts/agents/worker/plugin/skills/a.md", "CLAUDE.md"] }),
    )
    .await,
    (200, json!({ "touched": ["prompts/agents/worker/plugin/skills/a.md", "CLAUDE.md"] }))
  );

  // A handover naming no session with nothing kept on the model hands nothing over; a name that is no
  // worker's is refused.
  let handover = json!({ "model": "claude-opus-5-5", "cwd": home.path(), "thread": "todo:7", "session": null, "note": null });

  assert_eq!(post(port, token, "/workers/helper/sessions/handover", handover.clone()).await.0, 204);
  assert_eq!(post(port, token, "/workers/nobody/sessions/handover", handover.clone()).await.0, 400);

  // The session kept on the model is handed over, and its record dropped so the next turn opens fresh.
  let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;

  scratch
    .run(
      r#"INSERT INTO "worker_sessions" ("worker", "model", "account", "claude_session_id", "last_reply_at")
         VALUES ('helper', 'claude-opus-5-5', 'default', 'session-1', $1)"#,
      vec![now.into()],
    )
    .await;

  let (status, refused) = post(port, token, "/workers/helper/sessions/handover", handover.clone()).await;

  assert_eq!(status, 204, "{refused}");
  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "worker_sessions" WHERE "worker" = 'helper'"#, vec![])
      .await,
    0
  );
  assert_eq!(
    post(port, token, "/workers/helper/sessions/handover", json!({ "model": "m" })).await.0,
    400
  );
}
