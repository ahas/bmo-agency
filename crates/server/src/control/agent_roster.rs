//! Every agent registered and its registration ended through one pair of routes (docs/api.md, 「agents」):
//! a row of any kind written once every field has passed, owned by the login account the caller names, and
//! the brain of one that converses asked of the brain as it is registered; a registration ended with
//! everything of the agent's own but its brain, which goes only where the caller says so.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bmo_agency_client::BrainBirth;
use policy::companions::COMPANIONS;
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::reasoning::REASONING;
use policy::roles::ROLES;
use regex::Regex;
use sea_orm::{DatabaseTransaction, TransactionTrait, Value as Bound};
use serde_json::{Value, json};
use std::sync::LazyLock;
use store::run;

use super::agents::view_named;
use super::companion_reset::{reset_whole, rooms_named, to_its_end};
use super::companion_roster::{Left, given, pin_checked, remove, text};
use super::fault::{Fault, refused};
use super::sessions::{agent_row, asked_by};
use super::{Call, Refused, Route, answer};
use crate::conversational::Settings as Conversation;
use crate::now_ms;
use crate::peers::brain::{Brain, Made};
use crate::places::Ground;
use crate::places::locks::{agent_lock_path, held, pane_lock_path};

static NAME: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(&COMPANIONS.name_pattern).expect("the agent name pattern reads"));

// The kind that converses in rooms, whose agents are written as a companion added on the console is.
const CHAT: &str = "chat";

// The kind that takes jobs, whose agents stand in a checkout.
const WORKER: &str = "worker";

/// An agent asked for, every field as the body gave it.
struct Asked {
  name: String,
  label: String,
  kind: String,
  role: String,
  owner: Option<i64>,
  icon: String,
  thinking: Option<bool>,
  effort: String,
  provider: String,
  model: String,
  worker: String,
  cwd: String,
  node: String,
}

impl Asked {
  /// The ask as the body gives it, refused where the owner or the thinking is not of its type.
  fn of(body: &Value) -> Result<Self, Fault> {
    let words = &CONTROL_WORDS.agents;
    let owner = match body.get("ownerAccount") {
      None | Some(Value::Null) => None,
      Some(owner) => match owner.as_i64() {
        Some(owner) => Some(owner),
        None => return refused("ownerAccount", words.owner.as_str()),
      },
    };
    let thinking = match body.get("thinking") {
      None | Some(Value::Null) => None,
      Some(Value::Bool(thinking)) => Some(*thinking),
      Some(_) => return refused("thinking", words.thinking.as_str()),
    };
    let kind = text(body, "kind");
    let role = Some(text(body, "role"))
      .filter(|role| !role.is_empty())
      .or_else(|| ROLES.kinds.get(&kind).cloned())
      .unwrap_or_default();

    Ok(Self {
      name: text(body, "name"),
      label: text(body, "label"),
      kind,
      role,
      owner,
      icon: text(body, "icon"),
      thinking,
      effort: text(body, "effort"),
      provider: text(body, "provider"),
      model: text(body, "model"),
      worker: text(body, "worker"),
      cwd: text(body, "cwd"),
      node: text(body, "nodeName"),
    })
  }

  /// Whether the agent is given its own brain: one of the kind that converses, or holding that kind's role.
  fn given_a_brain(&self) -> bool {
    self.kind == CHAT || ROLES.kinds.get(CHAT) == Some(&self.role)
  }
}

