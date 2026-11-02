//! `delegate`: a supervisor hands a task to a run of a role, with no queue item: a slot of the role asked —
//! the delegator role, the supervisor's double, where the ask names none — and waited for in its line however
//! long that takes, one run of the role at the task's folder on the provider and model the slot carries and
//! waited on with no bound, the slot finished, and the run's words answered with the files it created or
//! changed in the task's folder; and the asking supervisor's delegations listed by their titles and stages
//! (docs/tools.md, 「The delegate」; docs/role-slots.md, 「A delegated task」).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use tool_call::room_gate::{self, AccountService, room_turn_of};
use tool_call::{Answer, Asked, Handed, Session, Turn};

/// The entry the manifest declares.
pub const NAMESPACE: &str = "bmo";
pub const NAME: &str = "delegate";
pub const TITLE: &str = "Delegate a task";
pub const DESCRIPTION: &str = "Hand a task that needs no queue item to a run of a role — your own double where you name none — under a one-line title, run with no time limit, and answer with what it wrote and the files it created or changed: run as bmo use bmo/delegate --async so its end wakes you in the room.";
pub const CAPABILITIES: &[&str] = &["room.tools"];

/// The entry that lists the asking supervisor's delegations.
pub const LIST_NAME: &str = "delegations";
pub const LIST_TITLE: &str = "List my delegations";
pub const LIST_DESCRIPTION: &str = "List the tasks you delegated: each one's title, its stage (waiting for a slot, running, ended), its role, the provider and model it runs on, when it was asked and, once ended, what it came to; the waiting and running ones first, then the latest ended.";

/// The source a delegated task's slot is asked under (docs/role-slots.md, 「A delegated task」).
pub const SOURCE: &str = "delegate";

/// How often a waiting slot asks again for its admission: a pace, never a bound on how long it waits.
pub const LOOK_EVERY: Duration = Duration::from_secs(1);

const DEFAULT_CONTROL_PORT: u16 = 3220;
const CONTROL_PORT: &str = "AGENCY_CONTROL_PORT";
const CONTROL_TOKEN: &str = "AGENCY_CONTROL_TOKEN";

// A slot's states it is let in at, and those it has been given back in.
const ADMITTED: [&str; 2] = ["active", "bound"];
const GIVEN_BACK: [&str; 2] = ["released", "cancelled"];

pub fn entry() -> String {
  format!("{NAMESPACE}/{NAME}")
}

pub fn list_entry() -> String {
  format!("{NAMESPACE}/{LIST_NAME}")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegateInput {
  /// One line naming the task, which the delegation is listed by.
  pub title: String,
  pub instruction: String,
  #[serde(default)]
  pub role: Option<String>,
  #[serde(default)]
  pub folder: Option<String>,
}

/// Agency's server as a placed tool reaches it: the port and the token the tool's own environment names, else
/// those the data root's `env/agency.env` names (docs/api.md, 「The control port」), and the pace a waiting
/// slot asks again at.
pub struct Api {
  pub port: u16,
  pub token: Option<String>,
  pub pace: Duration,
}

impl Api {
  pub fn from_record(session: &Session) -> Self {
    Self::read(&session.data_root(), |name| std::env::var(name).ok())
  }

  /// The server the data root's `env/agency.env` describes, each variable `variable` answers standing first.
  pub fn read(data_root: &Path, variable: impl Fn(&str) -> Option<String>) -> Self {
    let file: Vec<(String, String)> = dotenvy::from_path_iter(data_root.join("env").join("agency.env"))
      .map(|pairs| pairs.filter_map(Result::ok).collect())
      .unwrap_or_default();
    let named = |name: &str| {
      variable(name)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| file.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone()))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    };

    Self {
      port: named(CONTROL_PORT).and_then(|port| port.parse().ok()).unwrap_or(DEFAULT_CONTROL_PORT),
      // The token by value, else the file its `_FILE` variable names.
      token: named(CONTROL_TOKEN).or_else(|| {
        named(&format!("{CONTROL_TOKEN}_FILE"))
          .and_then(|file| std::fs::read_to_string(file).ok())
          .map(|token| token.trim().to_owned())
          .filter(|token| !token.is_empty())
      }),
      pace: LOOK_EVERY,
    }
  }

  // One call to agency's server, answered whenever it answers: no timeout cuts the wait.
  async fn call(
    &self,
    method: Method,
    path: &str,
    body: Option<Value>,
  ) -> Result<(StatusCode, Value), String> {
    let mut request =
      reqwest::Client::new().request(method.clone(), format!("http://127.0.0.1:{}{path}", self.port));

    if let Some(body) = body {
      request = request.json(&body);
    }

    if let Some(token) = &self.token {
      request = request.bearer_auth(token);
    }

    let answered = request
      .send()
      .await
      .map_err(|why| format!("agency could not be asked {method} {path}: {why}"))?;
    let status = answered.status();

    Ok((status, answered.json().await.unwrap_or(Value::Null)))
  }

  // A call whose answer must be a success: its body, or the error agency gave.
  async fn ok(&self, method: Method, path: &str, body: Option<Value>, what: &str) -> Result<Value, String> {
    let (status, said) = self.call(method, path, body).await?;

    if status.is_success() {
      return Ok(said);
    }

    let why = said.get("error").and_then(Value::as_str).unwrap_or("no reason given");

    Err(format!("{what}: agency answered {status}: {why}"))
  }
}

