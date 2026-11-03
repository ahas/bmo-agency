//! The old archive's move into agency's store (docs/store.md, and the migration plan's table map for
//! TODO #1923): one export directory read, this store written, nothing else touched.
//!
//! The export is the one the item's export hand wrote under
//! `workbench/worker/items/1923/exports/agency/`: `manifest.json` with each source table's exact count
//! and key, and one NDJSON file a table. The import reads only that directory, refuses a store already
//! holding rows of its tables, and runs whole inside one transaction, so a dry run and a failed run
//! leave the store as it stood.
//!
//! Most tables keep their old name and columns. The three that do not:
//!
//! - `clock_holds` becomes `run_clocks` and `clock_asks`: the run's clock, and the ask past it.
//! - `session_work_items` and `session_work_results` become `session_work`, one row a session, its
//!   latest item and every result it holds.
//! - the old `jobs` is the per-run record on a worker; the target's `jobs` is the harness's handover,
//!   so the item, the lease and the assignment fields the harness names are filled where the old row
//!   has them and zero where it does not.
//!
//! Agency's columns are epoch milliseconds; the old archive kept a second in most of its tables and a
//! millisecond in the co-worker pool's and the clocks'. Each source column that held a second is
//! multiplied as it is written; `usage_windows` alone keeps seconds, as its own writer does.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use sea_orm::{ConnectOptions, Database};
use sea_orm_migration::MigratorTrait;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::migration::{Migrator, TWO_AXES};

/// The store's own migrations, run over the address: those before the two axes where `before` says so, since
/// the export is in the shape the agent rows stood in, else every one left, which carries what was written.
async fn migrate(address: &str, before: bool) -> Result<(), String> {
  let mut options = ConnectOptions::new(address.to_owned());
  options.sqlx_logging(false);
  let db = Database::connect(options)
    .await
    .map_err(|error| format!("the migrations' connection: {error}"))?;
  let steps = if before {
    let pending = Migrator::get_pending_migrations(&db)
      .await
      .map_err(|error| format!("the migrations: {error}"))?;

    Some(pending.iter().take_while(|pending| pending.name() != TWO_AXES).count() as u32)
  } else {
    None
  };

  if steps != Some(0) {
    Migrator::up(&db, steps).await.map_err(|error| format!("the migrations: {error}"))?;
  }
  db.close().await.map_err(|error| format!("the migrations' connection: {error}"))?;

  Ok(())
}

/// One value written to the store, already in the target's shape.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
  Null,
  Int(i64),
  Text(String),
}

/// One mapped row: the target's columns and the cell each takes.
pub type Mapped = Vec<(&'static str, Cell)>;
/// The rows of one table, in the order they are written.
pub type MappedRows = Vec<Mapped>;

/// One table of the report.
#[derive(Clone, Debug, PartialEq)]
pub struct Moved {
  pub source: String,
  pub target: String,
  pub exported: u64,
  pub written: u64,
}

/// What an import run did.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
  pub dry_run: bool,
  pub snapshot: String,
  pub read_at: String,
  pub tables: Vec<Moved>,
  pub sample: Sample,
}

/// The count-and-sample compare's result.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
  pub checked: usize,
  pub failures: Vec<String>,
}

/// One export table's entry in the manifest.
#[derive(Clone, Debug)]
pub struct TableEntry {
  pub file: String,
  pub count: u64,
  pub key: Vec<String>,
}

/// The export directory's manifest.
#[derive(Clone, Debug)]
pub struct Manifest {
  pub snapshot: String,
  pub read_at: String,
  pub tables: HashMap<String, TableEntry>,
}

/// A source table's target, its target columns, and the columns that change on the way.
pub struct TableMap {
  pub source: &'static str,
  pub target: &'static str,
  pub columns: &'static [&'static str],
  /// A target column read from a differently named source column.
  pub renamed: &'static [(&'static str, &'static str)],
  /// Source columns held in seconds and written in milliseconds.
  pub seconds: &'static [&'static str],
  /// A target column the old row has no number for, written as this.
  pub fixed: &'static [(&'static str, i64)],
}

const fn same(source: &'static str, target: &'static str, columns: &'static [&'static str]) -> TableMap {
  TableMap {
    source,
    target,
    columns,
    renamed: &[],
    seconds: &[],
    fixed: &[],
  }
}

const fn seconds(
  source: &'static str,
  target: &'static str,
  columns: &'static [&'static str],
  held: &'static [&'static str],
) -> TableMap {
  TableMap {
    source,
    target,
    columns,
    renamed: &[],
    seconds: held,
    fixed: &[],
  }
}

