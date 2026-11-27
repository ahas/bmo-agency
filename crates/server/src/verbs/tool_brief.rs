//! The tool brief (docs/agent-control.md, 「The tool brief」): a `PreToolUse` hook on every MCP tool that hands the
//! program, with a tool's first call in a session, the tool's manual as the toolbook describes it and the brain's
//! lessons for it, and hands nothing on any later call of it, nor where either cannot be read in time.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::process::Command;

// How long either read may take before the call goes on without it.
const READ_SECONDS: u64 = 5;

// How many lessons a brief carries, and the most of a manual it carries.
const LESSONS: u32 = 3;
const MANUAL_CHARS: usize = 6_000;

/// The tool a call names, by its full name: `<namespace>/<tool>`, a call through the toolbook taken as the tool it
/// names; none for a call of no MCP tool.
pub fn tool_named(payload: &Value) -> Option<String> {
  let called = payload.get("tool_name").and_then(Value::as_str)?;
  let rest = called.strip_prefix("mcp__")?;
  let (server, tool) = rest.split_once("__")?;

  if server == "toolbook" {
    return payload
      .pointer("/tool_input/name")
      .and_then(Value::as_str)
      .filter(|name| name.contains('/'))
      .map(str::to_owned);
  }

  Some(format!("{server}/{tool}"))
}

// The file a session's briefed tools stand in, one name a line.
fn briefed_file(place: &Path, session: &str) -> PathBuf {
  let session: String = session.chars().filter(|one| one.is_ascii_alphanumeric() || *one == '-').collect();

  place.join("tool-briefs").join(format!("{session}.txt"))
}

fn briefed(place: &Path, session: &str) -> BTreeSet<String> {
  std::fs::read_to_string(briefed_file(place, session))
    .map(|text| text.lines().map(str::to_owned).collect())
    .unwrap_or_default()
}

fn mark_briefed(place: &Path, session: &str, tool: &str) {
  let file = briefed_file(place, session);
  let mut names = briefed(place, session);

  names.insert(tool.to_owned());

  if let Some(folder) = file.parent() {
    let _ = std::fs::create_dir_all(folder);
  }

  let _ = std::fs::write(&file, names.into_iter().collect::<Vec<_>>().join("\n") + "\n");
}