/// Every field of the ask checked, in the order the boundary states them; nothing is written until all have
/// passed. A name another agent holds is a conflict rather than a field to fix.
async fn checked(written: &DatabaseTransaction, ground: &Ground, asked: &Asked) -> Result<(), Fault> {
  let roster = &CONTROL_WORDS.companion_roster;
  let words = &CONTROL_WORDS.agents;

  if !NAME.is_match(&asked.name) {
    return refused("name", roster.name.as_str());
  }

  if agent_row(written, &asked.name).await?.is_some() {
    let taken = fill_marks(&roster.name_taken, &[("name", &asked.name)]);

    return Err(Fault::Failed(Refused::new(StatusCode::CONFLICT, taken).with_code("name-taken")));
  }

  if asked.label.is_empty() {
    return refused("label", roster.label.as_str());
  }

  if !ROLES.kinds.contains_key(&asked.kind) {
    let kinds = ROLES.kinds.keys().map(String::as_str).collect::<Vec<_>>().join(", ");

    return refused("kind", fill_marks(&words.kind, &[("kinds", &kinds)]));
  }

  if !ROLES.holds(&asked.role) {
    return refused("role", fill_marks(&words.role, &[("roles", &ROLES.labels().join(", "))]));
  }

  if !asked.effort.is_empty() && !REASONING.holds(&asked.effort) {
    return refused("effort", fill_marks(&words.effort, &[("levels", &REASONING.levels.join(", "))]));
  }

  if asked.kind == CHAT {
    pin_checked(written, ground, &asked.provider, &asked.model, &asked.worker).await?;
  }

  Ok(())
}

// A text field written where it was given, none where it was left empty.
fn bound(field: &str) -> Bound {
  given(field).map(str::to_owned).into()
}

/// A checked agent's row: its kind's own columns, its owner, and nothing seeded or answering every room.
/// One that converses is written as a companion added on the console is; a worker stands in the checkout
/// it names, the code tree's root where it names none.
async fn write(written: &DatabaseTransaction, ground: &Ground, asked: &Asked) -> Result<(), Fault> {
  let chat = asked.kind == CHAT;
  let conversation = chat.then(Conversation::companion_added);
  let icon = match given(&asked.icon) {
    None if chat => Some(COMPANIONS.added_icon.clone()),
    icon => icon.map(str::to_owned),
  };
  let cwd = match given(&asked.cwd) {
    None if asked.kind == WORKER => Some(ground.source.to_string_lossy().into_owned()),
    cwd => cwd.map(str::to_owned),
  };

  run(
    written,
    r#"INSERT INTO "agents" ("name", "kind", "label", "role", "owner_account", "icon", "thinking", "effort",
       "session_scope", "turn_shape", "default_answerer", "pinned_provider", "pinned_model", "item_worker",
       "brain_folder", "cwd", "node", "builtin", "last_run_at")
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 0, $11, $12, $13, $14, $15, $16, 0, $17)"#,
    vec![
      asked.name.clone().into(),
      asked.kind.clone().into(),
      asked.label.clone().into(),
      asked.role.clone().into(),
      asked.owner.into(),
      icon.into(),
      asked.thinking.map(i64::from).into(),
      bound(&asked.effort),
      conversation.as_ref().map(|settings| settings.scope.word().to_owned()).into(),
      conversation.as_ref().map(|settings| settings.shape.word().to_owned()).into(),
      if chat { bound(&asked.provider) } else { Option::<String>::None.into() },
      if chat { bound(&asked.model) } else { Option::<String>::None.into() },
      if chat { bound(&asked.worker) } else { Option::<String>::None.into() },
      chat.then(|| format!("agents/{}", asked.name)).into(),
      cwd.into(),
      if asked.kind == WORKER { bound(&asked.node) } else { Option::<String>::None.into() },
      chat.then(|| now_ms() / 1000).into(),
    ],
  )
  .await?;

  Ok(())
}

/// A refusal of one field answered 400 with the field and the code beside the error, or the failure handed on.
fn refusal(fault: Fault) -> Result<Response, Refused> {
  match fault {
    Fault::Field(field, error) => Ok(answer(
      StatusCode::BAD_REQUEST,
      json!({ "code": "bad-request", "error": error, "field": field }),
    )),
    Fault::Failed(failed) => Err(failed),
  }
}

/// An agent of any kind registered.
pub struct AgentCreate;

impl Route for AgentCreate {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/agents";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;

    if !body.is_object() {
      return Err(Refused::new(StatusCode::BAD_REQUEST, CONTROL_WORDS.request.action.as_str()));
    }

    let asked = match Asked::of(&body) {
      Ok(asked) => asked,
      Err(fault) => return refusal(fault),
    };
    let ground = call.shared_ground();
    let written = ground.db.begin().await?;
    let made = async {
      checked(&written, &ground, &asked).await?;
      write(&written, &ground, &asked).await
    }
    .await;