/// A slot as the role's line answers it: its token, its state, the role it is of and the provider, model and
/// effort it is let in on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seat {
  pub token: String,
  pub state: String,
  pub role: String,
  pub provider: String,
  pub model: String,
  pub effort: Option<String>,
}

impl Seat {
  fn of(said: &Value) -> Option<Self> {
    let text = |field: &str| said.get(field).and_then(Value::as_str).filter(|value| !value.is_empty());

    Some(Self {
      token: text("token")?.to_owned(),
      state: text("state").unwrap_or_default().to_owned(),
      role: text("role").unwrap_or_default().to_owned(),
      provider: text("provider").unwrap_or_default().to_owned(),
      model: text("model").unwrap_or_default().to_owned(),
      effort: text("effort").map(str::to_owned),
    })
  }

  fn admitted(&self) -> bool {
    ADMITTED.contains(&self.state.as_str())
  }

  fn given_back(&self) -> bool {
    GIVEN_BACK.contains(&self.state.as_str())
  }
}

/// What a delegation holds of agency's at the moment: its slot and whether its run was sent, so a call
/// stopped midway gives back what it would otherwise leave standing.
#[derive(Debug, Default)]
pub struct Held {
  state: Mutex<HeldNow>,
}

#[derive(Clone, Debug, Default)]
struct HeldNow {
  seat: Option<String>,
  sent: bool,
}

impl Held {
  fn now(&self) -> std::sync::MutexGuard<'_, HeldNow> {
    self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  /// Give back what a stopped call holds: a slot whose run was never sent is cancelled, since it never ran,
  /// and one whose run was sent is finished, the run going on to its end in agency whether or not this call
  /// still waits.
  pub async fn give_back(&self, api: &Api) {
    let now = self.now().clone();
    let Some(seat) = now.seat else { return };

    if now.sent {
      finish(api, &seat, "stopped: the call went away while its run went on", false).await;
    } else {
      finish(api, &seat, "cancelled: the call was stopped", true).await;
    }
  }
}

/// The folder a task works in: the named one, or the asking supervisor's scratch workspace under the data root.
fn workspace(session: &Session, folder: Option<&str>) -> Result<PathBuf, String> {
  let path = match folder.map(str::trim).filter(|folder| !folder.is_empty()) {
    Some(folder) => PathBuf::from(folder),
    None => session.data_root().join("agent-workspaces").join(asker(session)).join("scratch"),
  };

  std::fs::create_dir_all(&path).map_err(|why| format!("{} could not be made: {why}", path.display()))?;

  Ok(path)
}

// The supervisor the asking turn is, else the record's own, else the chat supervisor.
fn asker(session: &Session) -> String {
  Turn::of(session)
    .agent
    .or_else(|| session.variables.get(tool_call::AGENT_VARIABLE).cloned())
    .map(|agent| agent.trim().to_owned())
    .filter(|agent| !agent.is_empty())
    .unwrap_or_else(|| "chat".to_owned())
}

// A piece of a request key: every character the key's shape does not hold made `-`.
fn keyed(text: &str) -> String {
  text
    .chars()
    .map(|letter| if letter.is_ascii_alphanumeric() || "._-".contains(letter) { letter } else { '-' })
    .collect()
}