/// Every table this import moves with nothing folded, in the order it writes them.
pub const TABLES: &[TableMap] = &[
  same("agent_base_prompts", "agent_base_prompts", &["agent", "prompt"]),
  same("agent_executions", "agent_executions", &["id", "body"]),
  seconds(
    "agent_log",
    "agent_log",
    &[
      "id",
      "agent",
      "run",
      "chat_id",
      "at",
      "kind",
      "name",
      "parent",
      "ref",
      "truncated",
      "text",
      "job_id",
      "sequence",
      "source",
      "source_event",
      "source_block",
      "source_id",
      "state",
      "upstream_truncated",
      "detail_complete",
      "detail_length",
      "metadata_length",
      "detail",
      "metadata",
    ],
    &["at"],
  ),
  seconds(
    "agents",
    "agents",
    &[
      "name",
      "kind",
      "account",
      "last_mode",
      "last_model",
      "last_effort",
      "chat_id",
      "session_id",
      "last_run_at",
      "last_keepalive_at",
      "keepalive_stopped",
      "runs",
      "keepalive_paused_at",
      "context_tokens",
      "prompt_changed_at",
      "label",
      "aliases",
      "icon",
      "cwd",
      "builtin",
      "preamble",
      "dev_command",
      "downgrade_heal",
      "support_count",
      "support_keepalive",
      "support_of",
      "support_slot",
      "pinned_provider",
      "item_worker",
      "herdr_label",
      "resting_since",
      "pinned_model",
      "thinking",
      "effort",
      "brain_folder",
      "voice",
      "window_seconds",
      "wake_words",
      "node",
      "role",
      "session_scope",
      "turn_shape",
      "default_answerer",
    ],
    &[
      "last_run_at",
      "last_keepalive_at",
      "keepalive_paused_at",
      "prompt_changed_at",
      "resting_since",
    ],
  ),
  seconds(
    "base_prompts",
    "base_prompts",
    &["name", "label", "text", "updated_at", "updated_by"],
    &["updated_at"],
  ),
  seconds(
    "branch_agents",
    "branch_agents",
    &["pool", "slot", "mode", "model", "role", "upstream", "prefix_digest", "last_reply_at"],
    &["last_reply_at"],
  ),
  seconds(
    "claude_accounts",
    "claude_accounts",
    &[
      "id",
      "name",
      "place",
      "enabled",
      "credential_kind",
      "credential_path",
      "added_at",
      "needs_login_at",
      "user_id",
    ],
    &["added_at", "needs_login_at"],
  ),
  seconds(
    "companion_settings",
    "companion_settings",
    &[
      "agent",
      "reference_on",
      "pieces",
      "own_pieces",
      "persona",
      "tools",
      "updated_at",
      "updated_by",
      "generation",
    ],
    &["updated_at"],
  ),
  same(
    "coworker_pool_limits",
    "coworker_pool_limits",
    &["provider", "model", "effort", "capacity", "label", "set_at"],
  ),
  same("coworker_pool_meta", "coworker_pool_meta", &["version"]),
  same(
    "coworker_pool_native",
    "coworker_pool_native",
    &[
      "token",
      "session",
      "tool_use_id",
      "waiter_pid",
      "waiter_start",
      "owner_pid",
      "owner_start",
      "agent_id",
    ],
  ),
  same(
    "coworker_pool_native_reservations",
    "coworker_pool_native_reservations",
    &["token", "owner_pid", "owner_start"],
  ),
  same("coworker_pool_native_stops", "coworker_pool_native_stops", &["agent_id", "at"]),
  same(
    "coworker_pool_pings",
    "coworker_pool_pings",
    &["reservation", "place", "provider", "model", "at", "outcome", "answered_at"],
  ),
  same(
    "coworker_pool_place_writes",
    "coworker_pool_place_writes",
    &["place", "provider", "model", "effort", "upstream", "written_at"],
  ),
  same(
    "coworker_pool_requests",
    "coworker_pool_requests",
    &[
      "sequence",
      "token",
      "request_key",
      "owner",
      "source",
      "role",
      "work_key",
      "provider",
      "model",
      "effort",
      "tier",
      "capacity",
      "generation",
      "state",
      "enqueued_at",
      "leased_at",
      "child_id",
      "ended_at",
      "outcome",
      "reservation",
      "class",
      "place_key",
      "turn",
    ],
  ),
  same("coworker_pool_reservation_meta", "coworker_pool_reservation_meta", &["version"]),
  same(
    "coworker_pool_reservations",
    "coworker_pool_reservations",
    &[
      "token",
      "reservation_key",
      "owner",
      "source",
      "role",
      "work_key",
      "provider",
      "model",
      "effort",
      "tier",
      "wanted",
      "state",
      "created_at",
      "ended_at",
      "outcome",
    ],
  ),
  same(
    "coworker_pool_roles",
    "coworker_pool_roles",
    &["name", "description", "prompt", "tools", "skills", "enabled", "pooled", "set_at"],
  ),
  same(
    "coworker_pool_tier_limits",
    "coworker_pool_tier_limits",
    &["provider", "tier", "capacity", "set_at"],
  ),
  same("coworker_pool_tier_meta", "coworker_pool_tier_meta", &["version"]),
  same("execution_attempts", "execution_attempts", &["root", "attempt"]),
  seconds(
    "fallback_routes",
    "fallback_routes",
    &[
      "key",
      "account",
      "claude_model",
      "codex_model",
      "codex_effort",
      "openrouter_model",
      "set_at",
      "route_mode",
      "forced",
    ],
    &["set_at"],
  ),
  seconds(
    "fallback_sessions",
    "fallback_sessions",
    &["scope", "key", "mode", "model", "effort", "session_id", "last_reply_at", "run_json"],
    &["last_reply_at"],
  ),
  same(
    "live_claude_openings",
    "live_claude_openings",
    &["id", "job_id", "todo_id", "purpose", "opened_at"],
  ),
  same("live_claude_runs", "live_claude_runs", &["id", "opening_id", "what", "ran_at"]),
  seconds(
    "model_catalogue",
    "model_catalogue",
    &[
      "name",
      "label",
      "file",
      "bytes",
      "sha256",
      "source",
      "engine",
      "registration",
      "nodes",
      "thinking",
      "temperature",
      "top_p",
      "presence_penalty",
      "max_tokens",
      "note",
      "added_at",
    ],
    &["added_at"],
  ),
  seconds(
    "model_fallbacks",
    "model_fallbacks",
    &["model", "openrouter_model", "set_at", "forced"],
    &["set_at"],
  ),
  same("model_tier_assignments", "model_tier_assignments", &["key", "tier"]),
  same("model_tier_backfilled", "model_tier_backfilled", &["key"]),
  same(
    "model_tier_conflicts",
    "model_tier_conflicts",
    &["key", "provider", "previous", "accepted"],
  ),
  same("model_tier_legacy", "model_tier_legacy", &["key", "body"]),
  same(
    "model_tier_mappings",
    "model_tier_mappings",
    &["provider", "tier", "model", "effort", "answer_tokens"],
  ),
  same("model_tier_meta", "model_tier_meta", &["version"]),
  same("model_tier_seeded", "model_tier_seeded", &["provider"]),
  seconds(
    "model_uses",
    "model_uses",
    &[
      "id",
      "entry",
      "engine",
      "chosen_by",
      "started_at",
      "ended_at",
      "state",
      "step",
      "reason",
      "log",
    ],
    &["started_at", "ended_at"],
  ),
  seconds(
    "node_assignments",
    "node_assignments",
    &["node", "kind", "count", "last_at"],
    &["last_at"],
  ),
  seconds(
    "node_rotation",
    "node_rotation",
    &["rotation", "last_node", "updated_at"],
    &["updated_at"],
  ),
  seconds(
    "node_workplaces",
    "node_workplaces",
    &[
      "node",
      "root",
      "home",
      "runs",
      "state",
      "detail",
      "checked_at",
      "created_at",
      "updated_at",
    ],
    &["checked_at", "created_at", "updated_at"],
  ),
  seconds("prefix_moves", "prefix_moves", &["id", "agent", "at", "why", "detail"], &["at"]),
  seconds(
    "registered_model_addresses",
    "registered_model_addresses",
    &["registration", "node", "position", "url", "key_file", "key_owned", "placed_at"],
    &["placed_at"],
  ),
  seconds(
    "registered_models",
    "registered_models",
    &[
      "name",
      "label",
      "model",
      "context_tokens",
      "answer_tokens",
      "thinking",
      "walked",
      "added_at",
      "keeps_research",
    ],
    &["added_at"],
  ),
  seconds(
    "room_session_turns",
    "room_session_turns",
    &["agent", "chat_id", "generation", "position", "role", "text", "at"],
    &["at"],
  ),
  seconds(
    "room_sessions",
    "room_sessions",
    &[
      "agent",
      "chat_id",
      "generation",
      "opened_at",
      "last_message_id",
      "system_hash",
      "system_reset_at",
      "harness_session",
      "harness_road",
      "placement",
      "handoff_note",
      "spent_calls",
      "spent_ms",
    ],
    &["opened_at", "system_reset_at"],
  ),
  TableMap {
    // The old depth was a mode word (`basic`, `thorough`, `exhaustive`), not a nesting number; the
    // target's number has nothing to take from it, and the word has no column to keep it in.
    fixed: &[("depth", 0)],
    ..seconds(
      "runs",
      "runs",
      &[
        "id",
        "agent",
        "chat_id",
        "unit",
        "depth",
        "cap_ms",
        "started_at",
        "finished_at",
        "outcome",
        "started_by",
        "context_tokens",
        "search_id",
      ],
      &["started_at", "finished_at"],
    )
  },
  seconds(
    "session_resets",
    "session_resets",
    &["id", "agent", "chat_id", "asked_by", "asked_at", "reason", "done_at"],
    &["asked_at", "done_at"],
  ),
  TableMap {
    source: "usage_windows",
    target: "usage_windows",
    columns: &[
      "mode",
      "account",
      "name",
      "utilization",
      "resets_at",
      "seen_at",
      "duration_minutes",
      "limit_name",
    ],
    renamed: &[("duration_minutes", "duration_mins")],
    seconds: &[],
    fixed: &[],
  },
  seconds(
    "worker_sessions",
    "worker_sessions",
    &[
      "worker",
      "model",
      "account",
      "claude_session_id",
      "last_reply_at",
      "prefix_digest",
      "answered_model",
    ],
    &["last_reply_at"],
  ),
  seconds(
    "worker_words",
    "worker_words",
    &["id", "worker", "text", "added_at", "delivered_at"],
    &["added_at", "delivered_at"],
  ),
  same("settings", "settings", &["key", "value"]),
];