    if let Err(fault) = made {
      return refusal(fault);
    }

    written.commit().await?;

    let brain = if asked.given_a_brain() {
      born(ground.peers.brain.as_ref(), &asked).await
    } else {
      BrainBirth::None
    };
    let agent = view_named(&ground.db, &ground, call.machine(), &asked.name).await?;

    eprintln!("agency: the agent {} was registered", asked.name);

    Ok(answer(StatusCode::CREATED, json!({ "agent": agent, "brain": brain })))
  }
}

crate::moved!(AgentCreate);

/// The registered agent's own brain asked of the brain, under its name, with its label, filed under its
/// owner. The agent stands whatever the brain answers, so what it could not do is logged and refuses nothing.
async fn born(brain: &dyn Brain, asked: &Asked) -> BrainBirth {
  match brain.make(&asked.name, &asked.label, asked.owner).await {
    Ok(Made::Made) => {
      eprintln!("agency: the agent {} was given its brain", asked.name);

      BrainBirth::Made
    }
    Ok(Made::Kept) => BrainBirth::Kept,
    Ok(Made::Undeployed) => BrainBirth::None,
    Err(fault) => {
      eprintln!("agency: the agent {} was registered without its brain: {fault}", asked.name);

      BrainBirth::Failed
    }
  }
}

// Whether the query switches a flag on.
fn switched(call: &Call, name: &str) -> bool {
  call.query(name).is_some_and(|value| value == "true")
}

/// Whether a turn holds the agent now: its agent lock, or a worker's pane lock.
fn running(call: &Call, agent: &str, kind: &str) -> bool {
  let data_root = call.data_root();

  held(&agent_lock_path(&data_root, agent)) || (kind == WORKER && held(&pane_lock_path(&data_root, agent)))
}

/// An agent's registration ended, with everything of its own but its brain where the caller keeps it.
pub struct AgentDelete;

impl Route for AgentDelete {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/agents/{agentName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let words = &CONTROL_WORDS.agents;
    let asked = call.param("agentName").unwrap_or_default().to_owned();
    let force = switched(&call, "force");
    let forget_brain = switched(&call, "forgetBrain");
    let rooms = rooms_named(call.query("rooms").map(Value::String).as_ref());
    let Some(row) = agent_row(call.store(), &asked).await? else {
      return Err(Refused::new(StatusCode::NOT_FOUND, fill_marks(&words.not_found, &[("agent", &asked)])));
    };
    let agent: String = row.try_get("", "name")?;
    let kind: String = row.try_get("", "kind")?;
    let label = row.try_get::<Option<String>>("", "label")?.unwrap_or_else(|| agent.clone());
    let conversation = Conversation::of(&row).map_err(Refused::from)?;

    if row.try_get::<i64>("", "builtin")? == 1
      || conversation.as_ref().is_some_and(|settings| settings.default_answerer)
    {
      let seeded = fill_marks(&words.seeded, &[("label", &label)]);

      return Err(Refused::new(StatusCode::CONFLICT, seeded).with_code("seeded"));
    }

    if !force && running(&call, &agent, &kind) {
      let running = fill_marks(&words.running, &[("label", &label)]);

      return Err(Refused::new(StatusCode::CONFLICT, running).with_code("running"));
    }

    // A companion's conversation is reset first, so the rooms it answered keep their record and only its own
    // rows go; the reset cuts off a turn still holding it.
    let reset = conversation
      .as_ref()
      .is_some_and(|settings| settings.companion())
      .then(|| reset_whole(&call, &agent, rooms, &asked_by(&Value::Null), forget_brain));
    let left = Left::of(&call, &agent);

    to_its_end(async move {
      if let Some(reset) = reset {
        reset.await?;
      }

      let written = left.db.begin().await?;

      if let Err(fault) = remove(&written, &agent).await {
        return refusal(fault);
      }

      written.commit().await?;
      left.taken(&agent, forget_brain).await;
      eprintln!("agency: the registration of the agent {agent} was ended");

      Ok(StatusCode::NO_CONTENT.into_response())
    })
    .await
  }
}

crate::moved!(AgentDelete);
