//! `GET /agents`: the agent table as the store holds it (docs/api.md, 「Agents」), every agent, the resting
//! among them, each with what it is configured to begin on, what of its own thinking and effort reaches
//! that, its gates and the rooms it holds sessions in, and what a recollector reads of it: whether it is
//! awake, its brain folder and the registration its recollectors run on.

use std::path::Path;
use std::sync::LazyLock;

use axum::http::{Method, StatusCode};
use axum::response::Response;
use harness_providers::{ARCHIVE_PROVIDERS, Machine};
use policy::reasoning::REASONING;
use policy::roles::ROLES;
use policy::work_keys::WORK_KEYS;
use regex::Regex;
use sea_orm::{DatabaseConnection, QueryResult};
use serde_json::{Map, Value, json};

use super::agent_switches::{owns_reasoning, turn_gates_of};
use super::claude_accounts::Judging;
use super::harness::{enabled_providers, levels, line_provider, named, provider_enabled};
use super::roads::{self, LAYA};
use super::{Call, Refused, Route, answer, registry, setting, tiers};
use crate::conversational::{Scope, Settings};
use store::{all, one};

// A job's choice as the job launcher names one: a provider this build carries and a model of it.
static JOB_CHOICE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^([a-z0-9-]+)::([a-zA-Z0-9][a-zA-Z0-9._:/-]{0,200})$").expect("the job choice")
});

/// `GET /agents`.
pub struct Agents;

impl Route for Agents {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/agents";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let db = call.store();
    let judging = Judging::of(call.ground());
    let rows = store::all(db, r#"SELECT * FROM "agents" ORDER BY "kind", "name""#, vec![]).await?;
    let mut agents = Vec::with_capacity(rows.len());

    for row in &rows {
      agents.push(agent_view(judging, call.machine(), &Agent::of(row)?).await?);
    }

    Ok(answer(StatusCode::OK, json!({ "items": agents, "roles": roles_view(), "more": false })))
  }
}

crate::moved!(Agents);

