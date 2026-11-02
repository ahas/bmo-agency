//! The old archive's move into agency's store (store/src/import.rs, TODO #1923): the mapping, the
//! folded tables, the refusal of a store already holding rows, and the count-and-sample compare, over
//! a small export written into a temporary folder and a migrated scratch database.

use std::fs;
use std::path::PathBuf;

use crate::support::scratch::Scratch;
use serde_json::{Value, json};
use store::import::{self, Cell};

/// A small export in the shape the item's export hand writes: every table the import knows, with the
/// rows given and none elsewhere.
fn fixture() -> (tempfile::TempDir, PathBuf) {
  let dir = tempfile::tempdir().expect("a temporary folder");
  let rows: Vec<(&str, Vec<Value>, Vec<&str>)> = vec![
    (
      "agents",
      vec![
        json!({"name": "bmo", "kind": "chat", "keepalive_stopped": 0, "runs": 0, "builtin": 0, "downgrade_heal": 1, "support_count": 0, "support_keepalive": 0, "default_answerer": 1, "last_run_at": 100}),
      ],
      vec!["name"],
    ),
    (
      "jobs",
      vec![
        json!({"id": 1, "todo_id": 10, "worker": "worker", "unit": "u", "chat_id": 5, "ok": 1, "outcome": null, "report": "r", "launched_at": 100, "finished_at": 200, "prompt": "p", "prompt_english": null, "model": "m"}),
      ],
      vec!["id"],
    ),
    (
      "runs",
      vec![
        json!({"id": 1, "agent": "bmo", "chat_id": 5, "unit": "u", "depth": "basic", "cap_ms": 1000, "started_at": 100, "finished_at": 200, "outcome": "done", "started_by": null, "context_tokens": null, "search_id": null}),
      ],
      vec!["id"],
    ),
    (
      "usage_windows",
      vec![
        json!({"name": "w", "account": "", "mode": "m", "utilization": 0.5, "resets_at": 100, "seen_at": 50, "duration_mins": 60, "limit_name": null}),
      ],
      vec!["mode", "account", "name"],
    ),
    (
      "clock_holds",
      vec![
        json!({"id": 1, "run": "r", "agent": "a", "what": "w", "clock": "c", "clock_ms": 1000, "started_at": 1000, "last_activity_at": 2000, "last_activity": "x", "asked_at": 3000, "answer_by": 4000, "instruction_id": 1, "verdict": "cut", "keep_until": null, "decided_by": "bot", "why": "y", "decided_at": 5000, "closed": "cut", "closed_at": 5000}),
      ],
      vec!["id"],
    ),
    (
      "session_work_items",
      vec![json!({"session_id": "s", "todo_id": 7, "served_at": 100})],
      vec!["session_id"],
    ),
    (
      "session_work_results",
      vec![
        json!({"session_id": "s", "result": "ok", "version": "1", "loaded_at": 110}),
        json!({"session_id": "s", "result": "two", "version": "2", "loaded_at": 120}),
      ],
      vec!["session_id"],
    ),
    ("settings", vec![json!({"key": "provider_order", "value": "x"})], vec!["key"]),
  ];

  let mut sources: Vec<&str> = import::TABLES.iter().map(|table| table.source).collect();
  sources.extend(["jobs", "clock_holds", "session_work_items", "session_work_results"]);

  let mut manifest = Vec::new();
  for source in sources {
    let (held, key): (Vec<Value>, Vec<&str>) = rows
      .iter()
      .find(|(held, _, _)| *held == source)
      .map(|(_, rows, key)| (rows.clone(), key.clone()))
      .unwrap_or_else(|| (Vec::new(), vec!["id"]));
    let file = format!("{source}.ndjson");
    let mut body = String::new();
    for row in &held {
      body.push_str(&row.to_string());
      body.push('\n');
    }
    fs::write(dir.path().join(&file), body).expect("the export file");
    manifest.push(json!({
      "source": source,
      "file": file,
      "count": held.len(),
      "columns": [],
      "key": key
    }));
  }
  fs::write(
    dir.path().join("manifest.json"),
    json!({
      "unit": "agency",
      "snapshot": "1:1:",
      "readAt": "2026-10-21 17:00:00+09",
      "readOnly": true,
      "tables": manifest
    })
    .to_string(),
  )
  .expect("the manifest");

  let path = dir.path().to_path_buf();
  (dir, path)
}

