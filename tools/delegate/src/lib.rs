//! `delegate`: a chat agent hands a task to the helper, with no queue item: a co-worker seat asked of the pool
//! under the helper role and waited for in its line however long that takes, one turn sent into a helper
//! agent's own worker session on the seat's model and waited on with no bound, the session going on from the
//! last delegation while its cache holds, the seat finished, and the turn's words answered with the files it
//! created or changed in the task's folder; and the asking agent's delegations listed by their titles and
//! stages (docs/tools.md, 「The delegate」; docs/agent-control.md, 「Delegating a task to the helper」).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use policy::roles::ROLES;
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use tool_call::room_gate::{self, AccountService, room_turn_of};
use tool_call::{Answer, Asked, Handed, Session, Turn};

/// The entry the manifest declares.
pub const NAMESPACE: &str = "bmo";
pub const NAME: &str = "delegate";
pub const TITLE: &str = "Delegate a task";
pub const DESCRIPTION: &str = "Hand a task that needs no queue item to the helper on a co-worker seat, under a one-line title, run with no time limit, and answer with what it wrote and the files it created or changed: run as bmo use bmo/delegate --async so its end wakes you in the room.";
pub const CAPABILITIES: &[&str] = &["room.tools"];

/// The entry that lists the asking agent's delegations.
pub const LIST_NAME: &str = "delegations";
pub const LIST_TITLE: &str = "List my delegations";
pub const LIST_DESCRIPTION: &str = "List the tasks you delegated: each one's title, its stage (waiting for a seat, running on a helper, ended), the helper it runs on, when it was asked and, once ended, what it came to; the waiting and running ones first, then the latest ended.";

/// The source a delegated task's seat is asked under (docs/coworker-pool.md, 「Who takes a place」).
pub const SOURCE: &str = "delegate";

/// How often a waiting seat asks again for its admission, and a turn answered busy is sent again: a pace,
/// never a bound on how long either waits.
pub const LOOK_EVERY: Duration = Duration::from_secs(1);

// The provider the seat is asked on and the turn goes out on: a worker's own session, the one kept per model
// and login, is Claude Code's (docs/sessions.md, 「A worker's session per model」).
const PROVIDER: &str = "claude-code";
const DEFAULT_CONTROL_PORT: u16 = 3220;
const CONTROL_PORT: &str = "AGENCY_CONTROL_PORT";
const CONTROL_TOKEN: &str = "AGENCY_CONTROL_TOKEN";

// The code agency answers a turn sent into a session already in one with.
const SESSION_BUSY: &str = "session-busy";

// A seat's states it is let in at, and those it has been given back in.
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
/// seat asks again at.
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

/// A seat as the pool answers it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seat {
  pub token: String,
  pub state: String,
  pub model: String,
  pub effort: Option<String>,
  pub tier: Option<String>,
  pub work_key: String,
}

impl Seat {
  fn of(said: &Value) -> Option<Self> {
    let text = |field: &str| said.get(field).and_then(Value::as_str).filter(|value| !value.is_empty());

    Some(Self {
      token: text("token")?.to_owned(),
      state: text("state").unwrap_or_default().to_owned(),
      model: text("model").unwrap_or_default().to_owned(),
      effort: text("effort").map(str::to_owned),
      tier: text("tier").map(str::to_owned),
      work_key: text("workKey").unwrap_or_default().to_owned(),
    })
  }

  fn admitted(&self) -> bool {
    ADMITTED.contains(&self.state.as_str())
  }

  fn given_back(&self) -> bool {
    GIVEN_BACK.contains(&self.state.as_str())
  }
}

/// What a delegation holds of agency's at the moment: its seat and whether its turn was sent, so a call
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

  /// Give back what a stopped call holds: a seat whose turn was never sent is cancelled, since it never ran,
  /// and one whose turn was sent is finished, the turn going on to its end in agency whether or not this
  /// call still waits.
  pub async fn give_back(&self, api: &Api) {
    let now = self.now().clone();
    let Some(seat) = now.seat else { return };

    if now.sent {
      finish(api, &seat, "stopped: the call went away while its turn went on", false).await;
    } else {
      finish(api, &seat, "cancelled: the call was stopped", true).await;
    }
  }
}

