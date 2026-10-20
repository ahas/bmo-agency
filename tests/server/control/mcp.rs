//! The harness plugin's MCP server as a harness starts it: `bmo-agency mcp` over pipes, its tools listed as
//! the OpenAPI document states them.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use crate::binary;

#[tokio::test(flavor = "multi_thread")]
async fn the_mcp_server_starts_and_lists_exactly_the_document_s_tools() {
  let binary = binary::built("agency-server", "bmo-agency");

  let listed = tokio::task::spawn_blocking(move || {
    let mut child = Command::new(binary)
      .arg("mcp")
      .env("AGENCY_URL", "http://127.0.0.1:1")
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

  assert_eq!(names, ["assign_worker", "run_job"]);
}
