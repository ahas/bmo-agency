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

  let (status, lines) = get(port, token, "/log-lines?jobId=12&limit=2").await;

  assert_eq!(status, 200, "{lines}");
  assert_eq!(lines["items"].as_array().unwrap().len(), 2);
  assert_eq!(lines["items"][0]["id"], json!(1));
  assert_eq!(lines["items"][0]["agentName"], json!("helper"));
  assert_eq!(lines["items"][0]["jobId"], json!(12));
  assert_eq!(lines["items"][0]["kind"], json!("tool"));
  assert_eq!(lines["items"][1]["id"], json!(2));

  // A job with no line: nothing held.
  assert_eq!(get(port, token, "/log-lines").await.0, 400);

  // A note written under the job's run becomes its line, kept whole with its road and state.
  let note = json!({
    "agent": "helper", "run": "unit-b", "chat": -7, "job": 12, "text": "claude-code · m",
    "kind": "note", "source": "claude-code", "state": "running",
  });

  assert_eq!(post(port, token, "/log-lines", note.clone()).await.0, 201);

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
  assert_eq!(post(port, token, "/log-lines", stateless).await.0, 400);
  assert_eq!(
    post(port, token, "/log-lines", json!({ "agent": "helper", "run": "unit-b" })).await.0,
    400
  );

  // The note is read back as a line of its run.
  let (status, kept) = get(port, token, "/log-lines?run=unit-b").await;
  let note = kept["items"]
    .as_array()
    .unwrap()
    .iter()
    .find(|line| line["kind"] == "note")
    .expect("the note is kept");

  assert_eq!(status, 200, "{kept}");
  assert_eq!(note["text"], json!("claude-code · m"));
  assert_eq!(note["jobId"], json!(12));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_s_log_reads_across_its_runs_newest_before_after_and_at_a_moment() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  // Five lines in one run and five in the next, in order, and another agent's line between.
  scratch
    .run(
      r#"INSERT INTO "agent_log" ("id", "agent", "run", "at", "kind", "text")
         VALUES
           (1, 'worker', 'unit-a', 100, 'text', 'a1'),
           (2, 'worker', 'unit-a', 101, 'text', 'a2'),
           (3, 'worker', 'unit-a', 102, 'text', 'a3'),
           (4, 'worker', 'unit-a', 103, 'text', 'a4'),
           (5, 'worker', 'unit-a', 104, 'text', 'a5'),
           (6, 'other', 'unit-x', 105, 'text', 'x1'),
           (7, 'worker', 'unit-b', 106, 'text', 'b1'),
           (8, 'worker', 'unit-b', 107, 'text', 'b2'),
           (9, 'worker', 'unit-b', 108, 'text', 'b3'),
           (10, 'worker', 'unit-b', 109, 'text', 'b4'),
           (11, 'worker', 'unit-b', 110, 'text', 'b5')"#,
      vec![],
    )
    .await;

  // The newest page: the last three lines, oldest first, with another page older.
  let (status, newest) = get(port, token, "/log/agents/worker/lines?limit=3").await;

  assert_eq!(status, 200, "{newest}");
  assert_eq!(
    newest["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|line| line["id"].as_i64().unwrap())
      .collect::<Vec<_>>(),
    [9, 10, 11]
  );
  assert_eq!(newest["hasMore"], json!(true));
  assert_eq!(newest["hasOlder"], json!(false));
  assert_eq!(newest["items"][0]["fields"]["run"], json!("unit-b"));

  // The page before the oldest of those: three more, oldest first, still across the next run.
  let (status, older) = get(port, token, "/log/agents/worker/lines?limit=3&before=9").await;

  assert_eq!(status, 200, "{older}");
  assert_eq!(
    older["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|line| line["id"].as_i64().unwrap())
      .collect::<Vec<_>>(),
    [5, 7, 8]
  );
  assert_eq!(older["hasMore"], json!(true));
  assert!(
    older["items"]
      .as_array()
      .unwrap()
      .iter()
      .any(|line| line["fields"]["run"] == json!("unit-a")),
    "the page reaches back into the run before"
  );

  // The page after an id reads forward; the last of the log has no more.
  let (status, forward) = get(port, token, "/log/agents/worker/lines?limit=10&after=7").await;

  assert_eq!(status, 200, "{forward}");
  assert_eq!(
    forward["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|line| line["id"].as_i64().unwrap())
      .collect::<Vec<_>>(),
    [8, 9, 10, 11]
  );
  assert_eq!(forward["hasMore"], json!(false));

  // The page standing at a moment opens at the head of the run its line belongs to, and says older lines
  // stand before it and newer ones after.
  let (status, at) = get(port, token, "/log/agents/worker/lines?limit=3&at=107").await;

  assert_eq!(status, 200, "{at}");
  assert_eq!(at["hasOlder"], json!(true));
  assert_eq!(at["hasMore"], json!(true));
  assert_eq!(at["items"][0]["id"], json!(7));

  // An agent with no line reads an empty page.
  let (status, none) = get(port, token, "/log/agents/no-such-agent/lines?limit=3").await;

  assert_eq!((status, none["items"].as_array().unwrap().len()), (200, 0));
}
