//! Codex: present where its executable stands on the asking process's PATH, and its catalogue the one
//! bundled with that executable (docs/provider-presence.md).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use policy::providers::Declared;
use policy::providers::codex::CODEX;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::process::Command;

use crate::{Headline, Machine, Offered, OfferedModel, Offering, PlaceCache, Provider, Reading, Window};

pub struct Codex;

impl Provider for Codex {
  fn name(&self) -> &'static str {
    "codex"
  }

  // Codex names the thread it opened once, on the event that opens it.
  fn session_of_event(&self, event: &serde_json::Value) -> Option<String> {
    (event.get("type").and_then(serde_json::Value::as_str) == Some("thread.started"))
      .then(|| event.get("thread_id").and_then(serde_json::Value::as_str).map(str::to_owned))
      .flatten()
  }

  // What it said, and the commands it ran.
  fn progress_of(&self, line: &str) -> Option<String> {
    let event: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let item = event
      .get("item")
      .filter(|_| event.get("type").and_then(serde_json::Value::as_str) == Some("item.completed"))?;
    let text = |key: &str| item.get(key).and_then(serde_json::Value::as_str).map(str::to_owned);

    match item.get("type").and_then(serde_json::Value::as_str)? {
      "agent_message" => text("text"),
      "command_execution" => text("command").map(|command| format!("$ {command}")),
      _ => None,
    }
  }

  fn declared(&self) -> &'static Declared {
    &CODEX.declared
  }

  fn owns_program(&self, _machine: &Machine, args: &[String], _env: &BTreeMap<String, String>) -> bool {
    args.first().is_some_and(|program| *program == CODEX.presence.program)
  }

  fn spends_by_last_request(&self) -> bool {
    true
  }

  fn cache_minutes(&self, model: &str) -> Option<i64> {
    CODEX.cache.of(model).map(|entry| entry.ttl_minutes)
  }

  fn place_cache(&self, model: &str, _upstream: Option<&str>) -> Result<PlaceCache, String> {
    CODEX
      .cache
      .of(model)
      .map(|entry| PlaceCache {
        ttl_minutes: entry.ttl_minutes,
        idle_minutes: entry.keepalive_idle_minutes,
      })
      .ok_or_else(|| format!("Codex publishes no cache lifetime for {model}."))
  }

  fn missing(&self, machine: &Machine) -> Option<String> {
    machine
      .program(&CODEX.presence.program)
      .is_none()
      .then(|| CODEX.presence.missing.clone())
  }

  fn catalogue<'a>(&'a self, machine: &'a Machine, _config_dir: Option<&'a Path>) -> Reading<'a> {
    Box::pin(async move { Ok(json!({ "models": models(machine).await? })) })
  }

  // The standard windows where any is held, the legacy ones otherwise; each slot is the window of its
  // length, or the one named for it where the reading gave no length.
  fn headline(&self, windows: &[Window]) -> Headline {
    let under = |prefix: &str| -> Vec<usize> {
      (0..windows.len()).filter(|&at| windows[at].name.starts_with(prefix)).collect()
    };
    let standard = under("codex:codex:");
    let held = if standard.is_empty() { under("codex:default:") } else { standard };
    let preferred = |slot: &str, minutes: i64| {
      let named = format!(":{slot}");

      held
        .iter()
        .copied()
        .find(|&at| windows[at].duration_minutes == Some(minutes))
        .or_else(|| {
          held
            .iter()
            .copied()
            .find(|&at| windows[at].duration_minutes.is_none() && windows[at].name.ends_with(&named))
        })
    };

    Headline {
      session: preferred("primary", 5 * 60),
      week: preferred("secondary", 7 * 24 * 60),
    }
  }

  // A mapping of a model takes the efforts the catalogue says it supports.
  fn offered<'a>(
    &'a self,
    machine: &'a Machine,
    _config_dir: Option<&'a Path>,
    _now_ms: f64,
  ) -> Offering<'a> {
    Box::pin(async move {
      let models = models(machine).await?.into_iter().map(|model| OfferedModel {
        efforts: model.supported_reasoning_efforts.into_iter().map(Some).collect(),
        id: model.id,
        free: None,
        in_service: None,
      });

      Ok(Offered {
        models: models.collect(),
        stale_read_at: None,
      })
    })
  }
}