/// One agent's row as the boundary answers it, reset point and stand-in included.
pub async fn view_named(
  db: &DatabaseConnection,
  ground: &crate::places::Ground,
  machine: &Machine,
  name: &str,
) -> Result<Value, Refused> {
  let row = store::one(db, r#"SELECT * FROM "agents" WHERE "name" = $1"#, vec![name.into()])
    .await?
    .ok_or_else(|| Refused::new(StatusCode::NOT_FOUND, format!("no agent is named {name}")))?;
  let agent = Agent::of(&row)?;
  let judging = Judging::of(ground);
  let mut view = agent_view(judging, machine, &agent).await?;

  if let Some(view) = view.as_object_mut() {
    view.insert("resetAt".into(), json!(crate::companion::overtaken::point(db, &agent.name).await));
    view.insert(
      "standInAvailable".into(),
      json!(crate::stand_in::answers(ground, machine, &agent.name).await),
    );
  }

  Ok(view)
}

/// One agent: role, gates, reasoning, voice, reset point, stand-in.
pub struct AgentRoute;

impl Route for AgentRoute {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/agents/{agentName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("agentName").unwrap_or_default().to_owned();
    let db = call.store().clone();
    let ground = call.shared_ground();
    let machine = call.machine().clone();

    Ok(answer(StatusCode::OK, view_named(&db, &ground, &machine, &name).await?))
  }
}

crate::moved!(AgentRoute);

/// Every role an agent may hold, with the bundles each gives by name and label, as the screen offers
/// them (docs/agent-roles.md).
fn roles_view() -> Value {
  json!(
    ROLES
      .roles
      .iter()
      .map(|role| json!({
        "name": role.name,
        "label": role.label,
        "bundles": role.bundles.iter().map(|bundle| json!({ "name": bundle, "label": ROLES.bundle_label(bundle) })).collect::<Vec<_>>(),
      }))
      .collect::<Vec<_>>()
  )
}

/// One agent's row as the store's `agents` table holds it.
pub struct Agent {
  pub name: String,
  pub kind: String,
  role: Option<String>,
  pub last_mode: Option<String>,
  pub last_model: Option<String>,
  pub last_effort: Option<String>,
  pub label: Option<String>,
  aliases: Option<String>,
  icon: Option<String>,
  pub cwd: Option<String>,
  pub node: Option<String>,
  preamble: Option<String>,
  dev_command: Option<String>,
  builtin: bool,
  pub chat_id: Option<i64>,
  pub session_id: Option<String>,
  pub runs: i64,
  pub last_run_at: Option<i64>,
  pub last_keepalive_at: Option<i64>,
  context_tokens: Option<i64>,
  pub keepalive_stopped: bool,
  pub keepalive_paused_at: Option<i64>,
  pub downgrade_heal: bool,
  pub support_count: i64,
  pub support_keepalive: i64,
  pub support_of: Option<String>,
  pub support_slot: Option<i64>,
  pinned_provider: Option<String>,
  pinned_model: Option<String>,
  voice: Option<String>,
  thinking: Option<bool>,
  effort: Option<String>,
  /// Whether it is awake: no rest stands on it.
  pub awake: bool,
  /// Where its long-term memory lives, relative to the brain root, empty for the root; none where it keeps
  /// none.
  pub brain_folder: Option<String>,
  /// How the runner carries it, where it is a conversational agent.
  pub conversation: Option<Settings>,
}

impl Agent {
  pub fn of(row: &QueryResult) -> Result<Self, Refused> {
    let text = |column: &str| row.try_get::<Option<String>>("", column);
    let number = |column: &str| row.try_get::<Option<i64>>("", column);
    let pinned_provider = text("pinned_provider")?;

    Ok(Self {
      name: row.try_get("", "name")?,
      kind: row.try_get("", "kind")?,
      role: text("role")?,
      last_mode: text("last_mode")?,
      last_model: text("last_model")?,
      last_effort: text("last_effort")?,
      label: text("label")?,
      aliases: text("aliases")?,
      icon: text("icon")?,
      cwd: text("cwd")?,
      node: text("node")?,
      preamble: text("preamble")?,
      dev_command: text("dev_command")?,
      builtin: number("builtin")? == Some(1),
      chat_id: number("chat_id")?,
      session_id: text("session_id")?,
      runs: number("runs")?.unwrap_or_default(),
      last_run_at: number("last_run_at")?,
      last_keepalive_at: number("last_keepalive_at")?,
      context_tokens: number("context_tokens")?,
      keepalive_stopped: number("keepalive_stopped")? == Some(1),
      keepalive_paused_at: number("keepalive_paused_at")?,
      downgrade_heal: number("downgrade_heal")? != Some(0),
      support_count: number("support_count")?.unwrap_or_default(),
      support_keepalive: number("support_keepalive")?.unwrap_or_default(),
      support_of: text("support_of")?,
      support_slot: number("support_slot")?,
      pinned_model: if pinned_provider.is_some() { text("pinned_model")? } else { None },
      pinned_provider,
      voice: text("voice")?,
      thinking: number("thinking")?.map(|thinking| thinking != 0),
      effort: text("effort")?,
      awake: number("resting_since")?.is_none(),
      brain_folder: text("brain_folder")?.map(|folder| folder.trim().trim_end_matches('/').to_owned()),
      conversation: Settings::of(row).map_err(Refused::from)?,
    })
  }

  fn is_worker(&self) -> bool {
    self.kind == "worker"
  }
}

/// What an agent is configured to begin on: the provider its first road is on and the model there,
/// none where the key stands on Laya, whose checkpoint is the service's to name.
pub struct Configured {
  pub provider: String,
  pub model: Option<String>,
  pub laya: bool,
}

// Whether a directory is a git checkout.
fn is_checkout(path: &str) -> bool {
  Path::new(path).join(".git").exists()
}

async fn agent_view(judging: Judging<'_>, machine: &Machine, agent: &Agent) -> Result<Value, Refused> {
  let db = judging.db;
  let configured = configured(db, agent).await;
  let (configured_label, model_label) = start_labels(db, configured.as_ref()).await?;
  let mut view = Map::new();
  let mut put = |key: &str, value: Value| {
    view.insert(key.to_owned(), value);
  };

  put("name", json!(agent.name));
  put("kind", json!(agent.kind));
  put("role", json!(agent.role));
  put("sessionScope", json!(agent.conversation.as_ref().map(|settings| settings.scope.word())));
  put("turnShape", json!(agent.conversation.as_ref().map(|settings| settings.shape.word())));
  put(
    "defaultAnswerer",
    json!(agent.conversation.as_ref().is_some_and(|settings| settings.default_answerer)),
  );
  put("model", json!(configured.as_ref().and_then(|one| one.model.clone())));
  put("configuredMode", json!(configured.as_ref().map(|one| one.provider.clone())));
  put("configuredLabel", json!(configured_label));
  put("modelLabel", json!(model_label));
  put("lastMode", json!(agent.last_mode));
  put("lastModel", json!(agent.last_model));
  put("lastEffort", json!(agent.last_effort));
  put("label", json!(agent.label));
  put("awake", json!(agent.awake));
  put("brainFolder", json!(agent.brain_folder));
  put(
    "recollectorRegistration",
    json!(crate::run_clock::recollector_registration(db, &agent.name).await?),
  );
  put("aliases", json!(agent.aliases));
  put("icon", json!(agent.icon));
  put("cwd", json!(agent.cwd));

  if agent.is_worker() {
    put("node", json!(agent.node));

    // Whether its directory is a git checkout, which decides whether its jobs may co-work; a directory
    // on a node is the node's to say.
    if let (Some(cwd), None) = (&agent.cwd, &agent.node) {
      put("checkout", json!(is_checkout(cwd)));
    }
  }

  put("preamble", json!(agent.preamble));
  put("devCommand", json!(agent.dev_command));
  put("builtin", json!(agent.builtin));
  put("chatId", json!(agent.chat_id));
  put("runs", json!(agent.runs));
  put("lastRunAt", json!(agent.last_run_at));
  put("lastKeepaliveAt", json!(agent.last_keepalive_at));
  put("contextTokens", json!(agent.context_tokens));
  put("keepaliveStopped", json!(agent.keepalive_stopped));
  put("keepalivePausedAt", json!(agent.keepalive_paused_at));

  if agent.is_worker() {
    put("downgradeHeal", json!(agent.downgrade_heal));
    put("supportCount", json!(agent.support_count));
    put("supportKeepalive", json!(agent.support_keepalive));
    put("supportOf", json!(agent.support_of));
    put("supportSlot", json!(agent.support_slot));
    put("pinnedProvider", json!(agent.pinned_provider));
    put("pinnedModel", json!(agent.pinned_model));
  }

  put("reasoning", reasoning_view(judging, machine, agent, configured.as_ref()).await);
  put("gates", gates_view(db, agent).await?);
  put("voice", json!(agent.voice));

  let sessions = room_sessions(db, agent).await?;

  if !sessions.is_empty() {
    put("sessions", json!(sessions));
  }

  Ok(Value::Object(view))
}

// What an agent begins on, read the one way every screen reads it: out of the shared tier settings for its
// work key (docs/model-tiers.md). A worker is the worker key on the provider it is pinned to or the first
// its walk asks; a conversational agent pinned to a provider its pin; the deployment's chat, a gate and an
// editor their key's road; each research runner its key on the first provider carrying its line. Anything
// the settings refuse to answer leaves the agent with none.
pub async fn configured(db: &DatabaseConnection, agent: &Agent) -> Option<Configured> {
  match agent.kind.as_str() {
    "worker" => worker_choice(db, &agent.name).await.ok().flatten(),
    _ if agent.conversation.is_some() && agent.pinned_provider.is_some() => {
      agent.pinned_provider.clone().map(|provider| Configured {
        provider,
        model: agent.pinned_model.clone(),
        laya: false,
      })
    }
    _ => match roads::profile(&agent.name).map(|profile| profile.line.as_str()) {
      Some("chat" | "gate" | "editor") => road_choice(db, &agent.name).await.ok(),
      _ if agent.kind == "runner" && (agent.name == "search" || agent.name == "browse") => {
        research_choice(db, &agent.name).await.ok()
      }
      _ => None,
    },
  }
}

// One key's road: the provider it begins on and what its tier resolves to there.
async fn road_choice(db: &DatabaseConnection, key: &str) -> Result<Configured, Refused> {
  let provider = roads::road_provider(db, key).await?;
  let model = tiers::resolve(db, key, &provider)
    .await
    .map(|mapping| mapping.model)
    .unwrap_or_default();
  let laya = roads::selected_mode(db, key).await? == LAYA;

  Ok(Configured {
    provider,
    model: (!laya && !model.is_empty()).then_some(model),
    laya,
  })
}

async fn research_choice(db: &DatabaseConnection, key: &str) -> Result<Configured, String> {
  let provider = line_provider(db, key).await?;
  let mapping = tiers::resolve_execution(db, key, &provider, None).await?;

  Ok(Configured {
    provider,
    model: Some(mapping.model),
    laya: false,
  })
}

// The first of a worker's selections, as the job launcher walks them: one model where its pin names
// one, otherwise the worker key's tier on its pinned provider, the key's own pinned provider, or each
// provider its walk asks in the owner's order. A provider the settings refuse along the way refuses
// them all, as the launcher's walk does.
async fn worker_choice(db: &DatabaseConnection, worker: &str) -> Result<Option<Configured>, Refused> {
  let Some(key) = WORK_KEYS.profile.iter().find(|profile| profile.kind == "worker") else {
    return Ok(None);
  };
  let pin = worker_pin(db, worker).await?;

  if let Some((provider, Some(model))) = &pin {
    return Ok(job_choice(&format!("{provider}::{model}")));
  }

  let pinned = match pin {
    Some((provider, _)) => Some(provider),
    None => roads::pinned_provider(db, &key.key).await?,
  };
  let order = match pinned {
    Some(provider) => vec![provider],
    None => {
      let mut walked = Vec::new();

      for name in enabled_providers(db).await? {
        if roads::walked(db, named(&name)?, &key.key, &key.line).await? {
          walked.push(name);
        }
      }

      walked
    }
  };
  let mut selections = Vec::with_capacity(order.len());

  for provider in order {
    let mapping = tiers::resolve_execution(db, &key.key, &provider, None).await.map_err(Refused::failed)?;

    selections.push(format!("{provider}::{}", mapping.model));
  }

  Ok(selections.first().and_then(|first| job_choice(first)))
}

// What a worker is pinned to, a support worker by its base's pin: a provider the owner has on, and the
// one model of it where its models are registrations named one by one.
async fn worker_pin(
  db: &DatabaseConnection,
  worker: &str,
) -> Result<Option<(String, Option<String>)>, Refused> {
  let pin_of = async |name: &str| {
    one(
      db,
      r#"SELECT "support_of", "pinned_provider", "pinned_model" FROM "agents" WHERE "name" = $1"#,
      vec![name.into()],
    )
    .await
  };
  let Some(row) = pin_of(worker).await? else { return Ok(None) };
  let base: Option<String> = row.try_get("", "support_of")?;
  let owner = match base {
    Some(base) => pin_of(&base).await?,
    None => Some(row),
  };
  let Some(owner) = owner else { return Ok(None) };
  let Some(provider) = owner.try_get::<Option<String>>("", "pinned_provider")? else {
    return Ok(None);
  };

  if !provider_enabled(db, &provider).await? {
    return Ok(None);
  }

  let model = if named(&provider)?.not_registered().is_some() {
    owner.try_get::<Option<String>>("", "pinned_model")?
  } else {
    None
  };

  Ok(Some((provider, model)))
}

pub fn job_choice(selection: &str) -> Option<Configured> {
  let found = JOB_CHOICE.captures(selection)?;
  let provider = ARCHIVE_PROVIDERS.iter().find(|name| **name == &found[1])?;

  Some(Configured {
    provider: (*provider).to_owned(),
    model: Some(found[2].to_owned()),
    laya: false,
  })
}

// What a screen calls the provider and the model an agent begins on: the provider's own label, and the
// registration's where the model is one; none where the agent begins on nothing, or on Laya.
async fn start_labels(
  db: &DatabaseConnection,
  configured: Option<&Configured>,
) -> Result<(Option<String>, Option<String>), Refused> {
  let Some(configured) = configured.filter(|one| !one.laya) else {
    return Ok((None, None));
  };
  let provider = named(&configured.provider)?;
  let model_label = match (&configured.model, provider.unregistered()) {
    (Some(model), Some(_)) => registry::service_of(db, model).await?.map(|service| service.label),
    _ => None,
  };

  Ok((Some(provider.declared().label.clone()), model_label))
}

// What the agent's row names of its own thinking and effort, and what of it reaches the provider and
// model it begins on; null for an agent no choice of its own reaches.
async fn reasoning_view(
  judging: Judging<'_>,
  machine: &Machine,
  agent: &Agent,
  configured: Option<&Configured>,
) -> Value {
  if !owns_reasoning(&agent.name, agent.support_of.as_deref()) {
    return Value::Null;
  }

  let reaches = match configured.filter(|one| !one.laya).map(|one| (named(&one.provider), one)) {
    Some((Ok(provider), configured)) => {
      let levels =
        levels(judging, machine, provider, configured.model.as_deref().unwrap_or_default()).await;

      json!({ "thinking": provider.takes_thinking(), "levels": levels })
    }
    _ => Value::Null,
  };

  json!({
    "thinking": agent.thinking,
    "effort": agent.effort.as_ref().filter(|effort| REASONING.holds(effort)),
    "reaches": reaches,
  })
}

// Every gate the agent's turn passes, once, in the first part it stands in and in the turn's order,
// with the agent's own switch, on unless it holds "off"; null for an agent with no turn in a room.
async fn gates_view(db: &DatabaseConnection, agent: &Agent) -> Result<Value, Refused> {
  let Some(gates) = turn_gates_of(agent.conversation.as_ref().map(|settings| settings.shape)) else {
    return Ok(Value::Null);
  };
  let mut views = Vec::with_capacity(gates.len());

  for (part, key) in gates {
    let on = setting(db, &format!("agent_gate:{}:{key}", agent.name)).await?.as_deref() != Some("off");

    views.push(json!({ "part": part, "key": key, "on": on }));
  }

  Ok(json!(views))
}

// The rooms a designated agent holds sessions in, each with the moment its log last took a line there:
// its own room sessions, and for an agent of the agent scope every room its one session has answered.
async fn room_sessions(db: &DatabaseConnection, agent: &Agent) -> Result<Vec<Value>, Refused> {
  let (one_session, agent) = (
    agent.conversation.as_ref().is_some_and(|settings| settings.scope == Scope::Agent),
    agent.name.as_str(),
  );
  let mut rooms: Vec<i64> = Vec::new();

  for row in
    all(db, r#"SELECT "chat_id" FROM "room_sessions" WHERE "agent" = $1"#, vec![agent.into()]).await?
  {
    rooms.push(row.try_get("", "chat_id")?);
  }

  if one_session {
    rooms.extend(one_session_rooms(db, agent).await?);
  }

  rooms.sort_unstable();
  rooms.dedup();

  let mut sessions = Vec::with_capacity(rooms.len());

  for chat_id in rooms {
    let last = one(
      db,
      r#"SELECT MAX("at") AS "at" FROM "agent_log" WHERE "agent" = $1 AND "chat_id" = $2"#,
      vec![agent.into(), chat_id.into()],
    )
    .await?;
    let last_at = last.map(|row| row.try_get::<Option<i64>>("", "at")).transpose()?.flatten();

    sessions.push(json!({ "chatId": chat_id, "lastAt": last_at }));
  }

  Ok(sessions)
}

// The rooms the one session of the agent scope has answered: every room its run log has taken a line in.
async fn one_session_rooms(db: &DatabaseConnection, agent: &str) -> Result<Vec<i64>, Refused> {
  let mut rooms = Vec::new();

  for row in all(
    db,
    r#"SELECT DISTINCT "chat_id" FROM "agent_log" WHERE "agent" = $1 AND "chat_id" IS NOT NULL"#,
    vec![agent.into()],
  )
  .await?
  {
    rooms.push(row.try_get("", "chat_id")?);
  }

  Ok(rooms)
}