/// Every file under a folder with its length and the moment it was last written, its `.git` directories
/// passed over.
fn files(folder: &Path) -> BTreeMap<PathBuf, (u64, Option<SystemTime>)> {
  let mut found = BTreeMap::new();
  let mut open = vec![folder.to_path_buf()];

  while let Some(directory) = open.pop() {
    let Ok(entries) = std::fs::read_dir(&directory) else { continue };

    for entry in entries.flatten() {
      let path = entry.path();
      let Ok(kind) = entry.file_type() else { continue };

      if kind.is_dir() {
        if entry.file_name() != ".git" {
          open.push(path);
        }
      } else if let Ok(meta) = entry.metadata() {
        found.insert(path, (meta.len(), meta.modified().ok()));
      }
    }
  }

  found
}

/// The files created or changed between two readings of a folder.
fn changed(
  before: &BTreeMap<PathBuf, (u64, Option<SystemTime>)>,
  after: &BTreeMap<PathBuf, (u64, Option<SystemTime>)>,
) -> Vec<PathBuf> {
  after
    .iter()
    .filter(|(path, now)| before.get(*path) != Some(now))
    .map(|(path, _)| path.clone())
    .collect()
}

/// One delegation, past the owner's gate: answered with the run's words and the files it created or
/// changed, or why it could not be done. A termination signal while it runs gives back what it holds.
pub async fn delegate(session: &Session, input: DelegateInput) -> Answer {
  let accounts = AccountService::read(&session.data_root(), |name| std::env::var(name).ok());

  if let Err(refused) = may_delegate(&accounts, session).await {
    return Answer::failure(refused);
  }

  let api = Api::from_record(session);
  let held = Held::default();

  tokio::select! {
    answered = delegate_with(&api, session, input, &held) => answered,
    () = stopped() => {
      held.give_back(&api).await;

      Answer::failure("the delegation was stopped before its task answered")
    }
  }
}

// A termination signal, the way a caller that goes away ends a call; never where none can be listened for.
async fn stopped() {
  use tokio::signal::unix::{SignalKind, signal};

  let (Ok(mut terminate), Ok(mut interrupt), Ok(mut hang_up)) = (
    signal(SignalKind::terminate()),
    signal(SignalKind::interrupt()),
    signal(SignalKind::hangup()),
  ) else {
    return std::future::pending().await;
  };

  tokio::select! {
    _ = terminate.recv() => {}
    _ = interrupt.recv() => {}
    _ = hang_up.recv() => {}
  }
}

/// Whether the turn the call is made in may delegate (docs/launch-lines.md, 「The owner's gate」): the owner's
/// own turn, or one whose every message's sender the account service answers holds `launch` in the room.
pub async fn may_delegate(accounts: &AccountService, session: &Session) -> Result<(), String> {
  room_gate::gate(accounts, room_turn_of(session).as_ref(), room_gate::LAUNCH, "Delegating a task").await
}

