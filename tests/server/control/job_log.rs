//! A job's run log asked of a running server in the shapes the job unit reads and writes it: its lines read
//! by the job across its runs, a note written into a run's log becoming the job's last line, and a turn
//! record kept and read back under its run.

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
async fn a_running_server_reads_a_job_s_lines_and_keeps_its_notes_and_turn_records() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";
  let long = "x".repeat(2500);

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  // Job 12's lines across two runs: a call kept cut with its whole arguments in its detail, a call whose
  // arguments are no JSON, a branch's call under a parent, a result kept cut, the request lines and a
  // compaction.
  scratch
    .run(
      r#"INSERT INTO "agent_log"
           ("id", "agent", "run", "at", "kind", "name", "parent", "ref", "text", "detail", "job_id",
            "source", "source_id", "detail_complete", "detail_length")
         VALUES
           (1, 'helper', 'unit-a', 100, 'tool', 'Bash', NULL, 'call-1', '{"command":"car', '{"command":"cargo check"}', 12, 'claude-code', 'req-1', 1, 25),
           (2, 'helper', 'unit-a', 101, 'tool-result', NULL, NULL, 'call-1', 'ok', NULL, 12, 'claude-code', 'req-1', 0, 90000),
           (3, 'helper', 'unit-a', 102, 'text', NULL, NULL, NULL, 'thinking aloud', NULL, 12, 'claude-code', 'req-1', 1, 14),
           (4, 'helper', 'unit-b', 103, 'tool', 'Agent', 'call-0', 'call-2', '{}', NULL, 12, 'claude-code', 'req-2', 1, 2),
           (5, 'helper', 'unit-b', 104, 'thinking', NULL, NULL, NULL, 'hmm', NULL, 12, 'claude-code', 'req-3', 1, 3),
           (6, 'helper', 'unit-b', 105, 'tool', 'Read', NULL, 'call-3', $1, NULL, 12, NULL, 'req-3', 1, 2500),
           (7, 'helper', 'unit-b', 106, 'compact', NULL, NULL, NULL, 'compacted', NULL, 12, NULL, NULL, 1, 9),
           (8, 'other', 'unit-c', 107, 'tool', 'Bash', NULL, 'call-9', '{}', NULL, 13, NULL, 'req-9', 1, 2)"#,
      vec![long.clone().into()],
    )
    .await;
  scratch
    .run(r#"SELECT setval(pg_get_serial_sequence('agent_log', 'id'), 100)"#, vec![])
    .await;

  let (status, lines) = get(port, token, "/log/jobs/12").await;

  assert_eq!(status, 200, "{lines}");
  assert_eq!(
    lines,
    json!({
      "calls": [
        {
          "id": 1, "agent": "helper", "run": "unit-a", "reference": "call-1", "name": "Bash",
          "source": "claude-code", "at": 100, "command": "cargo check", "args": "{\"command\":\"cargo check\"}",
        },
        {
          "id": 6, "agent": "helper", "run": "unit-b", "reference": "call-3", "name": "Read",
          "source": null, "at": 105, "command": null, "args": "x".repeat(2000),
        },
      ],
      "results": [
        { "id": 2, "agent": "helper", "run": "unit-a", "reference": "call-1", "complete": false, "length": 90000, "at": 101 },
      ],
      "requests": 2,
      "compactions": 1,
    })
  );
  assert_eq!(
    get(port, token, "/log/jobs/12/last").await,
    (200, json!({ "id": 7, "at": 106, "kind": "compact", "name": null }))
  );

  // A job with no line: nothing held as its last line, and empty lists as its lines.
  let (status, absent) = get(port, token, "/log/jobs/99/last").await;

  assert_eq!((status, absent["code"].as_str()), (404, Some("nothing-held")), "{absent}");
  assert_eq!(
    get(port, token, "/log/jobs/99").await,
    (200, json!({ "calls": [], "results": [], "requests": 0, "compactions": 0 }))
  );
  assert_eq!(get(port, token, "/log/jobs/twelve").await.0, 400);

  // A note written under the job's run becomes its last line, kept whole with its road and state.
  let note = json!({
    "agent": "helper", "run": "unit-b", "chat": -7, "job": 12, "text": "claude-code · m",
    "source": "claude-code", "state": "running",
  });

  assert_eq!(post(port, token, "/log/notes", note.clone()).await.0, 204);

  let (_, last) = get(port, token, "/log/jobs/12/last").await;

  assert_eq!((&last["kind"], &last["name"]), (&json!("note"), &Value::Null), "{last}");
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "agent_log" WHERE "kind" = 'note' AND "agent" = 'helper' AND "run" = 'unit-b'
           AND "chat_id" = -7 AND "job_id" = 12 AND "text" = 'claude-code · m' AND "source" = 'claude-code'
           AND "state" = 'running'"#,
        vec![],
      )
      .await,
    1
  );

  let mut stateless = note.clone();

  stateless["state"] = json!("dancing");
  assert_eq!(post(port, token, "/log/notes", stateless).await.0, 400);
  assert_eq!(
    post(port, token, "/log/notes", json!({ "agent": "helper", "run": "unit-b" })).await.0,
    400
  );

  // A turn record kept under its run and read back.
  let record = json!({
    "run": "unit-b", "kind": "todo_add", "text": "A row filed", "recordId": 41, "wakes": false, "at": 5000,
  });

  assert_eq!(post(port, token, "/turn-records", record).await.0, 204);

  let (status, records) = get(port, token, "/run-records?run=unit-b").await;
  let kept = &records["records"][0];

  assert_eq!(status, 200, "{records}");
  assert_eq!(
    (&kept["kind"], &kept["text"], &kept["recordId"], &kept["wakes"], &kept["at"]),
    (&json!("todo_add"), &json!("A row filed"), &json!(41), &json!(0), &json!(5000))
  );
  assert_eq!(
    post(port, token, "/turn-records", json!({ "run": "unit-b", "kind": "todo_add" }))
      .await
      .0,
    400
  );
}
