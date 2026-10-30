//! A resident program's session token: issued by the tool
//! host for a record naming the line, the role of the agent it is issued for, the capabilities the line
//! declares with the bundles that role gives, the namespaces it may use and the entries it shows by name,
//! the checkout whose graph is read, the data root and archive, and the place each call reads its run
//! from; carried in the program's own environment and taken back once no written program carries it.
//!
//! The records, the reads of a namespace and the paragraph on finding and using tools go through the tool
//! host's published client (`bmo-pocket-client`), with the issuer token the host keeps under the data root.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bmo_pocket_client::{Failure, Pocket, SessionRequest};
use policy::deployment::DEPLOYMENT;
use policy::roles::ROLES;
use policy::run_lines::RUN_LINES;
use serde_json::{Map, Value, json};

use super::fault;
use crate::Refusal;
use crate::places::Ground;
use store::one;

/// The variable a program carries its token in, which every configuration's bearer header reads.
pub const TOKEN_VARIABLE: &str = "BMO_MCP_TOKEN";

/// The variable a launcher may name the issuer token file in, where a deployment keeps it elsewhere.
pub const ISSUER_TOKEN_VARIABLE: &str = "BMO_MCP_HOST_ISSUER_TOKEN";

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
pub fn host_socket_in(vars: &BTreeMap<String, String>) -> PathBuf {
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

/// The issuer token a launcher asks the host with: the one the environment names, else the host's own
/// under the data root, which it makes at its first start.
fn issuer_token(ground: &Ground) -> Option<String> {
  ground
    .vars()
    .get(ISSUER_TOKEN_VARIABLE)
    .filter(|token| !token.is_empty())
    .cloned()
    .or_else(|| std::fs::read_to_string(ground.roots.data.join("tool-host").join("issuer-token")).ok())
    .map(|token| token.trim().to_owned())
    .filter(|token| !token.is_empty())
}

// The client the issuer's token reaches the host with, where the token stands.
fn pocket(ground: &Ground, token: String) -> Pocket {
  Pocket::at(host_socket(ground), token)
}

/// What a namespace's owner says of it beside its tools, as the tool host keeps it, or none where it says
/// nothing or the host does not answer.
pub async fn facts(ground: &Ground, namespace: &str) -> Option<Value> {
  let socket = host_socket(ground);
  let Some(token) = issuer_token(ground) else {
    eprintln!("mcp-host at {}: no {namespace} facts: the issuer token is not there", socket.display());
    return None;
  };

  match pocket(ground, token).namespace(namespace).await {
    Ok(namespace) => Some(Value::Object(namespace.facts)),
    Err(Failure::Refused { status, .. }) => {
      eprintln!("mcp-host at {}: no {namespace} facts ({status})", socket.display());
      None
    }
    Err(why) => {
      eprintln!("mcp-host at {}: no {namespace} facts: {why}", socket.display());
      None
    }
  }
}

/// The namespaces the tool host saw a client of this session token reach since the token was issued, or none
/// where it saw none, which a host started since then answers as well, or does not answer.
pub async fn connected(ground: &Ground, token: &str) -> Option<Vec<String>> {
  let socket = host_socket(ground);
  let issuer = issuer_token(ground)?;

  match pocket(ground, issuer).tool_session(token).await {
    Ok(session) => {
      let reached: Vec<String> = session.connected.into_iter().map(|reached| reached.namespace).collect();

      (!reached.is_empty()).then_some(reached)
    }
    Err(why) => {
      eprintln!("mcp-host at {}: the session's servers were not read: {why}", socket.display());
      None
    }
  }
}

/// The tool host's paragraph on finding and using tools, as it words it, or why the host gave none: the
/// issuer token is not there, the socket does not answer, or the host refused.
pub async fn discipline(ground: &Ground) -> Result<String, String> {
  let socket = host_socket(ground);
  let Some(token) = issuer_token(ground) else {
    return Err(format!("mcp-host at {}: the issuer token is not there", socket.display()));
  };

  pocket(ground, token)
    .discipline()
    .await
    .map_err(|why| format!("mcp-host at {}: {why}", socket.display()))
}

/// The role and the capabilities of a record issued for `agent` on `line` (docs/agent-roles.md, 「Where the
/// role is read」): the line's capabilities, then the bundles the agent's role gives, each once. An agent
/// the archive holds no row of holds no role, but for a co-worker's place, known by its checkout, which
/// holds the role the policy names for co-workers.
async fn capabilities(
  ground: &Ground,
  line: &str,
  agent: &str,
  checkout: &str,
) -> Result<(Option<String>, Vec<String>), Refusal> {
  // A thrown-away session's place holds its session's agent's role; a branch place the co-workers'.
  let named = match crate::apart_places::session_agent(&ground.roots.data, agent) {
    Some(of_session) => of_session,
    None => agent.to_owned(),
  };
  let role =
    match one(&ground.db, r#"SELECT "role" FROM "agents" WHERE "name" = $1"#, vec![named.into()]).await? {
      Some(row) => row.try_get::<Option<String>>("", "role")?,
      None => crate::places::branch::place_of(&ground.roots.data, checkout).map(|_| ROLES.coworker.clone()),
    };
  let bundles = match role.as_deref() {
    Some(role) => crate::agent_roles::bundles_of(&ground.db, role).await?,
    None => vec![],
  };
  let mut held: Vec<String> = Vec::new();

  for capability in RUN_LINES.capabilities(line).iter().chain(&bundles) {
    if !held.contains(capability) {
      held.push(capability.clone());
    }
  }

  Ok((role, held))
}

// A record for a run of `line`, held by `holder`, with the namespaces and shown entries its line
// declares, an empty grant list, the data root every call reads, and the labels agency's tools read.
fn record(
  line: &str,
  holder: &str,
  capabilities: Vec<String>,
  variables: BTreeMap<String, String>,
  labels: Map<String, Value>,
  ttl_ms: Option<u64>,
) -> SessionRequest {
  SessionRequest {
    holder_name: holder.to_owned(),
    namespaces: RUN_LINES.namespaces(line).iter().cloned().collect::<BTreeSet<_>>(),
    capabilities: capabilities.into_iter().collect(),
    grants: Vec::new(),
    shown: RUN_LINES.shown(line).iter().cloned().collect(),
    variables,
    variables_file: None,
    labels,
    ttl_ms,
  }
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
  let (role, capabilities) = capabilities(ground, line, agent, &checkout.to_string_lossy()).await?;

  if capabilities.is_empty() {
    return Ok(None);
  }

  let data_root = ground.roots.data.to_string_lossy().into_owned();
  let mut variables = BTreeMap::from([("BMO_DATA_DIR".to_owned(), data_root)]);

  variables.extend(line_variables(ground, line));

  // A thrown-away session's place stands no agent row, so its calls name the agent its line's brain tools
  // write for, the chat agent, as a run of the line does (docs/agent-roles.md, 「The searcher role's bundles」).
  if crate::apart_places::session_agent(&ground.roots.data, agent).is_some()
    && RUN_LINES.writes_agent_brain(line)
  {
    let chat = crate::conversational::default_answerer(&ground.db, &DEPLOYMENT.agent_key).await?;

    variables.insert("BMO_TURN_AGENT".to_owned(), chat);
  }

  let mut labels = Map::new();

  labels.insert("line".to_owned(), json!(line));
  labels.insert("checkout".to_owned(), json!(checkout));
  labels.insert("archive".to_owned(), json!(crate::archive_place(&ground.roots.data)));
  labels.insert("place".to_owned(), json!(place));

  if let Some(role) = role {
    labels.insert("role".to_owned(), json!(role));
  }

  let request = record(line, agent, capabilities, variables, labels, None);

  issue_record(ground, &request).await.map(Some)
}

/// The deployment's variables the line hands every record of its runs, each the deployment sets.
pub fn line_variables(ground: &Ground, line: &str) -> Vec<(String, String)> {
  RUN_LINES
    .variables(line)
    .iter()
    .filter_map(|name| {
      let value = ground.vars().get(name).map(|value| value.trim()).filter(|value| !value.is_empty())?;

      Some((name.clone(), value.to_owned()))
    })
    .collect()
}

// The variables of a run's own, which its calls read their turn from.
fn turn_variable(name: &str) -> bool {
  ["BMO_RUN_NAME", "BMO_TASK_LOCKED", "BMO_WORK_ORDER_STEP"].contains(&name)
    || name.starts_with("BMO_TURN_")
}

/// The variable a run carries the labels its thrown-away session's caller named in, a JSON object's text,
/// which its record carries under each key (docs/api.md, 「sessions」).
pub const RECORD_LABELS: &str = "BMO_RECORD_LABELS";

/// The variable a run carries the capabilities its thrown-away session's caller lets its record keep in, a
/// JSON list's text.
pub const RECORD_TOOLS: &str = "BMO_RECORD_TOOLS";

/// The variables a thrown-away session's turn carries of the record its caller named: its labels and the
/// capabilities it keeps, each where the start named it.
pub fn record_variables(apart: &agent_session::Apart) -> Vec<(String, String)> {
  let mut variables = Vec::new();

  if let Some(labels) = &apart.labels {
    variables.push((RECORD_LABELS.to_owned(), labels.clone()));
  }

  if let Some(allowed) = &apart.tools_allowed {
    variables.push((RECORD_TOOLS.to_owned(), json!(allowed).to_string()));
  }

  variables
}

/// Whether a run's variables name a record of its caller's: labels or the capabilities it keeps.
pub fn names_record(env: &BTreeMap<String, String>) -> bool {
  [RECORD_LABELS, RECORD_TOOLS]
    .iter()
    .any(|name| env.get(*name).is_some_and(|value| !value.is_empty()))
}

// The labels a run's caller named, an object's keys and values; none where it named none or they do not read
// as an object.
fn named_labels(env: &BTreeMap<String, String>) -> Map<String, Value> {
  env
    .get(RECORD_LABELS)
    .and_then(|text| serde_json::from_str::<Value>(text).ok())
    .and_then(|value| match value {
      Value::Object(labels) => Some(labels),
      _ => None,
    })
    .unwrap_or_default()
}

// The capabilities a run's caller lets its record keep; none where it named no list, which keeps them all.
fn named_tools(env: &BTreeMap<String, String>) -> Option<Vec<String>> {
  env.get(RECORD_TOOLS).and_then(|text| serde_json::from_str::<Vec<String>>(text).ok())
}

/// A token for one run of `line` carried in `agent`'s host, working in `cwd` on `env`, standing until
/// `deadline` at most; none where the line needs none. Its record carries its line's capabilities with its
/// agent's role's bundles, narrowed to those the run's caller lets it keep, and the labels that caller
/// named. The agent is the turn's where the run names one, and the one it is carried for otherwise.
pub async fn issue_for_run(
  ground: &Ground,
  line: &str,
  agent: &str,
  cwd: &str,
  env: &BTreeMap<String, String>,
  deadline: u64,
) -> Result<Option<String>, Refusal> {
  let given = |name: &str| env.get(name).filter(|value| !value.is_empty()).cloned();
  let agent = given("BMO_TURN_AGENT").map_or_else(|| agent.to_owned(), |named| named.trim().to_owned());
  let (role, mut capabilities) = capabilities(ground, line, &agent, cwd).await?;

  if let Some(allowed) = named_tools(env) {
    capabilities.retain(|capability| allowed.contains(capability));
  }

  if capabilities.is_empty() {
    return Ok(None);
  }

  let data_root = given("BMO_DATA_DIR").unwrap_or_else(|| ground.roots.data.to_string_lossy().into_owned());
  let mut variables: BTreeMap<String, String> = env
    .iter()
    .filter(|(name, _)| turn_variable(name))
    .map(|(name, value)| (name.clone(), value.clone()))
    .collect();

  variables.insert("BMO_DATA_DIR".to_owned(), data_root);
  variables.extend(line_variables(ground, line));

  // A runner line's brain tools write the chat agent's brain, the agent whose row answers every room that
  // names nobody: the record names that agent as the turn its tools run for, while the run itself stays the
  // runner's (docs/agent-roles.md, 「The searcher role's bundles」).
  if RUN_LINES.writes_agent_brain(line) {
    let chat = crate::conversational::default_answerer(&ground.db, &DEPLOYMENT.agent_key).await?;

    variables.insert("BMO_TURN_AGENT".to_owned(), chat);
  }

  let mut labels = Map::new();
  labels.insert("line".to_owned(), json!(line));
  labels.insert("checkout".to_owned(), json!(cwd));
  labels.insert("archive".to_owned(), json!(crate::archive_place(&ground.roots.data)));

  if let Some(role) = role {
    labels.insert("role".to_owned(), json!(role));
  }

  if let Some(place) = given("BMO_AGENT_PLACE") {
    labels.insert("place".to_owned(), json!(place));
  }

  for (key, value) in named_labels(env) {
    labels.entry(key).or_insert(value);
  }

  let now = u64::try_from(crate::now_ms()).unwrap_or_default();
  let request = record(line, &agent, capabilities, variables, labels, Some(deadline.saturating_sub(now)));

  issue_record(ground, &request).await.map(Some)
}

// A token asked of the host for a record.
async fn issue_record(ground: &Ground, request: &SessionRequest) -> Result<String, Refusal> {
  let socket = host_socket(ground);
  let deadline = Instant::now() + ISSUE_WAIT;

  loop {
    let why = match issuer_token(ground) {
      None => "the issuer token is not there".to_owned(),
      Some(token) => match pocket(ground, token).issue(request).await {
        Ok(session) => return Ok(session.token),
        Err(Failure::Refused { status, error }) => {
          return Err(host_down(&socket, format!("{status} {} ({})", error.error, error.code.as_str())));
        }
        Err(Failure::Unreadable { status, why }) => {
          return Err(host_down(&socket, format!("{status} {why}")));
        }
        Err(Failure::Unreachable(why)) => why.to_string(),
      },
    };

    if Instant::now() >= deadline {
      return Err(host_down(&socket, why));
    }

    tokio::time::sleep(RETRY_EVERY).await;
  }
}

fn host_down(socket: &Path, why: impl Into<String>) -> Refusal {
  fault(format!(
    "The tool host did not answer at {}, so nothing ran: {}",
    socket.display(),
    why.into()
  ))
}

/// A token taken back; a host that does not answer has lost it already.
pub async fn revoke(ground: &Ground, token: &str) {
  let Some(issuer) = issuer_token(ground) else {
    return;
  };

  let _ = pocket(ground, issuer).revoke(token).await;
}

/// The checkout a program's tool session reads its graph from: its own, or for a program on a work
/// node the shadow its link keeps of that checkout's graph here (docs/work-nodes.md, 「The link」).
pub fn record_checkout(ground: &Ground, place: &Path, cwd: &str) -> PathBuf {
  match crate::watches::links::place_node(place) {
    None => PathBuf::from(cwd),
    Some(node) => crate::watches::links::checkout_shadow(&ground.roots.data, &node, cwd),
  }
}
