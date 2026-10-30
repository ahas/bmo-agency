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

use bmo_pocket_client::{Failure, NameQuery, Pocket, SessionRequest, ToolCallRequest};
use policy::deployment::DEPLOYMENT;
use policy::research::RESEARCH;
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

/// What a namespace of the tool host lists for a line's instructions: the listing lines its entries
/// declare, in their order, or none where the namespace writes none or the host does not answer.
pub async fn listing(ground: &Ground, namespace: &str) -> Option<String> {
  let socket = host_socket(ground);
  let Some(token) = issuer_token(ground) else {
    eprintln!(
      "mcp-host at {}: no {namespace} listing: the issuer token is not there",
      socket.display()
    );
    return None;
  };
  let client = pocket(ground, token);
  let mut after: Option<String> = None;
  let mut listed: Vec<(i64, String)> = Vec::new();

  loop {
    let page = match client
      .tools(
        namespace,
        &NameQuery {
          after: after.clone(),
          limit: None,
        },
      )
      .await
    {
      Ok(page) => page,
      Err(Failure::Refused { status, .. }) => {
        eprintln!("mcp-host at {}: no {namespace} listing ({status})", socket.display());
        return None;
      }
      Err(why) => {
        eprintln!("mcp-host at {}: no {namespace} listing: {why}", socket.display());
        return None;
      }
    };
    let mut last = None;

    for tool in page.items {
      last = Some(tool.name.clone());

      if let Some(listing) = tool.listing {
        listed.push((listing.order, listing.line));
      }
    }

    match (page.more, last) {
      (true, Some(name)) => after = Some(name),
      _ => break,
    }
  }

  listed.sort_by_key(|(order, _)| *order);

  Some(listed.into_iter().map(|(_, line)| line).collect::<Vec<_>>().join("\n"))
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
  let bundles = match role.as_deref() {
    Some(role) => crate::agent_roles::bundles_of(&ground.db, role).await?,
    None => vec![],
  };
  let mut held: Vec<String> = Vec::new();

  for capability in RUN_LINES.capabilities(line).iter().chain(&bundles) {
    if !held.contains(capability) && !withheld.contains(&capability.as_str()) {
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
  let (role, capabilities) = capabilities(ground, line, agent, &checkout.to_string_lossy(), &[]).await?;

  if capabilities.is_empty() {
    return Ok(None);
  }

  let data_root = ground.roots.data.to_string_lossy().into_owned();
  let mut variables = BTreeMap::from([("BMO_DATA_DIR".to_owned(), data_root)]);

  variables.extend(line_variables(ground, line));

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
/// every such capability left out for a run carrying none. A research run's record names its ledger run as
/// `run`, and where agency's session service answers and the token it takes as `sessionsAt` and
/// `sessionsToken`. A research branch's record carries its parent's settings and `run` held to the budget of
/// one of the parent's branches, opens no branch and names no session service, and names the parent's run as
/// `branchOf`.
pub fn research_record(env: &BTreeMap<String, String>) -> (Option<Value>, Vec<&'static str>) {
  let branch_of = count(env.get(BRANCH_OF)).filter(|run| *run > 0);
  let narrowed;
  let env = match branch_of {
    Some(_) => {
      narrowed = branch_variables(env);
      &narrowed
    }
    None => env,
  };
  let (research, withheld) = settings_record(env);

  match (research, branch_of) {
    (Some(Value::Object(mut research)), Some(parent)) => {
      research.insert("branchOf".to_owned(), json!(parent));

      (Some(Value::Object(research)), withheld)
    }
    (research, _) => (research, withheld),
  }
}

/// The variable that names, on a research branch's run, the ledger's run it is a branch of.
pub const BRANCH_OF: &str = "BMO_RESEARCH_BRANCH_OF";

/// The variable that names a research run's ledger run, which a branch of it carries as its parent's.
pub const RUN: &str = "BMO_RESEARCH_RUN";

/// The variable that names where agency's session service answers a research run's tools.
pub const SESSIONS_AT: &str = "BMO_RESEARCH_SESSIONS_AT";

/// The variable that names the token agency's session service takes from a research run's tools.
pub const SESSIONS_TOKEN: &str = "BMO_RESEARCH_SESSIONS_TOKEN";

// A branch's variables as its record reads them: its parent's settings, its page budget the one each branch
// of the parent has, and no branch of its own nor the session service one is opened through.
fn branch_variables(env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
  let mut narrowed = env.clone();
  let budget = narrowed.remove("BMO_RESEARCH_BRANCH_PAGES");

  for opens in ["BMO_RESEARCH_TOOLS", SESSIONS_AT, SESSIONS_TOKEN] {
    narrowed.remove(opens);
  }

  narrowed.insert("BMO_RESEARCH_BRANCHES".to_owned(), "0".to_owned());

  match budget {
    Some(pages) => narrowed.insert("BMO_RESEARCH_PAGES".to_owned(), pages),
    None => narrowed.remove("BMO_RESEARCH_PAGES"),
  };

  narrowed
}

// The research settings a run's variables carry, and the capabilities those settings do not open.
fn settings_record(env: &BTreeMap<String, String>) -> (Option<Value>, Vec<&'static str>) {
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

  if let Some(run) = count(set(RUN)).filter(|run| *run > 0) {
    research.insert("run".to_owned(), json!(run));
  }

  for (key, variable) in [
    ("owner", "BMO_RESEARCH_OWNER"),
    ("source", "BMO_RESEARCH_SOURCE"),
    ("role", "BMO_RESEARCH_ROLE"),
    ("sessionsAt", SESSIONS_AT),
    ("sessionsToken", SESSIONS_TOKEN),
  ] {
    let value =
      set(variable).filter(|value| key != "source" || ["search", "browse"].contains(&value.as_str()));

    if let Some(value) = value {
      research.insert(key.to_owned(), json!(value));
    }
  }

  research.insert("branches".to_owned(), json!(count(set("BMO_RESEARCH_BRANCHES")).unwrap_or(0)));

  for (key, variable) in [("pages", "BMO_RESEARCH_PAGES"), ("branchPages", "BMO_RESEARCH_BRANCH_PAGES")] {
    if let Some(pages) = count(set(variable)) {
      research.insert(key.to_owned(), json!(pages));
    }
  }
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
/// `deadline` at most; none where the line needs none. Its record carries the run's research settings, and
/// its line's capabilities with its agent's role's bundles, less the web tools those settings do not open.
/// The agent is the turn's where the run names one, and the one it is carried for otherwise.
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
  let (research, withheld) = research_record(env);
  let (role, mut capabilities) = capabilities(ground, line, &agent, cwd, &withheld).await?;

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

  // A research line's brain tools write the chat agent's brain, the agent whose row answers every room that
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

  if let Some(research) = research {
    labels.insert("research".to_owned(), research);
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

/// One call of an entry the tool host keeps for the system alone (docs/research-runs.md, 「What a run is held
/// to: the research settings」), made under a record of its own naming only the entry's namespace and taken
/// back once the call ends: the tool's structured content, or why there is none.
pub async fn system_call(
  ground: &Ground,
  namespace: &str,
  name: &str,
  input: Map<String, Value>,
) -> Result<Value, String> {
  let variables =
    BTreeMap::from([("BMO_DATA_DIR".to_owned(), ground.roots.data.to_string_lossy().into_owned())]);
  let mut labels = Map::new();

  labels.insert("line".to_owned(), json!("system"));

  let request = SessionRequest {
    holder_name: "agency".to_owned(),
    namespaces: BTreeSet::from([namespace.to_owned()]),
    capabilities: Default::default(),
    grants: Vec::new(),
    shown: Default::default(),
    variables,
    variables_file: None,
    labels,
    ttl_ms: Some(SYSTEM_CALL_TTL_MS),
  };
  let token = issue_record(ground, &request).await.map_err(|why| why.detail)?;
  let called = pocket(ground, token.clone())
    .call(&ToolCallRequest {
      namespace: namespace.to_owned(),
      name: name.to_owned(),
      input,
    })
    .await;

  revoke(ground, &token).await;

  let call = called.map_err(|failure| match failure {
    Failure::Refused { status, error } => format!("{status} {} ({})", error.error, error.code.as_str()),
    Failure::Unreadable { status, why } => format!("{status} {why}"),
    Failure::Unreachable(why) => why.to_string(),
  })?;

  if call.result.is_error == Some(true) {
    let said: Vec<&str> = call.result.content.iter().filter_map(|block| block["text"].as_str()).collect();

    return Err(said.join(" "));
  }

  call
    .result
    .structured_content
    .ok_or_else(|| format!("{namespace}/{name} answered no structured content"))
}

// How long the record of one system call may stand where nothing takes its token back.
const SYSTEM_CALL_TTL_MS: u64 = 60_000;

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
