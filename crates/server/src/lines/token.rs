//! A resident program's session token: issued by the tool
//! host for a record naming the line, the role of the agent it is issued for, the capabilities the line
//! declares with the bundles that role gives, the checkout whose graph is read, the data root and archive,
//! and the place each call reads its run from; carried in the program's own environment and taken back
//! once no written program carries it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use axum::http::{Method, Request, header};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use policy::research::RESEARCH;
use policy::roles::ROLES;
use policy::run_lines::RUN_LINES;
use serde_json::{Value, json};
use tokio::net::UnixStream;

use super::fault;
use crate::Refusal;
use crate::places::Ground;
use store::one;

/// The variable a program carries its token in, which every configuration's bearer header reads.
pub const TOKEN_VARIABLE: &str = "BMO_MCP_TOKEN";

// How long a launch waits for a host restarting under a deployment before it refuses, and how often it
// asks again meanwhile.
const ISSUE_WAIT: Duration = Duration::from_secs(10);
const RETRY_EVERY: Duration = Duration::from_millis(200);

/// The tool host's control socket: the one the environment names, or `bmo/mcp-host.sock` under the
/// user's runtime directory.
pub fn host_socket(ground: &Ground) -> PathBuf {
  host_socket_in(ground.vars())
}

/// The tool host's control socket as these variables name it.
pub fn host_socket_in(vars: &std::collections::BTreeMap<String, String>) -> PathBuf {
  if let Some(named) = vars.get("BMO_MCP_HOST_SOCKET") {
    return PathBuf::from(named);
  }

  let runtime = vars.get("XDG_RUNTIME_DIR").map_or_else(
    // SAFETY: `getuid` reads the process's own user id and cannot fail.
    || PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })),
    PathBuf::from,
  );

  runtime.join("bmo").join("mcp-host.sock")
}