/// The target tables of the folded sources.
pub const FOLDED_TARGETS: [&str; 3] = ["jobs", "run_clocks", "clock_asks"];

fn text(value: Option<&Value>) -> Option<String> {
  match value {
    Some(Value::String(said)) => Some(said.clone()),
    Some(Value::Null) | None => None,
    Some(other) => Some(other.to_string()),
  }
}

fn int(value: Option<&Value>) -> Option<i64> {
  match value {
    Some(Value::Number(number)) => number.as_i64().or_else(|| number.as_f64().map(|n| n as i64)),
    Some(Value::String(said)) => said.parse().ok(),
    _ => None,
  }
}

fn cell_of(value: Option<&Value>, seconds: bool) -> Cell {
  match value {
    None | Some(Value::Null) => Cell::Null,
    Some(Value::Bool(flag)) => Cell::Int(i64::from(*flag)),
    Some(Value::Number(number)) => match number.as_i64() {
      Some(number) => Cell::Int(if seconds { number * 1000 } else { number }),
      None => Cell::Text(number.to_string()),
    },
    Some(Value::String(said)) => match said.parse::<i64>() {
      Ok(number) if seconds => Cell::Int(number * 1000),
      _ => Cell::Text(said.clone()),
    },
    Some(other) => Cell::Text(other.to_string()),
  }
}

