//! A pane program's tool calls (docs/run-records.md, 「Agent log」): the progress lines bmo-tui writes as dsh
//! reports a call over the Agent Client Protocol — asked, under way, settled — filed as a `tool` line under
//! the tool's name and the `tool-result` that answers it, how it ended and how long it ran, matched first in,
//! first out under the turn's id and the call's place in it.

use agency_server::run_log::json::Json;
use agency_server::run_log::pane_calls::PaneCalls;
use serde_json::json;

fn progress(at: i64, fields: serde_json::Value) -> Json {
  let mut line = json!({ "kind": "progress", "id": "turn-7", "at": at });

  line.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
  Json::parse(&line.to_string()).unwrap()
}

#[test]
fn a_stand_in_stream_s_calls_are_filed_with_their_names_endings_and_times() {
  let mut calls = PaneCalls::default();
  let stream = [
    progress(1_000, json!({ "phase": "thinking", "tool": null })),
    progress(1_100, json!({ "phase": "tool", "tool": "bash", "state": "pending" })),
    progress(1_120, json!({ "phase": "tool", "tool": "bash", "state": "in_progress" })),
    progress(1_350, json!({ "phase": "tool", "tool": "tool", "state": "completed" })),
    progress(1_400, json!({ "context": { "used": 10, "size": 100 } })),
    progress(
      1_500,
      json!({ "phase": "tool", "tool": "mcp__bmo__memory_list", "state": "in_progress" }),
    ),
    progress(1_510, json!({ "phase": "tool", "tool": "read", "state": "in_progress" })),
    progress(1_700, json!({ "phase": "tool", "tool": "tool", "state": "failed" })),
    progress(1_900, json!({ "phase": "tool", "tool": "tool", "state": "completed" })),
    progress(2_000, json!({ "phase": "writing", "tool": null })),
  ];
  let lines: Vec<_> = stream.iter().flat_map(|event| calls.lines(event)).collect();
  let read: Vec<_> = lines
    .iter()
    .map(|line| {
      (
        line.kind,
        line.name.as_deref(),
        line.reference.as_deref(),
        line.state,
        line.text.as_str(),
      )
    })
    .collect();

  assert_eq!(
    read,
    [
      ("tool", Some("bash"), Some("turn-7:1"), Some("requested"), ""),
      ("tool-result", None, Some("turn-7:1"), Some("succeeded"), "completed in 250 ms"),
      ("tool", Some("mcp__bmo__memory_list"), Some("turn-7:2"), Some("requested"), ""),
      ("tool", Some("read"), Some("turn-7:3"), Some("requested"), ""),
      ("tool-result", None, Some("turn-7:2"), Some("failed"), "failed in 200 ms"),
      ("tool-result", None, Some("turn-7:3"), Some("succeeded"), "completed in 390 ms"),
    ]
  );
  assert_eq!(calls.made(), 3);
}

#[test]
fn a_settling_with_no_call_open_and_an_unnamed_start_file_nothing() {
  let mut calls = PaneCalls::default();

  assert!(
    calls
      .lines(&progress(1, json!({ "phase": "tool", "tool": "tool", "state": "completed" })))
      .is_empty()
  );
  assert!(
    calls
      .lines(&progress(2, json!({ "phase": "tool", "tool": "tool", "state": "in_progress" })))
      .is_empty()
  );
  assert_eq!(calls.made(), 0);
}