#[test]
fn the_old_job_is_the_harness_s_job_and_a_second_is_a_millisecond() {
  let row = import::job(
    &json!({"id": 1, "todo_id": 10, "worker": "worker", "unit": "u", "chat_id": 5, "ok": 1, "outcome": null, "report": "r", "launched_at": 100, "finished_at": 200, "prompt": "p", "prompt_english": null, "model": "m"}),
  );
  let get = |column: &str| {
    row
      .iter()
      .find(|(held, _)| *held == column)
      .map(|(_, cell)| cell.clone())
      .expect("a mapped column")
  };
  assert_eq!(get("item_id"), Cell::Int(10));
  assert_eq!(get("room_id"), Cell::Int(5));
  assert_eq!(get("state"), Cell::Text("ended".to_owned()));
  assert_eq!(get("started_at"), Cell::Int(100_000));
  assert_eq!(get("ended_at"), Cell::Int(200_000));
}

#[test]
fn a_clock_hold_becomes_its_clock_and_its_ask() {
  let (clock, ask) = import::clock(
    &json!({"id": 1, "run": "r", "agent": "a", "what": "w", "clock": "c", "clock_ms": 1000, "started_at": 1000, "last_activity_at": 2000, "last_activity": "x", "asked_at": 3000, "answer_by": 4000, "instruction_id": 1, "verdict": "cut", "keep_until": null, "decided_by": "bot", "why": "y", "decided_at": 5000, "closed": "cut", "closed_at": 5000}),
  );
  let get = |row: &Vec<(&'static str, Cell)>, column: &str| {
    row
      .iter()
      .find(|(held, _)| *held == column)
      .map(|(_, cell)| cell.clone())
      .expect("a mapped column")
  };
  assert_eq!(get(&clock, "run"), Cell::Text("r".to_owned()));
  assert_eq!(get(&clock, "bound_ms"), Cell::Int(1000));
  assert_eq!(get(&ask, "asked_at"), Cell::Int(3000));
}

#[tokio::test]
async fn the_import_writes_its_rows_refuses_a_second_run_and_the_compare_finds_a_changed_row() {
  let (_folder, export) = fixture();
  let scratch = Scratch::new().await;
  let address = scratch.address();

  let report = import::run(&address, &export, false).await.expect("the import");
  assert_eq!(report.snapshot, "1:1:");
  assert!(report.sample.failures.is_empty(), "{:?}", report.sample.failures);

  assert_eq!(scratch.count("SELECT COUNT(*) FROM \"jobs\"", vec![]).await, 1);
  assert_eq!(scratch.count("SELECT COUNT(*) FROM \"run_clocks\"", vec![]).await, 1);
  assert_eq!(scratch.count("SELECT COUNT(*) FROM \"clock_asks\"", vec![]).await, 1);
  assert_eq!(scratch.count("SELECT COUNT(*) FROM \"session_work\"", vec![]).await, 1);
  assert_eq!(
    scratch.count(
      "SELECT jsonb_array_length(\"results\"::jsonb)::bigint FROM \"session_work\" WHERE \"session_id\" = 's'",
      vec![]
    )
    .await,
    2
  );
  assert_eq!(
    scratch.count("SELECT \"depth\"::bigint FROM \"runs\" WHERE \"id\" = 1", vec![]).await,
    0
  );

  let again = import::run(&address, &export, false).await;
  assert!(again.is_err(), "a store holding rows is refused");
  assert!(again.unwrap_err().contains("already holds rows"));

  let passes = import::verify(&address, &export).await.expect("the compare");
  assert!(passes.failures.is_empty(), "{:?}", passes.failures);

  scratch
    .run("UPDATE \"agents\" SET \"kind\" = 'changed' WHERE \"name\" = 'bmo'", vec![])
    .await;
  let changed = import::verify(&address, &export).await.expect("the compare");
  assert!(
    changed.failures.iter().any(|failure| failure.contains("kind")),
    "{:?}",
    changed.failures
  );
}

#[tokio::test]
async fn a_dry_run_writes_nothing() {
  let (_folder, export) = fixture();
  let scratch = Scratch::new().await;
  let address = scratch.address();

  let report = import::run(&address, &export, true).await.expect("the dry run");
  assert!(report.dry_run);
  assert_eq!(scratch.count("SELECT COUNT(*) FROM \"jobs\"", vec![]).await, 0);
}