/// One source row written in its target's columns.
pub fn row(table: &TableMap, old: &Value) -> Mapped {
  table
    .columns
    .iter()
    .map(|column| {
      if let Some((_, number)) = table.fixed.iter().find(|(target, _)| target == column) {
        return (*column, Cell::Int(*number));
      }
      let source = table
        .renamed
        .iter()
        .find(|(target, _)| target == column)
        .map(|(_, source)| *source)
        .unwrap_or(*column);
      (*column, cell_of(old.get(source), table.seconds.contains(&source)))
    })
    .collect()
}

/// The old job as the harness's job: the item and room it names, filled where the old row has them.
pub fn job(old: &Value) -> Mapped {
  let ok = int(old.get("ok")).unwrap_or(0);
  let outcome =
    text(old.get("outcome")).unwrap_or_else(|| if ok == 1 { "ok".to_owned() } else { "failed".to_owned() });

  vec![
    ("id", cell_of(old.get("id"), false)),
    ("item_id", Cell::Int(int(old.get("todo_id")).unwrap_or(0))),
    ("lease_id", Cell::Int(0)),
    ("holder_name", Cell::Text(text(old.get("worker")).unwrap_or_default())),
    ("assignment_id", Cell::Int(0)),
    ("worker_name", Cell::Text(text(old.get("worker")).unwrap_or_default())),
    ("session_id", Cell::Null),
    ("run_id", Cell::Null),
    ("run_name", cell_of(old.get("unit"), false)),
    ("room_id", cell_of(old.get("chat_id"), false)),
    ("state", Cell::Text(if ok == 1 { "ended" } else { "stopped" }.to_owned())),
    ("outcome", Cell::Text(outcome)),
    ("report", cell_of(old.get("report"), false)),
    ("token_count", Cell::Int(0)),
    ("started_at", cell_of(old.get("launched_at"), true)),
    ("ended_at", cell_of(old.get("finished_at"), true)),
    ("prompt", Cell::Text(text(old.get("prompt")).unwrap_or_default())),
    ("prompt_english", cell_of(old.get("prompt_english"), false)),
    ("model", cell_of(old.get("model"), false)),
  ]
}

/// The old clock hold as the run's clock and the ask past it.
pub fn clock(hold: &Value) -> (Mapped, Mapped) {
  let run = text(hold.get("run")).unwrap_or_default();
  let facts = serde_json::json!({
    "what": hold.get("what"),
    "clock": hold.get("clock"),
    "lastActivityAt": hold.get("last_activity_at"),
    "lastActivity": hold.get("last_activity"),
  })
  .to_string();
  let clock_row = vec![
    ("run", Cell::Text(run.clone())),
    ("parent", Cell::Null),
    ("work_key", Cell::Text(run.clone())),
    ("agent", cell_of(hold.get("agent"), false)),
    ("what", Cell::Text(text(hold.get("what")).unwrap_or_default())),
    ("started_at", cell_of(hold.get("started_at"), false)),
    ("bound_ms", Cell::Int(int(hold.get("clock_ms")).unwrap_or(0))),
    ("ended_at", cell_of(hold.get("decided_at").or_else(|| hold.get("closed_at")), false)),
  ];
  let ask = vec![
    ("id", cell_of(hold.get("id"), false)),
    ("run", Cell::Text(run)),
    ("facts", Cell::Text(facts)),
    ("asked_at", cell_of(hold.get("asked_at"), false)),
    ("verdict", cell_of(hold.get("verdict"), false)),
    ("kept_until", cell_of(hold.get("keep_until"), false)),
    ("decided_by", cell_of(hold.get("decided_by"), false)),
    ("why", cell_of(hold.get("why"), false)),
    ("closed_at", cell_of(hold.get("closed_at"), false)),
  ];

  (clock_row, ask)
}