/// The same delegation against the server the caller names, keeping what it holds in `held`: the slot, the
/// run and the folder's files read before and after it.
pub async fn delegate_with(api: &Api, session: &Session, input: DelegateInput, held: &Held) -> Answer {
  let instruction = input.instruction.trim();

  if instruction.is_empty() {
    return Answer::failure("instruction must not be empty");
  }

  let title = input.title.split_whitespace().collect::<Vec<_>>().join(" ");

  if title.is_empty() {
    return Answer::failure("title must not be empty");
  }

  // No role named leaves it to agency: the delegator role, on the asking supervisor's own road.
  let role = input.role.as_deref().map(str::trim).filter(|role| !role.is_empty());
  let folder = match workspace(session, input.folder.as_deref()) {
    Ok(folder) => folder,
    Err(why) => return Answer::failure(why),
  };
  let turn = Turn::of(session);
  let asker = asker(session);
  let key = format!("{SOURCE}:{}:{}:{}", keyed(&asker), now_ms(), std::process::id());
  let mut asked = json!({
    "requestKey": key,
    "owner": key,
    "source": SOURCE,
    "title": title,
    "askedBy": asker,
    "path": folder.display().to_string(),
  });

  if let Some(role) = role {
    asked["role"] = json!(role);
  }

  let seat = match api.ok(Method::POST, "/slot-requests", Some(asked), "no slot was asked").await {
    Ok(said) => match Seat::of(&said) {
      Some(seat) => seat,
      None => return Answer::failure("agency answered the slot with no token"),
    },
    Err(why) => return Answer::failure(why),
  };

  held.now().seat = Some(seat.token.clone());

  let seat = match admitted(api, seat).await {
    Ok(seat) => seat,
    Err(why) => {
      held.give_back(api).await;

      return Answer::failure(why);
    }
  };
  // The role's run at the task's folder, on the road the slot was let in on (docs/api.md, 「sessions」).
  let start = json!({
    "scope": "role",
    "agentName": seat.role,
    "options": {
      "role": seat.role,
      "provider": seat.provider,
      "model": seat.model,
      "cwd": folder.display().to_string(),
    },
  });
  let started = match api.ok(Method::POST, "/sessions", Some(start), "the role's run was not started").await
  {
    Ok(said) => said,
    Err(why) => {
      finish(api, &seat.token, "cancelled: its run was not started", true).await;

      return Answer::failure(why);
    }
  };
  let Some(run_session) = started.get("id").and_then(Value::as_str).map(str::to_owned) else {
    finish(api, &seat.token, "cancelled: its run was not started", true).await;

    return Answer::failure("agency answered the role's run with no session");
  };
  let run = format!("{SOURCE}-{}-{}", keyed(&asker), now_ms());
  // The slot is bound to the run, so its line reads it standing while the run goes.
  let bound = json!({ "state": "bound", "childId": run, "agentName": run_session });

  if let Err(why) = api
    .ok(
      Method::PATCH,
      &format!("/slot-requests/{}", seat.token),
      Some(bound),
      "the slot was not bound",
    )
    .await
  {
    finish(api, &seat.token, "cancelled: not bound", true).await;

    return Answer::failure(why);
  }

  let body = json!({
    "prompt": format!("Work in {}.\n\n{instruction}", folder.display()),
    "context": {
      "work": {
        "provider": seat.provider,
        "model": seat.model,
        "mapping": { "model": seat.model, "effort": seat.effort },
        "resume": { "kind": "auto" },
        "cwd": folder.display().to_string(),
        "name": run,
        "title": format!("delegated by {asker}: {title}"),
        "chat": turn.chat.unwrap_or_default(),
        "workKey": seat.role,
        "hearsWords": false,
        "variables": {},
      },
    },
  });
  let before = files(&folder);

  held.now().sent = true;

  let ended = send_turn(api, &run_session, &body).await;
  let after = files(&folder);
  let answered = ended.and_then(|ended| {
    let answer = ended.get("answer").and_then(Value::as_str).unwrap_or_default().trim().to_owned();

    if answer.is_empty() {
      let outcome = ended.get("outcome").and_then(Value::as_str).unwrap_or("no outcome");

      return Err(format!("the delegated task ended with no answer ({outcome})"));
    }

    Ok(answer)
  });
  let outcome = match &answered {
    Ok(_) => "answered".to_owned(),
    Err(why) => format!("failed: {why}"),
  };

  finish(api, &seat.token, &outcome, false).await;

  let mut text = match answered {
    Ok(answer) => answer,
    Err(why) => return Answer::failure(why),
  };
  let made = changed(&before, &after);

  if !made.is_empty() {
    text.push_str("\n\nFiles created or changed:\n");

    for file in made {
      text.push_str(&format!("- {}\n", file.display()));
    }
  }

  Answer::text(text)
}

/// One run of the role, answered once it ends however long that takes.
async fn send_turn(api: &Api, session: &str, body: &Value) -> Result<Value, String> {
  api
    .ok(
      Method::POST,
      &format!("/sessions/{session}/turns"),
      Some(body.clone()),
      "the run did not come back",
    )
    .await
}

/// The slot let in, asked for its admission at the pace again and again however long the line takes; or why
/// it never will be: it was given back before it was let in, or agency could not be asked.
async fn admitted(api: &Api, mut seat: Seat) -> Result<Seat, String> {
  let path = format!("/slot-requests/{}", seat.token);

  loop {
    if seat.admitted() {
      return Ok(seat);
    }

    if seat.given_back() {
      return Err(format!("the slot was given back ({}) before it was let in", seat.state));
    }

    tokio::time::sleep(api.pace).await;

    let said = api.ok(Method::PATCH, &path, Some(json!({})), "the slot could not be let in").await?;

    seat = match said.get("request").and_then(Seat::of) {
      Some(now) => now,
      // Not let in now: read where it stands, so a slot given back is told from one still waiting.
      None => match Seat::of(&api.ok(Method::GET, &path, None, "the slot could not be read").await?) {
        Some(now) if now.admitted() => Seat {
          state: "waiting".to_owned(),
          ..now
        },
        Some(now) => now,
        None => return Err("agency answered the slot with no token".to_owned()),
      },
    };
  }
}

