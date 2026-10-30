//! `worker_ask`: a chat agent hands a resident worker a small coding task as a turn into the worker's
//! session, with no queue item, and reads the words it wrote and the files the turn changed
//! (docs/tools.md, 「The worker ask」; docs/agent-control.md, 「Asking a worker directly」).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tool_call::{Answer, Asked, Handed, Session};

/// The entry the manifest declares.
pub const NAMESPACE: &str = "bmo";
pub const NAME: &str = "worker_ask";
pub const TITLE: &str = "Ask a worker";
pub const DESCRIPTION: &str = "Give a resident worker a small coding task as a turn, without filing a queue item, and answer with what it wrote and the files it changed. A task worth keeping and reviewing is a queue item.";
pub const CAPABILITIES: &[&str] = &["room.tools"];

const DEFAULT_BOUND_SECONDS: u64 = 600;
const DEFAULT_CONTROL_PORT: u16 = 3220;

pub fn entry() -> String {
  format!("{NAMESPACE}/{NAME}")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskInput {
  pub worker: String,
  pub words: String,
  #[serde(default)]
  pub folder: Option<String>,
  #[serde(default)]
  pub time_bound_seconds: Option<u64>,
}

/// The agency server a tool reaches: its port and token, from its own environment or the server's env file.
pub struct Api {
  pub port: u16,
  pub token: Option<String>,
}

impl Api {
  pub fn from_record(session: &Session) -> Self {
    let root = session.data_root();
    let named = |name: &str| {
      std::env::var(name).ok().or_else(|| {
        let text = std::fs::read_to_string(root.join("env").join("agency.env")).ok()?;
        text.lines().find_map(|line| {
          let (key, value) = line.split_once('=')?;
          (key.trim() == name).then(|| value.trim().to_owned())
        })
      })
    };

    Self {
      port: named("AGENCY_CONTROL_PORT")
        .and_then(|port| port.parse().ok())
        .unwrap_or(DEFAULT_CONTROL_PORT),
      token: named("AGENCY_CONTROL_TOKEN"),
    }
  }

  async fn get(&self, path: &str) -> Result<(reqwest::StatusCode, Value), String> {
    let mut request = reqwest::Client::new()
      .get(format!("http://127.0.0.1:{}{path}", self.port))
      .timeout(Duration::from_secs(10));

    if let Some(token) = &self.token {
      request = request.bearer_auth(token);
    }

    let answered =
      request.send().await.map_err(|why| format!("agency could not be asked {path}: {why}"))?;
    let status = answered.status();

    Ok((status, answered.json().await.unwrap_or(Value::Null)))
  }

  async fn post(
    &self,
    path: &str,
    body: Value,
    bound: Duration,
  ) -> Result<(reqwest::StatusCode, Value), String> {
    let mut request = reqwest::Client::new()
      .post(format!("http://127.0.0.1:{}{path}", self.port))
      .json(&body)
      .timeout(bound);

    if let Some(token) = &self.token {
      request = request.bearer_auth(token);
    }

    let answered =
      request.send().await.map_err(|why| format!("agency could not be asked {path}: {why}"))?;
    let status = answered.status();

    Ok((status, answered.json().await.unwrap_or(Value::Null)))
  }
}

/// The folder a turn works in: the named one, or the asking agent's scratch workspace under the data root.
fn workspace(session: &Session, folder: Option<&str>) -> Result<PathBuf, String> {
  if let Some(folder) = folder.filter(|folder| !folder.trim().is_empty()) {
    let path = PathBuf::from(folder);

    std::fs::create_dir_all(&path).map_err(|why| format!("{} could not be made: {why}", path.display()))?;

    return Ok(path);
  }

  let agent = session
    .variables
    .get(tool_call::AGENT_VARIABLE)
    .cloned()
    .unwrap_or_else(|| "chat".to_owned());
  let path = session.data_root().join("agent-workspaces").join(agent).join("scratch");

  std::fs::create_dir_all(&path).map_err(|why| format!("{} could not be made: {why}", path.display()))?;

  Ok(path)
}

/// Every file path a run's log lines name as changed, under the folder the turn worked in.
fn changed(lines: &[Value], folder: &Path) -> Vec<String> {
  let mut found = BTreeSet::new();
  let prefix = folder.to_string_lossy().into_owned();

  fn walk(value: &Value, prefix: &str, found: &mut BTreeSet<String>) {
    match value {
      Value::Object(map) => {
        for (key, value) in map {
          if matches!(key.as_str(), "path" | "file" | "filePath" | "file_path")
            && let Some(path) = value.as_str()
            && path.starts_with(prefix)
          {
            found.insert(path.to_owned());
          }

          walk(value, prefix, found);
        }
      }
      Value::Array(items) => items.iter().for_each(|item| walk(item, prefix, found)),
      _ => {}
    }
  }

  for line in lines {
    walk(line, &prefix, &mut found);
  }

  found.into_iter().collect()
}

/// One worker ask, answered with the worker's words and the files its turn changed.
pub async fn ask(session: &Session, input: AskInput) -> Answer {
  let api = Api::from_record(session);

  ask_with(&api, session, input).await
}

/// The same ask against the server the caller names, for a test of the refusals and the round trip.
pub async fn ask_with(api: &Api, session: &Session, input: AskInput) -> Answer {
  if input.worker.trim().is_empty() {
    return Answer::failure("worker must not be empty");
  }

  if input.words.trim().is_empty() {
    return Answer::failure("words must not be empty");
  }

  let bound = input
    .time_bound_seconds
    .unwrap_or(DEFAULT_BOUND_SECONDS)
    .clamp(1, DEFAULT_BOUND_SECONDS);
  let owner = session.variables.get("BMO_TURN_PRIVILEGED").is_some_and(|flag| flag == "1");
  let worker = input.worker.trim();
  let (status, described) = match api.get(&format!("/workers/{worker}")).await {
    Ok(answer) => answer,
    Err(why) => return Answer::failure(why),
  };

  if status == reqwest::StatusCode::NOT_FOUND {
    return Answer::failure(format!("no worker is named {worker}"));
  }

  if !status.is_success() {
    return Answer::failure(
      described
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("agency refused the worker")
        .to_owned(),
    );
  }

  let state = described.get("state").and_then(Value::as_str).unwrap_or("idle");

  if state != "idle" && !owner {
    return Answer::failure(format!("{worker} is busy: {state}"));
  }

  let folder = match workspace(session, input.folder.as_deref()) {
    Ok(folder) => folder,
    Err(why) => return Answer::failure(why),
  };
  let cwd = described.get("cwd").and_then(Value::as_str).unwrap_or_default();

  if !owner && Path::new(cwd) == folder {
    return Answer::failure(format!(
      "{worker}'s own checkout may be named only in the owner's turn; name another folder"
    ));
  }

  if described.get("nodeName").and_then(Value::as_str).is_some_and(|node| !node.is_empty()) {
    return Answer::failure(format!(
      "{worker} stands on a node, and this tool names no folder on that node"
    ));
  }

  let Some(session_id) = described.get("sessionId").and_then(Value::as_str).filter(|id| !id.is_empty())
  else {
    return Answer::failure(format!("{worker} holds no session to put a turn into"));
  };
  let since = crate::now_ms();
  let prompt = format!("Work in {}.\n\n{}", folder.display(), input.words.trim());
  let (status, turned) = match api
    .post(
      &format!("/sessions/{session_id}/turns"),
      json!({ "prompt": prompt, "context": { "cwd": folder } }),
      Duration::from_secs(bound),
    )
    .await
  {
    Ok(answer) => answer,
    Err(why) => return Answer::failure(why),
  };

  if !status.is_success() {
    return Answer::failure(
      turned.get("error").and_then(Value::as_str).unwrap_or("the turn was refused").to_owned(),
    );
  }

  let answer = turned
    .get("answer")
    .and_then(Value::as_str)
    .or_else(|| {
      turned
        .get("text")
        .and_then(Value::as_array)
        .and_then(|text| text.first())
        .and_then(Value::as_str)
    })
    .unwrap_or("")
    .to_owned();
  let lines: Vec<Value> = api
    .get(&format!("/log/agents/{worker}/lines?since={since}"))
    .await
    .ok()
    .and_then(|(_, body)| body.get("lines").and_then(Value::as_array).cloned())
    .unwrap_or_default();
  let files = changed(&lines, &folder);
  let mut text = answer.trim_end().to_owned();

  if !files.is_empty() {
    text.push_str("\n\nFiles changed:\n");

    for file in files {
      text.push_str(&format!("- {file}\n"));
    }
  }

  Answer::text(text)
}

pub async fn call(session: &Session, arguments: Value) -> Result<Answer, String> {
  let input: AskInput =
    serde_json::from_value(arguments).map_err(|why| format!("invalid arguments: {NAME}: {why}"))?;

  Ok(ask(session, input).await)
}

pub async fn answer(handed: Handed) -> Result<Value, String> {
  let answered = match handed.asked {
    Asked::Call {
      entry: asked,
      arguments,
    } if asked == entry() => call(&handed.session, arguments).await.unwrap_or_else(Answer::failure),
    Asked::Call { entry: asked, .. } => Answer::failure(format!("this executable holds no entry {asked}")),
    Asked::ResourcesList => return Ok(json!({ "resources": [] })),
    Asked::ResourcesRead { .. } => return Ok(json!({ "error": "this executable serves no resources" })),
    Asked::Other => return Err("this executable answers calls alone".to_owned()),
  };

  serde_json::to_value(answered).map_err(|why| format!("the answer could not be written: {why}"))
}

fn now_ms() -> i64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_or(0, |since| since.as_millis() as i64)
}