/// One session's work: its latest item, and every result it holds as one list.
pub fn session_work(session: &str, items: &[Value], results: &[Value]) -> Mapped {
  let item = items
    .iter()
    .max_by_key(|row| int(row.get("served_at")).unwrap_or(0))
    .and_then(|row| int(row.get("todo_id")));
  let list: Vec<Value> = results
    .iter()
    .map(|row| {
      serde_json::json!({
        "result": row.get("result"),
        "version": row.get("version"),
        "loadedAt": int(row.get("loaded_at")).map(|seconds| seconds * 1000),
      })
    })
    .collect();
  let changed = items
    .iter()
    .filter_map(|row| int(row.get("served_at")))
    .chain(results.iter().filter_map(|row| int(row.get("loaded_at"))))
    .max()
    .unwrap_or(0);

  vec![
    ("session_id", Cell::Text(session.to_owned())),
    (
      "item_id",
      match item {
        Some(item) => Cell::Int(item),
        None => Cell::Null,
      },
    ),
    ("results", Cell::Text(Value::Array(list).to_string())),
    ("changed_at", Cell::Int(changed * 1000)),
  ]
}

/// The export directory's manifest.
pub fn read_manifest(dir: &Path) -> Result<Manifest, String> {
  let path = dir.join("manifest.json");
  let said = std::fs::read_to_string(&path)
    .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
  let document: Value =
    serde_json::from_str(&said).map_err(|error| format!("{} is not JSON: {error}", path.display()))?;
  let mut tables = HashMap::new();
  let list = document.get("tables").and_then(Value::as_array).ok_or("the manifest names no tables")?;
  for table in list {
    let source = table
      .get("source")
      .and_then(Value::as_str)
      .ok_or("a manifest table names no source")?
      .to_owned();
    let file = table
      .get("file")
      .and_then(Value::as_str)
      .ok_or("a manifest table names no file")?
      .to_owned();
    let count = table.get("count").and_then(Value::as_u64).ok_or("a manifest table names no count")?;
    let key: Vec<String> = table
      .get("key")
      .and_then(Value::as_array)
      .map(|keys| keys.iter().filter_map(Value::as_str).map(str::to_owned).collect())
      .unwrap_or_default();
    tables.insert(source, TableEntry { file, count, key });
  }

  Ok(Manifest {
    snapshot: document.get("snapshot").and_then(Value::as_str).unwrap_or_default().to_owned(),
    read_at: document.get("readAt").and_then(Value::as_str).unwrap_or_default().to_owned(),
    tables,
  })
}

/// One table's rows, each checked against the manifest's count as it is read.
pub fn rows(dir: &Path, manifest: &Manifest, source: &str) -> Result<Vec<Value>, String> {
  let entry = manifest
    .tables
    .get(source)
    .ok_or_else(|| format!("the manifest has no table {source}"))?;
  let path = dir.join(&entry.file);
  let said = std::fs::read_to_string(&path)
    .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
  let mut out = Vec::new();
  for (number, line) in said.lines().enumerate() {
    if line.trim().is_empty() {
      continue;
    }
    let row: Value = serde_json::from_str(line)
      .map_err(|error| format!("{} line {} is not JSON: {error}", path.display(), number + 1))?;
    out.push(row);
  }
  if out.len() as u64 != entry.count {
    return Err(format!(
      "{}: the manifest says {} rows and the file holds {}",
      path.display(),
      entry.count,
      out.len()
    ));
  }

  Ok(out)
}

/// The export read and mapped, before anything is written.
pub struct Prepared {
  pub manifest: Manifest,
  /// The plain tables: source, target, rows.
  pub tables: Vec<(&'static str, &'static str, MappedRows)>,
  /// The old jobs and their harness shape.
  pub jobs: MappedRows,
  /// The old clocks: each hold's clock and its ask.
  pub clocks: Vec<(Mapped, Mapped)>,
  /// The sessions' work, one row a session.
  pub work: MappedRows,
  /// The counts of the folded sources, for the report.
  pub folded: BTreeMap<&'static str, u64>,
}

/// Reads the export and maps every row to its target's shape.
pub fn prepare(export: &Path) -> Result<Prepared, String> {
  let manifest = read_manifest(export)?;
  let mut tables = Vec::new();
  for table in TABLES {
    let old = rows(export, &manifest, table.source)?;
    let mapped: Vec<_> = old.iter().map(|old| row(table, old)).collect();
    tables.push((table.source, table.target, mapped));
  }

  let old_jobs = rows(export, &manifest, "jobs")?;
  let jobs: Vec<_> = old_jobs.iter().map(job).collect();

  let holds = rows(export, &manifest, "clock_holds")?;
  let clocks: Vec<_> = holds.iter().map(clock).collect();

  let items = rows(export, &manifest, "session_work_items")?;
  let results = rows(export, &manifest, "session_work_results")?;
  let mut by_session: BTreeMap<String, (Vec<Value>, Vec<Value>)> = BTreeMap::new();
  for item in &items {
    if let Some(session) = text(item.get("session_id")) {
      by_session.entry(session).or_default().0.push(item.clone());
    }
  }
  for result in &results {
    if let Some(session) = text(result.get("session_id")) {
      by_session.entry(session).or_default().1.push(result.clone());
    }
  }
  let work: Vec<_> = by_session
    .iter()
    .map(|(session, (items, results))| session_work(session, items, results))
    .collect();

  let mut folded = BTreeMap::new();
  folded.insert("jobs", old_jobs.len() as u64);
  folded.insert("clock_holds", holds.len() as u64);
  folded.insert("session_work_items", items.len() as u64);
  folded.insert("session_work_results", results.len() as u64);

  Ok(Prepared {
    manifest,
    tables,
    jobs,
    clocks,
    work,
    folded,
  })
}