// One `bmo use` call's standard output, none where it fails or runs past the bound.
async fn used(args: &[&str]) -> Option<String> {
  let run = Command::new("bmo").arg("use").args(args).kill_on_drop(true).output();
  let output = tokio::time::timeout(Duration::from_secs(READ_SECONDS), run).await.ok()?.ok()?;

  output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The skill of a plugin's own MCP server, read from its service's deploy tree under `root` (the home's
/// `.local/share`): the first `SKILL.md` standing in a `skills` folder of `bmo-<server>/deploy`, a few folders
/// deep, its front matter left out and cut to the bound; none where no such file stands.
pub fn skill_of(root: &Path, server: &str) -> Option<String> {
  if server.is_empty() || !server.chars().all(|one| one.is_ascii_alphanumeric() || one == '-' || one == '_')
  {
    return None;
  }

  let found = find_skill(&root.join(format!("bmo-{server}")).join("deploy"), 0)?;
  let text = std::fs::read_to_string(found).ok()?;
  let body = match text.strip_prefix("---") {
    Some(rest) => rest.split_once("\n---").map(|(_, body)| body.trim_start_matches('-')).unwrap_or(rest),
    None => text.as_str(),
  };
  let body = body.trim();

  (!body.is_empty()).then(|| body.chars().take(MANUAL_CHARS).collect())
}

// A `SKILL.md` in a `skills` folder at or under `folder`, the folders read in name order, five deep at most, past
// the folders a build or a package manager fills.
fn find_skill(folder: &Path, depth: usize) -> Option<PathBuf> {
  if depth > 5 {
    return None;
  }

  let mut entries: Vec<PathBuf> = std::fs::read_dir(folder)
    .ok()?
    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
    .collect();

  entries.sort();

  for entry in &entries {
    let named = entry.file_name().and_then(|name| name.to_str()).unwrap_or_default();

    if named == "SKILL.md"
      && entry
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .is_some_and(|name| name == "skills")
    {
      return Some(entry.clone());
    }
  }

  entries
    .iter()
    .filter(|entry| entry.is_dir())
    .filter(|entry| {
      let named = entry.file_name().and_then(|name| name.to_str()).unwrap_or_default();

      !matches!(named, "node_modules" | "target" | ".git") && !named.starts_with('.')
    })
    .find_map(|entry| find_skill(entry, depth + 1))
}

// The tool's description and manual as the toolbook describes it, cut to the bound.
async fn manual_of(tool: &str) -> Option<String> {
  let text = described_text(&used(&[tool, "--describe"]).await?);

  (!text.trim().is_empty()).then(|| text.chars().take(MANUAL_CHARS).collect())
}

/// The words a describe printed: the text a JSON string holds, the texts of a tool answer's `content`, the
/// description and manual of an entry, or what was printed where it reads as none of them.
pub fn described_text(printed: &str) -> String {
  let Ok(value) = serde_json::from_str::<Value>(printed) else {
    return printed.trim().to_owned();
  };

  match &value {
    Value::String(text) => text.trim().to_owned(),
    Value::Object(fields) => {
      if let Some(content) = fields.get("content").and_then(Value::as_array) {
        return content
          .iter()
          .filter_map(|part| part.get("text").and_then(Value::as_str))
          .collect::<Vec<_>>()
          .join("\n")
          .trim()
          .to_owned();
      }

      ["description", "manual"]
        .iter()
        .filter_map(|field| fields.get(*field).and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
    }
    _ => printed.trim().to_owned(),
  }
}

/// The words a call's situation is read by: the tool's name and the names and short text values of its arguments.
pub fn situation_of(tool: &str, payload: &Value) -> String {
  let mut words = vec![tool.replace(['/', '_'], " ")];

  if let Some(input) = payload.get("tool_input").and_then(Value::as_object) {
    for (key, value) in input {
      words.push(key.clone());

      if let Some(text) = value.as_str().filter(|text| text.chars().count() <= 60) {
        words.push(text.to_owned());
      }
    }
  }

  words.join(" ")
}

// The brain's active tips and checks whose situation the call meets, one a line.
async fn lessons_of(situation: &str) -> Vec<String> {
  let asked = json!({ "situation": situation, "type": "tip,check", "status": "active", "limit": LESSONS })
    .to_string();
  let Some(answer) = used(&["bmo/brain_list", "--json", &asked]).await else {
    return vec![];
  };
  let Ok(answer) = serde_json::from_str::<Value>(&answer) else { return vec![] };
  let rows = answer
    .get("items")
    .or_else(|| answer.get("memories"))
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();

  rows
    .iter()
    .filter_map(|row| row.get("text").and_then(Value::as_str))
    .map(|text| format!("- {text}"))
    .take(LESSONS as usize)
    .collect()
}

/// The brief a call is handed, as the hook's JSON answer: none where the call names no MCP tool, the session has
/// called that tool before, or nothing could be read of it.
pub async fn brief(place: &Path, payload: &Value) -> Option<String> {
  let tool = tool_named(payload)?;
  let session = payload.get("session_id").and_then(Value::as_str).unwrap_or("session");

  if briefed(place, session).contains(&tool) {
    return None;
  }

  let situation = situation_of(&tool, payload);
  let (manual, lessons) = tokio::join!(manual_of(&tool), lessons_of(&situation));
  let mut parts = Vec::new();
  let server = tool.split('/').next().unwrap_or_default().to_owned();
  let skill_mark = format!("skill:{server}");

  if let Some(manual) = manual {
    parts.push(format!("How {tool} is used:\n{manual}"));
  } else if !briefed(place, session).contains(&skill_mark)
    && let Some(root) = std::env::home_dir().map(|home| home.join(".local/share"))
    && let Some(skill) = skill_of(&root, &server)
  {
    // A plugin's own server the tool host does not describe: its skill, once a session.
    parts.push(format!("How the {server} tools are used:\n{skill}"));
    mark_briefed(place, session, &skill_mark);
  }

  if !lessons.is_empty() {
    parts.push(format!("Lessons kept for {tool}:\n{}", lessons.join("\n")));
  }

  // A tool nothing could be read of is briefed at its next call instead.
  if parts.is_empty() {
    return None;
  }

  mark_briefed(place, session, &tool);

  Some(
    json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse", "additionalContext": parts.join("\n\n") } })
      .to_string(),
  )
}
