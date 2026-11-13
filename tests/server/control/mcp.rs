//! The harness plugin's MCP server as a harness starts it: `bmo-agency mcp` over pipes, its tools listed as
//! the OpenAPI document states them.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::binary;

#[tokio::test(flavor = "multi_thread")]
async fn the_mcp_server_starts_and_lists_exactly_the_document_s_tools() {
  let binary = binary::path("bmo-agency");

  let listed = tokio::task::spawn_blocking(move || {
    let mut child = Command::new(binary)
      .args(["mcp", "--agent", "chat-agent"])
      .env("AGENCY_URL", "http://[::1]:1")
      .env("AGENCY_CONTROL_TOKEN", "secret")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .expect("the plugin starts");
    let mut input = child.stdin.take().expect("its standard input");
    let mut output = BufReader::new(child.stdout.take().expect("its standard output"));

    for request in [
      json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
          "protocolVersion": "2025-06-18",
          "capabilities": {},
          "clientInfo": { "name": "test", "version": "1" },
        },
      }),
      json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
      json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
    ] {
      writeln!(input, "{request}").expect("the request is written");
      input.flush().expect("the request is flushed");
    }

    let mut names = Vec::new();

    for _ in 0..20 {
      let mut line = String::new();

      if output.read_line(&mut line).unwrap_or(0) == 0 {
        break;
      }
      let Ok(answer) = serde_json::from_str::<Value>(&line) else { continue };

      if answer["id"] == json!(2) {
        names = answer["result"]["tools"]
          .as_array()
          .expect("tools are listed")
          .iter()
          .map(|tool| tool["name"].as_str().expect("a tool has a name").to_owned())
          .collect();
        break;
      }
    }

    let _ = child.kill();
    let _ = child.wait();

    names
  });

  let names = tokio::time::timeout(Duration::from_secs(30), listed)
    .await
    .expect("the plugin answers")
    .expect("the reader runs");

  // A server started for an agent files that agent's items.
  assert_eq!(
    names,
    ["file_item", "stop_job", "review_job", "assign_worker", "release_worker", "run_job"]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_agent_s_run_job_on_a_turn_not_the_owner_s_is_refused_where_the_check_cannot_be_made() {
  let binary = binary::path("bmo-agency");
  let root = tempfile::tempdir().unwrap();
  let place = root.path().join("place");

  std::fs::create_dir_all(&place).unwrap();
  std::fs::write(
    place.join("run.json"),
    json!({ "name": "chat:-7:1", "person": false,
      "env": { "BMO_TURN_CHAT": "-7", "BMO_TURN_MESSAGES": "11", "BMO_TURN_PRIVILEGED": "0" } })
    .to_string(),
  )
  .unwrap();

  let data = root.path().to_path_buf();
  let answered = tokio::task::spawn_blocking(move || {
    let mut child = Command::new(binary)
      .args(["mcp", "--agent", "chat-agent"])
      .env("AGENCY_URL", "http://[::1]:1")
      .env("AGENCY_CONTROL_TOKEN", "secret")
      .env("BMO_AGENT_PLACE", &place)
      .env("BMO_DATA_DIR", &data)
      .env_remove("WEB_ADDRESS")
      .env_remove("WEB_TOKEN")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::null())
      .spawn()
      .expect("the plugin starts");
    let mut input = child.stdin.take().expect("its standard input");
    let mut output = BufReader::new(child.stdout.take().expect("its standard output"));
    let run = json!({
      "assignmentId": 1, "itemId": 2, "leaseId": 3, "holderName": "chat-agent",
      "item": { "title": "t", "brief": "b", "workOrder": null, "notes": [], "labels": {}, "scope": null },
    });

    for request in [
      json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
          "protocolVersion": "2025-06-18",
          "capabilities": {},
          "clientInfo": { "name": "test", "version": "1" },
        },
      }),
      json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
      json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "run_job", "arguments": run } }),
    ] {
      writeln!(input, "{request}").expect("the request is written");
      input.flush().expect("the request is flushed");
    }

    let mut answer = Value::Null;

    for _ in 0..20 {
      let mut line = String::new();

      if output.read_line(&mut line).unwrap_or(0) == 0 {
        break;
      }
      let Ok(read) = serde_json::from_str::<Value>(&line) else { continue };

      if read["id"] == json!(2) {
        answer = read;
        break;
      }
    }

    let _ = child.kill();
    let _ = child.wait();

    answer
  });

  let answer = tokio::time::timeout(Duration::from_secs(30), answered)
    .await
    .expect("the plugin answers")
    .expect("the reader runs");
  let text = answer.to_string();

  // Refused before agency is asked: the address it names answers nothing, and the refusal is the gate's.
  assert!(text.contains("not-permitted"), "{text}");
  assert!(text.contains("Running a job is refused"), "{text}");
}