/// Every target table this import writes.
pub fn targets() -> Vec<&'static str> {
  let mut out: Vec<&'static str> = TABLES.iter().map(|table| table.target).collect();
  out.extend(FOLDED_TARGETS);
  out.push("session_work");
  out
}

/// The target's columns and their types, so every value can be cast from its text.
async fn types(pool: &PgPool, table: &str) -> Result<HashMap<String, String>, String> {
  let rows = sqlx::query(
    "SELECT column_name, data_type FROM information_schema.columns \
     WHERE table_schema = 'public' AND table_name = $1",
  )
  .bind(table)
  .fetch_all(pool)
  .await
  .map_err(|error| format!("{table}'s columns could not be read: {error}"))?;
  let mut out = HashMap::new();
  for row in rows {
    out.insert(row.get::<String, _>("column_name"), row.get::<String, _>("data_type"));
  }
  if out.is_empty() {
    return Err(format!("{table} holds no columns"));
  }

  Ok(out)
}

fn bind<'q>(
  mut query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
  cell: &Cell,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
  let said = match cell {
    Cell::Null => None,
    Cell::Int(number) => Some(number.to_string()),
    Cell::Text(said) => Some(said.clone()),
  };
  query = query.bind(said);
  query
}

async fn insert(
  tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  table: &str,
  kinds: &HashMap<String, String>,
  row: &[(&'static str, Cell)],
) -> Result<(), String> {
  let columns = row.iter().map(|(column, _)| format!("\"{column}\"")).collect::<Vec<_>>().join(", ");
  let values = row
    .iter()
    .enumerate()
    .map(|(position, (column, _))| {
      let kind = kinds.get(*column).map(String::as_str).unwrap_or("text");
      format!("CAST(${} AS {kind})", position + 1)
    })
    .collect::<Vec<_>>()
    .join(", ");
  let sql = format!("INSERT INTO \"{table}\" ({columns}) VALUES ({values})");
  let mut query = sqlx::query(&sql);
  for (_, cell) in row {
    query = bind(query, cell);
  }
  query
    .execute(&mut **tx)
    .await
    .map_err(|error| format!("{table} could not be written: {error}"))?;

  Ok(())
}

/// Refuses a store that already holds rows of any table this import writes.
pub async fn refused_if_holding(pool: &PgPool) -> Result<(), String> {
  let mut holding = Vec::new();
  for table in targets() {
    // A table the two axes carried away and dropped holds nothing to write over.
    let stands: Option<String> = sqlx::query_scalar(&format!("SELECT to_regclass('\"{table}\"')::text"))
      .fetch_one(pool)
      .await
      .unwrap_or(None);
    if stands.is_none() {
      continue;
    }
    let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM \"{table}\""))
      .fetch_one(pool)
      .await
      .map_err(|error| format!("{table} could not be counted: {error}"))?;
    if count > 0 {
      holding.push(format!("{table} holds {count}"));
    }
  }
  if holding.is_empty() {
    Ok(())
  } else {
    Err(format!(
      "the store already holds rows of its tables, so nothing was written: {}",
      holding.join(", ")
    ))
  }
}