/// One model the installed CLI offers a chooser.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
  pub id: String,
  pub name: String,
  pub description: String,
  pub context_length: Option<i64>,
  pub default_reasoning_effort: String,
  pub supported_reasoning_efforts: Vec<String>,
}

/// The catalogue as the CLI prints it. Hidden compatibility aliases stay hidden: a chooser offers the
/// models the CLI offers a person, not every slug its parser still understands.
pub fn read_models(text: &str) -> Result<Vec<Model>, String> {
  let payload: Value = serde_json::from_str(text).map_err(|error| format!("codex models: {error}"))?;
  let rows = payload
    .get("models")
    .and_then(Value::as_array)
    .ok_or("codex models: the CLI returned no model list")?;

  Ok(
    rows
      .iter()
      .filter(|row| row.get("visibility").and_then(Value::as_str) != Some("hide"))
      .filter_map(|row| {
        let id = row.get("slug")?.as_str().filter(|slug| !slug.is_empty())?;
        let text = |name: &str| row.get(name).and_then(Value::as_str).map(str::to_owned);

        Some(Model {
          id: id.to_owned(),
          name: text("display_name").unwrap_or_else(|| id.to_owned()),
          description: text("description").unwrap_or_default(),
          context_length: row.get("context_window").and_then(Value::as_f64).map(|length| length as i64),
          default_reasoning_effort: text("default_reasoning_level").unwrap_or_else(|| "medium".to_owned()),
          supported_reasoning_efforts: row
            .get("supported_reasoning_levels")
            .and_then(Value::as_array)
            .map(|levels| {
              levels
                .iter()
                .filter_map(|level| level.get("effort")?.as_str().map(str::to_owned))
                .collect()
            })
            .unwrap_or_default(),
        })
      })
      .collect(),
  )
}

/// What a failed run of the CLI said: the last error its event stream reported, or else the last line
/// of its standard error, or else its exit status.
pub fn failure(stdout: &str, stderr: &str, status: Option<i32>) -> String {
  let mut reported = stdout
    .lines()
    .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
    .filter_map(|event| {
      let message = match event.get("type").and_then(Value::as_str) {
        Some("error") => event.get("message"),
        Some("turn.failed") => event.pointer("/error/message"),
        _ => None,
      };

      message.and_then(Value::as_str).filter(|message| !message.is_empty()).map(str::to_owned)
    });

  reported
    .next_back()
    .or_else(|| stderr.lines().map(str::trim).rfind(|line| !line.is_empty()).map(str::to_owned))
    .unwrap_or_else(|| {
      format!("exit {}", status.map_or_else(|| "null".to_owned(), |code| code.to_string()))
    })
}

/// Every model the installed executable offers, asked within the seconds the policy gives it.
pub async fn models(machine: &Machine) -> Result<Vec<Model>, String> {
  let program = machine.program(&CODEX.presence.program).ok_or_else(|| CODEX.presence.missing.clone())?;
  let mut command = Command::new(program);

  command
    .args(&CODEX.catalogue.args)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);

  if let Some(path) = machine.var("PATH") {
    command.env("PATH", path);
  }

  let child = command.spawn().map_err(|error| format!("codex models: {error}"))?;
  let output = tokio::time::timeout(Duration::from_secs(CODEX.catalogue.seconds), child.wait_with_output())
    .await
    .map_err(|_| format!("codex models: no answer within {} seconds", CODEX.catalogue.seconds))?
    .map_err(|error| format!("codex models: {error}"))?;
  let stdout = String::from_utf8_lossy(&output.stdout);

  if !output.status.success() {
    return Err(failure(&stdout, &String::from_utf8_lossy(&output.stderr), output.status.code()));
  }

  read_models(&stdout)
}
