//! The panes standing on this machine: each agent's resident program, whether a turn or job of it is
//! live, and what memory its process tree holds (docs/agent-control.md, 「A pane nobody uses」). A pane
//! with no live turn or job and a living holder is closed by its place; a name no agent row knows is
//! left for the owner.

use std::path::Path;

use serde_json::{Value, json};
use store::all;

use super::{AgentRow, Ground};
use crate::lines::place::Place;

/// One standing pane, as the screen reads it.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Standing {
  pub agent: String,
  pub pid: i32,
  pub memory_bytes: u64,
  pub known: bool,
  pub live: bool,
  pub closer: bool,
}

fn rss(pid: i32) -> u64 {
  let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/status")) else { return 0 };
  text
    .lines()
    .find_map(|line| line.strip_prefix("VmRSS:"))
    .and_then(|value| value.split_whitespace().next()?.parse::<u64>().ok())
    .unwrap_or(0)
    * 1024
}

// The program and its direct children, which is the node process a dsh program starts.
fn tree_memory(pid: i32) -> u64 {
  let mut total = rss(pid);
  let Ok(entries) = std::fs::read_dir("/proc") else { return total };

  for entry in entries.flatten() {
    let Ok(child) = entry.file_name().to_string_lossy().parse::<i32>() else { continue };
    let Ok(text) = std::fs::read_to_string(format!("/proc/{child}/status")) else {
      continue;
    };
    let ppid = text
      .lines()
      .find_map(|line| line.strip_prefix("PPid:"))
      .and_then(|value| value.split_whitespace().next()?.parse::<i32>().ok());

    if ppid == Some(pid) {
      total += rss(child);
    }
  }

  total
}

fn turn_open(place: &Path) -> bool {
  let Ok(text) = std::fs::read_to_string(place.join("loop.jsonl")) else { return false };
  let mut open = false;

  for record in text.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()) {
    match record.get("kind").and_then(Value::as_str) {
      Some("start") => open = true,
      Some("end") | Some("interrupted") => open = false,
      _ => {}
    }
  }

  open
}

async fn job_open(ground: &Ground, agent: &str) -> bool {
  all(
    &ground.db,
    r#"SELECT 1 FROM "jobs" WHERE "worker_name" = $1 AND "state" IN ('starting', 'running') LIMIT 1"#,
    vec![agent.into()],
  )
  .await
  .map(|rows| !rows.is_empty())
  .unwrap_or(true)
}

/// Every pane standing under a known agent, and every place whose name no row knows.
pub async fn standing(ground: &Ground) -> Vec<Standing> {
  let rows = AgentRow::standing(&ground.db).await.unwrap_or_default();
  let mut known = Vec::new();
  let mut names = std::collections::BTreeSet::new();

  for row in rows {
    names.insert(row.name.clone());

    let place = Place::of(&ground.roots.data, &row.name);
    let Some(running) = place.program_alive() else { continue };
    let live = job_open(ground, &row.name).await || turn_open(&place.path);

    known.push(Standing {
      agent: row.name,
      pid: running.pid,
      memory_bytes: tree_memory(running.pid),
      known: true,
      live,
      closer: !live,
    });
  }

  if let Ok(entries) = std::fs::read_dir(ground.roots.data.join("agent-places")) {
    for entry in entries.flatten() {
      let name = entry.file_name().to_string_lossy().into_owned();
      if names.contains(&name) || name.starts_with('.') {
        continue;
      }

      let place = Place::of(&ground.roots.data, &name);
      let Some(running) = place.program_alive() else { continue };

      known.push(Standing {
        agent: name,
        pid: running.pid,
        memory_bytes: tree_memory(running.pid),
        known: false,
        live: false,
        closer: false,
      });
    }
  }

  known.sort_by(|one, two| one.agent.cmp(&two.agent));
  known
}

/// The count and the memory the standing panes hold.
pub async fn summary(ground: &Ground) -> Value {
  let panes = standing(ground).await;
  let memory: u64 = panes.iter().map(|pane| pane.memory_bytes).sum();
  let unknown: Vec<&str> =
    panes.iter().filter(|pane| !pane.known).map(|pane| pane.agent.as_str()).collect();

  json!({
    "count": panes.len(),
    "memoryBytes": memory,
    "unknown": unknown,
    "items": panes,
  })
}

/// Close each pane with no live turn or job, one at a time, saying what closed and why.
pub async fn reap(ground: &Ground) {
  for pane in standing(ground).await {
    if !pane.known || pane.live || !pane.closer {
      continue;
    }

    let place = Place::of(&ground.roots.data, &pane.agent);
    let before = place.release().await;

    if before.is_some() {
      println!(
        "pane {}: closed (no turn or job; {:.1} MB)",
        pane.agent,
        pane.memory_bytes as f64 / 1024.0 / 1024.0
      );
    }
  }
}