fn digest(said: &str) -> String {
  Sha256::digest(said.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sample<'a>(rows: &'a [Mapped], key: &[String]) -> Vec<&'a Mapped> {
  if rows.len() <= 20 {
    return rows.iter().collect();
  }
  let mut ordered: Vec<(String, &Mapped)> = rows
    .iter()
    .map(|row| {
      let said = key
        .iter()
        .map(|column| {
          row
            .iter()
            .find(|(held, _)| held == column)
            .map(|(_, cell)| format!("{cell:?}"))
            .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("|");
      (digest(&said), row)
    })
    .collect();
  ordered.sort_by(|left, right| left.0.cmp(&right.0));
  let mut picked: Vec<&Mapped> = vec![&rows[0], &rows[rows.len() - 1]];
  for (_, row) in ordered.into_iter().take(20) {
    if !picked.iter().any(|held| std::ptr::eq(*held, row)) {
      picked.push(row);
    }
  }

  picked
}

async fn compare(pool: &PgPool, prepared: &Prepared) -> Sample {
  let mut failures = Vec::new();
  let mut checked = 0;
  for (source, target, rows) in &prepared.tables {
    let entry = match prepared.manifest.tables.get(*source) {
      Some(entry) => entry,
      None => continue,
    };
    // A table the two axes carried into their own rows and dropped is compared no more.
    let stands: Option<String> = sqlx::query_scalar(&format!("SELECT to_regclass('\"{target}\"')::text"))
      .fetch_one(pool)
      .await
      .unwrap_or(None);
    if stands.is_none() {
      continue;
    }
    // A setting a migration writes on its own, from what the rows held, is no row of the export's.
    let own = if *target == "settings" {
      format!(
        " WHERE \"key\" NOT IN ({})",
        crate::migration::SETTINGS_WRITTEN
          .iter()
          .map(|key| format!("'{key}'"))
          .collect::<Vec<_>>()
          .join(", ")
      )
    } else {
      String::new()
    };
    let count: Result<i64, _> = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM \"{target}\"{own}"))
      .fetch_one(pool)
      .await;
    match count {
      Ok(held) if held as u64 == entry.count => {}
      Ok(held) => {
        failures.push(format!("{source}: the export holds {} rows and {target} holds {held}", entry.count))
      }
      Err(error) => failures.push(format!("{source}: {target} could not be counted: {error}")),
    }
    if entry.key.is_empty() {
      failures.push(format!("{source}: the export records no key to sample by"));
      continue;
    }
    let kinds = match types(pool, target).await {
      Ok(kinds) => kinds,
      Err(error) => {
        failures.push(error);
        continue;
      }
    };
    for row in sample(rows, &entry.key) {
      checked += 1;
      let selected = row
        .iter()
        .map(|(column, _)| format!("\"{column}\"::text AS \"{column}\""))
        .collect::<Vec<_>>()
        .join(", ");
      let mut sql = format!("SELECT {selected} FROM \"{target}\" WHERE ");
      for (position, column) in entry.key.iter().enumerate() {
        if position > 0 {
          sql.push_str(" AND ");
        }
        let kind = kinds.get(column).map(String::as_str).unwrap_or("text");
        sql.push_str(&format!("\"{column}\" = CAST(${} AS {kind})", position + 1));
      }
      let mut query = sqlx::query(&sql);
      let mut bound = true;
      for column in &entry.key {
        match row.iter().find(|(held, _)| held == column).map(|(_, cell)| cell) {
          Some(cell) => query = bind(query, cell),
          None => {
            failures.push(format!("{source}: the mapped row carries no {column}"));
            bound = false;
            break;
          }
        }
      }
      if !bound {
        continue;
      }
      match query.fetch_optional(pool).await {
        Ok(Some(held)) => {
          for (column, cell) in row {
            let actual = held
              .try_get::<Option<String>, _>(*column)
              .map(|said| said.map_or(Cell::Null, Cell::Text));
            let expected = match cell {
              Cell::Null => Cell::Null,
              Cell::Int(number) => Cell::Text(number.to_string()),
              Cell::Text(said) => Cell::Text(said.clone()),
            };
            match actual {
              Ok(actual) if actual == expected => {}
              Ok(actual) => failures
                .push(format!("{source} -> {target}: {column} was written {expected:?} but is {actual:?}")),
              Err(error) => {
                failures.push(format!("{source} -> {target}: {column} could not be read: {error}"))
              }
            }
          }
        }
        Ok(None) => failures.push(format!("{source} -> {target}: a sampled row is missing")),
        Err(error) => failures.push(format!("{source} -> {target}: the sample could not be read: {error}")),
      }
    }
  }

  // The folded tables: one job a row, one clock and one ask a hold, one session a row.
  let jobs: i64 = count(pool, "jobs", &mut failures).await;
  if jobs as u64 != prepared.jobs.len() as u64 {
    failures.push(format!("jobs: the export holds {} rows and jobs holds {jobs}", prepared.jobs.len()));
  }
  let clocks: i64 = count(pool, "run_clocks", &mut failures).await;
  let asks: i64 = count(pool, "clock_asks", &mut failures).await;
  if clocks as u64 != prepared.folded["clock_holds"] || asks as u64 != prepared.folded["clock_holds"] {
    failures.push(format!(
      "clock_holds: the export holds {} rows, run_clocks {clocks} and clock_asks {asks}",
      prepared.folded["clock_holds"]
    ));
  }
  let work: i64 = count(pool, "session_work", &mut failures).await;
  if work as u64 != prepared.work.len() as u64 {
    failures.push(format!(
      "session work: the export names {} sessions and session_work holds {work}",
      prepared.work.len()
    ));
  }
  // Every session's results, read back and counted.
  for row in &prepared.work {
    let Some(session) = row.iter().find(|(column, _)| *column == "session_id") else {
      continue;
    };
    checked += 1;
    let Cell::Text(session) = &session.1 else {
      failures.push("session work: a session has no key".to_owned());
      continue;
    };
    let expected = row
      .iter()
      .find(|(column, _)| *column == "results")
      .and_then(|(_, cell)| match cell {
        Cell::Text(said) => serde_json::from_str::<Value>(said).ok(),
        _ => None,
      })
      .and_then(|value| value.as_array().map(Vec::len))
      .unwrap_or(0);
    let held: Result<Option<String>, _> =
      sqlx::query_scalar("SELECT \"results\"::text FROM \"session_work\" WHERE \"session_id\" = $1")
        .bind(session)
        .fetch_optional(pool)
        .await;
    match held {
      Ok(Some(said)) => {
        let got = serde_json::from_str::<Value>(&said)
          .ok()
          .and_then(|value| value.as_array().map(Vec::len))
          .unwrap_or(0);
        if got != expected {
          failures.push(format!("session_work {session}: wrote {expected} results and holds {got}"));
        }
      }
      Ok(None) => failures.push(format!("session_work {session}: the row is missing")),
      Err(error) => failures.push(format!("session_work {session}: could not be read: {error}")),
    }
  }

  Sample { checked, failures }
}