// The slot given back with what its run came to, cancelled where it never ran.
async fn finish(api: &Api, token: &str, outcome: &str, cancel: bool) {
  let body = json!({ "state": "finished", "outcome": outcome, "cancel": cancel });

  if let Err(why) = api
    .ok(
      Method::PATCH,
      &format!("/slot-requests/{token}"),
      Some(body),
      "the slot was not given back",
    )
    .await
  {
    eprintln!("{why}");
  }
}

pub async fn call(session: &Session, arguments: Value) -> Result<Answer, String> {
  let input: DelegateInput =
    serde_json::from_value(arguments).map_err(|why| format!("invalid arguments: {NAME}: {why}"))?;

  Ok(delegate(session, input).await)
}

/// What the listing is asked: how many of the latest ended delegations to name beside the open ones.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListInput {
  #[serde(default)]
  pub ended: Option<u32>,
}

/// The asking supervisor's delegations as agency answers them, one line each: the title, the stage, the role
/// and the road it runs on, when it was asked and what it came to once ended.
pub async fn list_with(api: &Api, session: &Session, input: ListInput) -> Answer {
  let asker = asker(session);
  let mut path = format!("/delegations?askedBy={}", encoded(&asker));

  if let Some(ended) = input.ended {
    path.push_str(&format!("&ended={ended}"));
  }

  let said = match api.ok(Method::GET, &path, None, "the delegations could not be read").await {
    Ok(said) => said,
    Err(why) => return Answer::failure(why),
  };
  let items = said.get("items").and_then(Value::as_array).cloned().unwrap_or_default();

  if items.is_empty() {
    return Answer::text("You have delegated nothing that is listed.");
  }

  let lines: Vec<String> = items.iter().map(listed).collect();

  Answer::text(lines.join("\n"))
}

// One delegation as a line of the listing.
fn listed(item: &Value) -> String {
  let text = |field: &str| item.get(field).and_then(Value::as_str).filter(|value| !value.is_empty());
  let title = text("title").unwrap_or("(untitled)");
  let stage = match text("stage") {
    Some("waiting") => "waiting for a slot".to_owned(),
    Some("ended") => format!("ended: {}", text("outcome").unwrap_or("no outcome")),
    other => other.unwrap_or("unknown").to_owned(),
  };
  let role = text("role").unwrap_or("?");
  let road = match (text("provider"), text("model")) {
    (Some(provider), Some(model)) => format!(" on {provider} {model}"),
    _ => String::new(),
  };
  let asked = item.get("askedAt").and_then(Value::as_i64).map_or_else(|| "?".to_owned(), moment);

  format!("- {title} — {stage} — {role}{road} — asked {asked}")
}

// A moment in epoch milliseconds as a UTC time a reader takes in at a glance.
fn moment(ms: i64) -> String {
  chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
    .map_or_else(|| ms.to_string(), |at| at.format("%Y-%m-%d %H:%M UTC").to_string())
}

// A query value with every byte outside the unreserved set percent-encoded.
fn encoded(text: &str) -> String {
  text
    .bytes()
    .map(|byte| {
      if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
        (byte as char).to_string()
      } else {
        format!("%{byte:02X}")
      }
    })
    .collect()
}

pub async fn list(session: &Session, arguments: Value) -> Result<Answer, String> {
  let input: ListInput = if arguments.is_null() {
    ListInput::default()
  } else {
    serde_json::from_value(arguments).map_err(|why| format!("invalid arguments: {LIST_NAME}: {why}"))?
  };

  Ok(list_with(&Api::from_record(session), session, input).await)
}

pub async fn answer(handed: Handed) -> Result<Value, String> {
  let answered = match handed.asked {
    Asked::Call {
      entry: asked,
      arguments,
    } if asked == entry() => call(&handed.session, arguments).await.unwrap_or_else(Answer::failure),
    Asked::Call {
      entry: asked,
      arguments,
    } if asked == list_entry() => list(&handed.session, arguments).await.unwrap_or_else(Answer::failure),
    Asked::Call { entry: asked, .. } => Answer::failure(format!("this executable holds no entry {asked}")),
    Asked::ResourcesList => return Ok(json!({ "resources": [] })),
    Asked::ResourcesRead { .. } => return Ok(json!({ "error": "this executable serves no resources" })),
    Asked::Other => return Err("this executable answers calls alone".to_owned()),
  };

  serde_json::to_value(answered).map_err(|why| format!("the answer could not be written: {why}"))
}

fn now_ms() -> i64 {
  SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .map_or(0, |since| since.as_millis() as i64)
}