// One request to the host's control socket: its status and its body as JSON. Failing to reach the
// socket at all is an error of its own, which a caller waiting for a restarting host tells apart.
async fn ask(
  socket: &Path,
  method: Method,
  path: &str,
  body: Option<&Value>,
) -> std::io::Result<(u16, Value)> {
  let (status, body) = asked(socket, method, path, body).await?;

  Ok((status, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

/// What a namespace of the tool host lists for a line's instructions:
/// its listing, or none where the namespace writes none or the host does not answer.
pub async fn listing(ground: &Ground, namespace: &str) -> Option<String> {
  let socket = host_socket(ground);

  match asked(&socket, Method::GET, &format!("/listings/{namespace}"), None).await {
    Ok((200, body)) => Some(String::from_utf8_lossy(&body).into_owned()),
    Ok((status, _)) => {
      eprintln!("mcp-host at {}: no {namespace} listing ({status})", socket.display());
      None
    }
    Err(why) => {
      eprintln!("mcp-host at {}: no {namespace} listing: {why}", socket.display());
      None
    }
  }
}

/// What a namespace's owner says of it beside its tools, as the tool host keeps it, or none where it says
/// nothing or the host does not answer.
pub async fn facts(ground: &Ground, namespace: &str) -> Option<Value> {
  let socket = host_socket(ground);

  match ask(&socket, Method::GET, &format!("/facts/{namespace}"), None).await {
    Ok((200, body)) => Some(body),
    Ok((status, _)) => {
      eprintln!("mcp-host at {}: no {namespace} facts ({status})", socket.display());
      None
    }
    Err(why) => {
      eprintln!("mcp-host at {}: no {namespace} facts: {why}", socket.display());
      None
    }
  }
}

/// One request to the host's control socket: its status and its body as it came.
pub(crate) async fn asked(
  socket: &Path,
  method: Method,
  path: &str,
  body: Option<&Value>,
) -> std::io::Result<(u16, Bytes)> {
  let stream = UnixStream::connect(socket).await?;
  let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
    .await
    .map_err(std::io::Error::other)?;

  tokio::spawn(connection);

  let mut asked = Request::builder().method(method).uri(path).header(header::HOST, "localhost");
  let bytes = match body {
    Some(body) => {
      asked = asked.header(header::CONTENT_TYPE, "application/json");
      Bytes::from(serde_json::to_vec(body)?)
    }
    None => Bytes::new(),
  };
  let response = sender
    .send_request(asked.body(Full::new(bytes)).map_err(std::io::Error::other)?)
    .await
    .map_err(std::io::Error::other)?;
  let status = response.status().as_u16();
  let body = response.into_body().collect().await.map_err(std::io::Error::other)?.to_bytes();

  Ok((status, body))
}

/// The role and the capabilities of a record issued for `agent` on `line` (docs/agent-roles.md, 「Where the
/// role is read」): the line's capabilities, then the bundles the agent's role gives, each once, less what
/// `withheld` names. An agent the archive holds no row of holds no role, but for a co-worker's place, known
/// by its checkout, which holds the role the policy names for co-workers.
async fn capabilities(
  ground: &Ground,
  line: &str,
  agent: &str,
  checkout: &str,
  withheld: &[&str],
) -> Result<(Option<String>, Vec<String>), Refusal> {
  let role =
    match one(&ground.db, r#"SELECT "role" FROM "agents" WHERE "name" = $1"#, vec![agent.into()]).await? {
      Some(row) => row.try_get::<Option<String>>("", "role")?,
      None => crate::places::branch::place_of(&ground.roots.data, checkout).map(|_| ROLES.coworker.clone()),
    };
  let bundles = role.as_deref().map_or(&[][..], |role| ROLES.bundles_of(role));
  let mut held: Vec<String> = Vec::new();

  for capability in RUN_LINES.capabilities(line).iter().chain(bundles) {
    if !held.contains(capability) && !withheld.contains(&capability.as_str()) {
      held.push(capability.clone());
    }
  }

  Ok((role, held))
}

/// A token for a resident program of `line` standing in `place` for `agent` and working in `checkout`, or
/// none where the line needs none: one whose record holds no capability. A host that does not answer
/// within its wait, or one that refuses the record, refuses the program.
pub async fn issue(
  ground: &Ground,
  line: &str,
  agent: &str,
  place: &Path,
  checkout: &Path,
) -> Result<Option<String>, Refusal> {
  let (role, capabilities) = capabilities(ground, line, agent, &checkout.to_string_lossy(), &[]).await?;

  if capabilities.is_empty() {
    return Ok(None);
  }

  let mut record = json!({
    "line": line,
    "capabilities": capabilities,
    "checkout": checkout,
    "dataRoot": ground.roots.data,
    "archive": crate::archive_place(&ground.roots.data),
    "place": place,
  });

  if let Some(role) = role {
    record["role"] = json!(role);
  }

  issue_record(ground, &record).await.map(Some)
}

// The variables of a run's own, which its calls read their turn from.
fn turn_variable(name: &str) -> bool {
  ["BMO_RUN_NAME", "BMO_TASK_LOCKED", "BMO_WORK_ORDER_STEP"].contains(&name)
    || name.starts_with("BMO_TURN_")
}

// A count a research variable holds: a whole number no smaller than nothing, as a script's `Number` reads
// it; none for anything else.
fn count(value: Option<&String>) -> Option<i64> {
  let read = value.filter(|value| !value.is_empty())?.trim().parse::<f64>().ok()?;

  (read.fract() == 0.0 && (0.0..=9_007_199_254_740_991.0).contains(&read)).then_some(read as i64)
}

// The number a chat variable leads with, as a script's `parseInt` reads it; nothing read is no chat.
fn leading_number(value: Option<&String>) -> i64 {
  let text = value.map_or("", |value| value.trim_start());
  let (sign, digits) = match text.strip_prefix('-') {
    Some(rest) => (-1, rest),
    None => (1, text.strip_prefix('+').unwrap_or(text)),
  };
  let digits: String = digits.chars().take_while(char::is_ascii_digit).collect();

  digits.parse::<i64>().map_or(0, |read| sign * read)
}

/// What a run's session record carries of its research settings,
/// read out of the run's environment where its runner put them, and the capabilities the policy ties to
/// those settings that they do not open; no settings and
/// every such capability left out for a run carrying none.
pub fn research_record(
  env: &std::collections::BTreeMap<String, String>,
) -> (Option<Value>, Vec<&'static str>) {
  let set = |name: &str| env.get(name).filter(|value| !value.is_empty());

  if !env.iter().any(|(name, value)| {
    (name.starts_with("BMO_RESEARCH_") || name.starts_with("BMO_WEB_")) && !value.is_empty()
  }) {
    return (None, RESEARCH.withheld(false, |_| None));
  }

  let rulebooks: Vec<&str> = set("BMO_RESEARCH_RULEBOOKS")
    .map(|value| value.split(',').filter(|name| !name.is_empty()).collect())
    .unwrap_or_default();
  let mut research = serde_json::Map::new();

  if let Some(at) = count(set("BMO_RESEARCH_CLOSES_AT")) {
    research.insert("closesAt".to_owned(), json!(at));
  }

  for (key, variable) in [
    ("owner", "BMO_RESEARCH_OWNER"),
    ("source", "BMO_RESEARCH_SOURCE"),
    ("role", "BMO_RESEARCH_ROLE"),
  ] {
    let value =
      set(variable).filter(|value| key != "source" || ["search", "browse"].contains(&value.as_str()));

    if let Some(value) = value {
      research.insert(key.to_owned(), json!(value));
    }
  }

  research.insert("branches".to_owned(), json!(count(set("BMO_RESEARCH_BRANCHES")).unwrap_or(0)));
  research.insert("chat".to_owned(), json!(leading_number(set("BMO_RESEARCH_CHAT"))));

  if let Some(strategy) = set("BMO_WEB_STRATEGY") {
    research.insert("strategy".to_owned(), json!(strategy));
  }

  for (key, variable) in [
    ("hostIntervalMs", "BMO_WEB_HOST_INTERVAL_MS"),
    ("engineIntervalMs", "BMO_WEB_ENGINE_INTERVAL_MS"),
  ] {
    if let Some(interval) = count(set(variable)) {
      research.insert(key.to_owned(), json!(interval));
    }
  }

  research.insert("rulebooks".to_owned(), json!(rulebooks));

  let withheld = RESEARCH.withheld(true, |variable| env.get(variable).map(String::as_str));

  (Some(Value::Object(research)), withheld)
}

/// A token for one run of `line` carried in `agent`'s host, working in `cwd` on `env`, standing until
/// `deadline` at most; none where the line needs none. Its record carries the run's research settings, and
/// its line's capabilities with its agent's role's bundles, less the web tools those settings do not open.
/// The agent is the turn's where the run names one, and the one it is carried for otherwise.
pub async fn issue_for_run(
  ground: &Ground,
  line: &str,
  agent: &str,
  cwd: &str,
  env: &std::collections::BTreeMap<String, String>,
  deadline: u64,
) -> Result<Option<String>, Refusal> {
  let given = |name: &str| env.get(name).filter(|value| !value.is_empty()).cloned();
  let agent = given("BMO_TURN_AGENT").map_or_else(|| agent.to_owned(), |named| named.trim().to_owned());
  let (research, withheld) = research_record(env);
  let (role, capabilities) = capabilities(ground, line, &agent, cwd, &withheld).await?;

  if capabilities.is_empty() {
    return Ok(None);
  }

  let mut record = json!({
    "line": line,
    "capabilities": capabilities,
    "checkout": cwd,
    "dataRoot": given("BMO_DATA_DIR").unwrap_or_else(|| ground.roots.data.to_string_lossy().into_owned()),
    "archive": crate::archive_place(&ground.roots.data),
  });
  let run_variables: serde_json::Map<String, Value> = env
    .iter()
    .filter(|(name, _)| turn_variable(name))
    .map(|(name, value)| (name.clone(), json!(value)))
    .collect();

  if let Some(role) = role {
    record["role"] = json!(role);
  }

  if let Some(place) = given("BMO_AGENT_PLACE") {
    record["place"] = json!(place);
  }

  if !run_variables.is_empty() {
    record["runVariables"] = Value::Object(run_variables);
  }

  if let Some(desk) = given("BMO_FRAGMENT_DESK") {
    record["fragmentDesk"] = json!(desk);
  }

  if let Some(research) = research {
    record["research"] = research;
  }

  record["deadline"] = json!(deadline);

  issue_record(ground, &record).await.map(Some)
}

// A token asked of the host for a record.
async fn issue_record(ground: &Ground, record: &Value) -> Result<String, Refusal> {
  let socket = host_socket(ground);
  let deadline = Instant::now() + ISSUE_WAIT;

  loop {
    let why = match ask(&socket, Method::POST, "/sessions", Some(record)).await {
      Ok((status, said)) => {
        if let (200..=299, Some(token)) = (status, said.get("token").and_then(Value::as_str)) {
          return Ok(token.to_owned());
        }

        let detail = said.get("detail").and_then(Value::as_str).unwrap_or_default();

        return Err(host_down(&socket, format!("{status} {detail}").trim()));
      }
      Err(why) => why.to_string(),
    };

    if Instant::now() >= deadline {
      return Err(host_down(&socket, &why));
    }

    tokio::time::sleep(RETRY_EVERY).await;
  }
}

fn host_down(socket: &Path, why: &str) -> Refusal {
  fault(format!("The tool host did not answer at {}, so nothing ran: {why}", socket.display()))
}

/// A token taken back; a host that does not answer has lost it already.
pub async fn revoke(ground: &Ground, token: &str) {
  let path = format!(
    "/sessions/{}",
    percent_encoding::utf8_percent_encode(token, percent_encoding::NON_ALPHANUMERIC)
  );

  let _ = ask(&host_socket(ground), Method::DELETE, &path, None).await;
}

/// The checkout a program's tool session reads its graph from: its own, or for a program on a work
/// node the shadow its link keeps of that checkout's graph here (docs/work-nodes.md, 「The link」).
pub fn record_checkout(ground: &Ground, place: &Path, cwd: &str) -> PathBuf {
  match crate::watches::links::place_node(place) {
    None => PathBuf::from(cwd),
    Some(node) => crate::watches::links::checkout_shadow(&ground.roots.data, &node, cwd),
  }
}