async fn count(pool: &PgPool, table: &str, failures: &mut Vec<String>) -> i64 {
  match sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM \"{table}\""))
    .fetch_one(pool)
    .await
  {
    Ok(count) => count,
    Err(error) => {
      failures.push(format!("{table} could not be counted: {error}"));
      0
    }
  }
}

/// Runs the import: reads the export, refuses a store holding rows, writes every row, and compares
/// what it wrote. A dry run rolls the transaction back and reports what would have been written.
pub async fn run(database: &str, export: &Path, dry_run: bool) -> Result<Report, String> {
  let prepared = prepare(export)?;
  migrate(database, true).await.map_err(|error| format!("the migrations: {error}"))?;
  let pool = PgPoolOptions::new()
    .max_connections(1)
    .connect(database)
    .await
    .map_err(|error| format!("{database} could not be opened: {error}"))?;
  refused_if_holding(&pool).await?;

  let mut kinds: HashMap<&str, HashMap<String, String>> = HashMap::new();
  for (_, target, _) in &prepared.tables {
    kinds.insert(target, types(&pool, target).await?);
  }
  for target in ["jobs", "run_clocks", "clock_asks", "session_work"] {
    kinds.insert(target, types(&pool, target).await?);
  }

  let mut tx = pool
    .begin()
    .await
    .map_err(|error| format!("the transaction could not be opened: {error}"))?;
  for (_, target, rows) in &prepared.tables {
    let table_kinds = kinds.get(target).expect("a target's types");
    for row in rows {
      insert(&mut tx, target, table_kinds, row).await?;
    }
  }
  for row in &prepared.jobs {
    insert(&mut tx, "jobs", kinds.get("jobs").expect("jobs"), row).await?;
  }
  for (clock, ask) in &prepared.clocks {
    insert(&mut tx, "run_clocks", kinds.get("run_clocks").expect("run_clocks"), clock).await?;
    insert(&mut tx, "clock_asks", kinds.get("clock_asks").expect("clock_asks"), ask).await?;
  }
  for row in &prepared.work {
    insert(&mut tx, "session_work", kinds.get("session_work").expect("session_work"), row).await?;
  }

  if dry_run {
    tx.rollback()
      .await
      .map_err(|error| format!("the dry run could not be rolled back: {error}"))?;
  } else {
    tx.commit()
      .await
      .map_err(|error| format!("the import could not be committed: {error}"))?;
  }

  let sample = if dry_run {
    Sample {
      checked: 0,
      failures: Vec::new(),
    }
  } else {
    compare(&pool, &prepared).await
  };

  // What was written in the shape before the two axes is carried over as the rest of the migrations run.
  if !dry_run {
    migrate(database, false).await.map_err(|error| format!("the migrations: {error}"))?;
  }

  let mut tables: Vec<Moved> = prepared
    .tables
    .iter()
    .map(|(source, target, rows)| Moved {
      source: (*source).to_owned(),
      target: (*target).to_owned(),
      exported: prepared.manifest.tables.get(*source).map(|entry| entry.count).unwrap_or(0),
      written: rows.len() as u64,
    })
    .collect();
  for (source, target, written) in [
    ("jobs", "jobs", prepared.jobs.len() as u64),
    ("clock_holds", "run_clocks", prepared.clocks.len() as u64),
    ("clock_holds", "clock_asks", prepared.clocks.len() as u64),
    ("session_work_items", "session_work", prepared.work.len() as u64),
    ("session_work_results", "session_work", 0),
  ] {
    tables.push(Moved {
      source: source.to_owned(),
      target: target.to_owned(),
      exported: prepared.folded.get(source).copied().unwrap_or(0),
      written,
    });
  }
  pool.close().await;

  Ok(Report {
    dry_run,
    snapshot: prepared.manifest.snapshot.clone(),
    read_at: prepared.manifest.read_at.clone(),
    tables,
    sample,
  })
}

/// The count-and-sample compare alone, against a store the import wrote.
pub async fn verify(database: &str, export: &Path) -> Result<Sample, String> {
  let prepared = prepare(export)?;
  let pool = PgPoolOptions::new()
    .max_connections(1)
    .connect(database)
    .await
    .map_err(|error| format!("{database} could not be opened: {error}"))?;
  let sample = compare(&pool, &prepared).await;
  pool.close().await;

  Ok(sample)
}