/// The folder a task works in: the named one, or the asking agent's scratch workspace under the data root.
fn workspace(session: &Session, folder: Option<&str>) -> Result<PathBuf, String> {
  let path = match folder.map(str::trim).filter(|folder| !folder.is_empty()) {
    Some(folder) => PathBuf::from(folder),
    None => session.data_root().join("agent-workspaces").join(asker(session)).join("scratch"),
  };

  std::fs::create_dir_all(&path).map_err(|why| format!("{} could not be made: {why}", path.display()))?;

  Ok(path)
}

// The agent the asking turn is, else the record's own, else the chat agent's line.
fn asker(session: &Session) -> String {
  Turn::of(session)
    .agent
    .or_else(|| session.variables.get(tool_call::AGENT_VARIABLE).cloned())
    .map(|agent| agent.trim().to_owned())
    .filter(|agent| !agent.is_empty())
    .unwrap_or_else(|| "chat".to_owned())
}

// A piece of a pool key: every character the key's shape does not hold made `-`.
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

/// One delegation, past the owner's gate: answered with the co-worker's words and the files it created or
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

/// The same delegation against the server the caller names, keeping what it holds in `held`: the agent that
/// carries it, the seat, the turn and the folder's files read before and after it.
pub async fn delegate_with(api: &Api, session: &Session, input: DelegateInput, held: &Held) -> Answer {
  let instruction = input.instruction.trim();

  if instruction.is_empty() {
    return Answer::failure("instruction must not be empty");
  }

  let title = input.title.split_whitespace().collect::<Vec<_>>().join(" ");

  if title.is_empty() {
    return Answer::failure("title must not be empty");
  }

  let role = input
    .role
    .as_deref()
    .map(str::trim)
    .filter(|role| !role.is_empty())
    .unwrap_or(&ROLES.delegate)
    .to_owned();
  let folder = match workspace(session, input.folder.as_deref()) {
    Ok(folder) => folder,
    Err(why) => return Answer::failure(why),
  };
  let agent = match carrier(api, &role).await {
    Ok(agent) => agent,
    Err(why) => return Answer::failure(why),
  };
  let turn = Turn::of(session);
  let asker = asker(session);
  let key = format!("{SOURCE}:{}:{}:{}", keyed(&asker), now_ms(), std::process::id());
  let asked = json!({
    "requestKey": key,
    "owner": key,
    "role": role,
    "selection": "auto",
    "source": SOURCE,
    "provider": PROVIDER,
    "title": title,
    "askedBy": asker,
  });
  let seat = match api.ok(Method::POST, "/seat-requests", Some(asked), "the pool gave no seat").await {
    Ok(said) => match Seat::of(&said) {
      Some(seat) => seat,
      None => return Answer::failure("the pool answered the seat with no token"),
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
  let began = now_ms();
  let run = format!("{SOURCE}-{}-{began}", keyed(&asker));
  // The seat is bound to the turn's run, which the turn's execution names, so the pool reads it standing while
  // the turn goes (docs/coworker-pool.md, 「Requests and leases」).
  let bound = json!({ "state": "bound", "childId": run, "agentName": agent.name });

  if let Err(why) = api
    .ok(
      Method::PATCH,
      &format!("/seat-requests/{}", seat.token),
      Some(bound),
      "the seat was not bound",
    )
    .await
  {
    finish(api, &seat.token, "cancelled: not bound", true).await;

    return Answer::failure(why);
  }

  // The turn stands in the agent's own checkout, the folder its session is kept in, so a delegation on the
  // same model within the cache lifetime goes on in the session the last one left; the task's folder is
  // named in its words.
  let body = json!({
    "prompt": format!("Work in {}.\n\n{instruction}", folder.display()),
    "provider": PROVIDER,
    "model": seat.model,
    "mapping": { "model": seat.model, "effort": seat.effort, "tier": seat.tier },
    "resume": { "kind": "auto" },
    "cwd": agent.cwd,
    "name": run,
    "title": format!("delegated by {asker}"),
    "chat": turn.chat.unwrap_or_default(),
    "workKey": seat.work_key,
    "hearsWords": false,
    "variables": {},
  });
  let before = files(&folder);

  held.now().sent = true;

  let ended = send_turn(api, &agent.session, &body).await;
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

/// The agent a delegation goes to: its name, the session its turns go into and the checkout that session is
/// kept in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Carrier {
  pub name: String,
  pub session: String,
  pub cwd: String,
}

/// The agent holding the role that carries the task: of the workers holding it that stand on this machine and
/// do not rest, a free one first, else the first by name, whose turn the delegation then waits for.
pub async fn carrier(api: &Api, role: &str) -> Result<Carrier, String> {
  let said = api.ok(Method::GET, "/workers", None, "the workers could not be read").await?;
  let text = |worker: &Value, field: &str| {
    worker.get(field).and_then(Value::as_str).map(str::trim).unwrap_or_default().to_owned()
  };
  let mut holding: Vec<&Value> = said
    .get("items")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .filter(|worker| text(worker, "role") == role)
    .filter(|worker| text(worker, "nodeName").is_empty())
    .filter(|worker| worker.get("restingSince").is_none_or(Value::is_null))
    .collect();

  holding.sort_by_key(|worker| (text(worker, "state") != "idle", text(worker, "name")));

  let Some(worker) = holding.first() else {
    return Err(format!("no agent on this machine holds the {role} role, so nobody can carry the task"));
  };
  let name = text(worker, "name");
  let session = Some(text(worker, "sessionId"))
    .filter(|id| !id.is_empty())
    .unwrap_or(format!("{name}@worker"));

  Ok(Carrier {
    cwd: text(worker, "cwd"),
    name,
    session,
  })
}

/// One turn into the agent's session, answered once it ends however long that takes; a send answered busy,
/// the session being in a turn of its own, is sent again at the pace until the session takes it.
async fn send_turn(api: &Api, session: &str, body: &Value) -> Result<Value, String> {
  let path = format!("/sessions/{session}/turns");

  loop {
    let (status, said) = api.call(Method::POST, &path, Some(body.clone())).await?;

    if status.is_success() {
      return Ok(said);
    }

    if status == StatusCode::CONFLICT && said.get("code").and_then(Value::as_str) == Some(SESSION_BUSY) {
      tokio::time::sleep(api.pace).await;
      continue;
    }

    let why = said.get("error").and_then(Value::as_str).unwrap_or("no reason given");

    return Err(format!("the turn did not come back: agency answered {status}: {why}"));
  }
}

/// The seat let in, asked for its admission at the pace again and again however long the line takes; or why
/// it never will be: the pool gave it back before it was let in, or agency could not be asked.
async fn admitted(api: &Api, mut seat: Seat) -> Result<Seat, String> {
  let path = format!("/seat-requests/{}", seat.token);

  loop {
    if seat.admitted() {
      return Ok(seat);
    }

    if seat.given_back() {
      return Err(format!("the pool gave the seat back ({}) before it was let in", seat.state));
    }

    tokio::time::sleep(api.pace).await;

    let said = api.ok(Method::PATCH, &path, Some(json!({})), "the seat could not be let in").await?;

    seat = match said.get("request").and_then(Seat::of) {
      Some(now) => now,
      // Not let in now: read where it stands, so a seat given back is told from one still waiting.
      None => match Seat::of(&api.ok(Method::GET, &path, None, "the seat could not be read").await?) {
        Some(now) if now.admitted() => Seat {
          state: "waiting".to_owned(),
          ..now
        },
        Some(now) => now,
        None => return Err("agency answered the seat with no token".to_owned()),
      },
    };
  }
}

// The seat given back with what its work came to, cancelled where it never ran.
async fn finish(api: &Api, token: &str, outcome: &str, cancel: bool) {
  let body = json!({ "state": "finished", "outcome": outcome, "cancel": cancel });

  if let Err(why) = api
    .ok(
      Method::PATCH,
      &format!("/seat-requests/{token}"),
      Some(body),
      "the seat was not given back",
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

/// The asking agent's delegations as agency answers them, one line each: the title, the stage and the helper
/// it runs on, when it was asked and what it came to once ended.
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
  let stage = match (text("stage"), text("agentName")) {
    (Some("waiting"), _) => "waiting for a seat".to_owned(),
    (Some("running"), Some(agent)) => format!("running on {agent}"),
    (Some("running"), None) => "running".to_owned(),
    (Some("ended"), _) => format!("ended: {}", text("outcome").unwrap_or("no outcome")),
    (other, _) => other.unwrap_or("unknown").to_owned(),
  };
  let asked = item.get("askedAt").and_then(Value::as_i64).map_or_else(|| "?".to_owned(), moment);
  let mut line = format!("- {title} — {stage} — asked {asked}");

  if let (Some("ended"), Some(agent)) = (text("stage"), text("agentName")) {
    line.push_str(&format!(" — ran on {agent}"));
  }

  line
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
