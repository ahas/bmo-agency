//! An agent's own gate switches, its own thinking and effort, and its role (docs/model-tiers.md, 「An
//! agent's own thinking and effort」; docs/agent-roles.md, 「Changing a role」). Agency keeps an agent's
//! switches; whether a turn heeds a gate's is its consumer's. Which gates a turn passes, which agents run
//! as gates and the scale an effort is named on are the policy's (docs/policy.md).

use axum::http::{Method, StatusCode};
use axum::response::Response;
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::reasoning::REASONING;
use policy::roles::ROLES;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer, canonical_agent, set_setting, setting};
use crate::control::registry;
use crate::conversational::{Settings, Shape};
use store::{one, run};

/// An agent's row, as far as its switches read it.
struct Agent {
  name: String,
  kind: String,
  label: Option<String>,
  support_of: Option<String>,
  /// The gates its turn passes, once each with the first part it stands in.
  gates: Option<Vec<(&'static str, String)>>,
  /// Whether its name is a gate kind the gate declared.
  gate_kind: bool,
}

impl Agent {
  /// The agent a name stands for, as the store finds it.
  async fn find(db: &DatabaseConnection, name: &str) -> Result<Option<Self>, Refused> {
    let row = one(
      db,
      r#"SELECT "name", "kind", "session_scope", "turn_shape", "default_answerer", "label", "support_of" FROM "agents"
         WHERE "name" = $1"#,
      vec![canonical_agent(db, name).await?.into()],
    )
    .await?;

    let Some(row) = row else { return Ok(None) };
    let name: String = row.try_get("", "name")?;
    let shape = Settings::of(&row).map_err(Refused::from)?.map(|settings| settings.shape);
    let gates = turn_gates_of(db, shape).await?;
    let gate_kind = crate::gate_kinds::holds(db, &name).await?;

    Ok(Some(Self {
      name,
      kind: row.try_get("", "kind")?,
      label: row.try_get("", "label")?,
      support_of: row.try_get("", "support_of")?,
      gates,
      gate_kind,
    }))
  }

  /// The agent's name as a person reads it.
  fn called<'a>(&'a self, asked: &'a str) -> &'a str {
    self.label.as_deref().unwrap_or(asked)
  }

  /// The gates the agent's turn passes, once each.
  fn turn_gates(&self) -> Option<Vec<String>> {
    self.gates.as_ref().map(|gates| gates.iter().map(|(_, key)| key.clone()).collect())
  }

  fn owns_reasoning(&self) -> bool {
    !self.gate_kind && self.support_of.is_none()
  }

  /// Why it has no gates to switch.
  fn no_gates(&self) -> String {
    fill_marks(
      &CONTROL_WORDS.agents.no_gates,
      &[("agent", self.label.as_deref().unwrap_or(&self.name))],
    )
  }
}

/// The gates a conversational agent's turn passes, as the shape of its turns declares them, once each with
/// the first part it stands in; none for an agent that answers in no room.
pub async fn turn_gates_of(
  db: &DatabaseConnection,
  shape: Option<Shape>,
) -> Result<Option<Vec<(&'static str, String)>>, Refused> {
  if shape.is_none() {
    return Ok(None);
  }

  let turn = crate::gate_kinds::turn(&crate::gate_kinds::declared(db).await?);

  Ok(Some(turn.gates().into_iter().map(|(part, key)| (part, key.to_owned())).collect()))
}

/// Whether an agent's own choice reaches its runs: not a gate's, a judgement asked under its tier, and
/// not a support worker's, which runs on its base's.
pub async fn owns_reasoning(
  db: &DatabaseConnection,
  name: &str,
  support_of: Option<&str>,
) -> Result<bool, Refused> {
  Ok(!crate::gate_kinds::holds(db, name).await? && support_of.is_none())
}

/// What an agent there is no row of is refused with.
fn not_found(name: &str) -> String {
  fill_marks(&CONTROL_WORDS.agents.not_found, &[("agent", name)])
}

/// The name a body asks about: a string, trimmed, or nothing.
fn asked_name(body: &Value) -> String {
  body.get("agent").and_then(Value::as_str).map(str::trim).unwrap_or_default().to_owned()
}

/// One of an agent's gates switched on or off. The next turn of the agent reads it; nothing stands
/// again for it.
pub struct AgentGate;

impl Route for AgentGate {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/agent-gate";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let name = asked_name(&body);
    let gate = body.get("gate").cloned().unwrap_or(Value::Null);
    let on = body.get("on").cloned().unwrap_or(Value::Null);

    if let Err(fault) = switch_gate(call.store(), &name, &gate, &on).await? {
      return Err(Refused::new(StatusCode::BAD_REQUEST, fault));
    }

    Ok(answer(StatusCode::OK, json!({ "ok": true, "agent": name, "gate": gate, "on": on })))
  }
}

/// One of an agent's gates switched on or off, or the sentence refusing it: an agent there is no row of,
/// one that answers in no room, a gate its turn does not pass, and a switch that is neither.
pub async fn switch_gate(
  db: &DatabaseConnection,
  name: &str,
  gate: &Value,
  on: &Value,
) -> Result<Result<(), String>, Refused> {
  let Some(agent) = Agent::find(db, name).await? else {
    return Ok(Err(not_found(name)));
  };
  let Some(gates) = agent.turn_gates() else {
    return Ok(Err(agent.no_gates()));
  };
  let Some(gate) = gate.as_str().filter(|gate| gates.iter().any(|one| one == gate)) else {
    return Ok(Err(fill_marks(
      &CONTROL_WORDS.agents.gates_of,
      &[("agent", agent.called(name)), ("gates", &gates.join(", "))],
    )));
  };
  let Some(on) = on.as_bool() else {
    return Ok(Err(CONTROL_WORDS.agents.gate_switch.clone()));
  };

  set_setting(db, &format!("agent_gate:{}:{gate}", agent.name), if on { "on" } else { "off" }).await?;

  Ok(Ok(()))
}

/// One gate an agent's turn passes: the part it first stands in, its key, and the agent's own switch.
pub struct GateSwitch {
  pub part: &'static str,
  pub key: String,
  pub on: bool,
}

/// Every gate an agent's turn passes, once each in the turn's order, with the agent's own switch, on unless
/// it holds off; or the sentence saying why it has none.
pub async fn agent_gates(
  db: &DatabaseConnection,
  name: &str,
) -> Result<Result<Vec<GateSwitch>, String>, Refused> {
  let Some(agent) = Agent::find(db, name).await? else {
    return Ok(Err(not_found(name)));
  };
  let Some(gates) = agent.gates.clone() else {
    return Ok(Err(agent.no_gates()));
  };
  let mut switches = Vec::with_capacity(gates.len());

  for (part, key) in gates {
    let on = setting(db, &format!("agent_gate:{}:{key}", agent.name)).await?.as_deref() != Some("off");

    switches.push(GateSwitch { part, key, on });
  }

  Ok(Ok(switches))
}

/// An agent's own thinking and effort set, each null to leave it to the tier and the line. A worker's
/// standing program is stood again on it, as on a new pin.
pub struct AgentReasoning;

impl Route for AgentReasoning {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/agent-reasoning";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let name = asked_name(&body);
    let thinking = body.get("thinking").cloned().unwrap_or(Value::Null);
    let effort = body.get("effort").cloned().unwrap_or(Value::Null);
    let kind = match set_reasoning(call.store(), &name, &thinking, &effort).await? {
      Ok(kind) => kind,
      Err(fault) => return Err(Refused::new(StatusCode::BAD_REQUEST, fault)),
    };

    // A worker's standing program is stood again on it, as on a new pin, not waited on.
    if kind == "worker"
      && let Some(places) = call.places().cloned()
    {
      tokio::spawn(async move { crate::places::standby::restand_all(&places.ground, None).await });
    }

    Ok(answer(
      StatusCode::OK,
      json!({ "ok": true, "agent": name, "thinking": thinking, "effort": effort }),
    ))
  }
}

/// An agent's own thinking and effort written, each null to leave it to the tier and the line: the kind
/// of the agent it was set on, or the sentence refusing it.
pub async fn set_reasoning(
  db: &DatabaseConnection,
  name: &str,
  thinking: &Value,
  effort: &Value,
) -> Result<Result<String, String>, Refused> {
  let Some(agent) = Agent::find(db, name).await? else {
    return Ok(Err(not_found(name)));
  };

  if !agent.owns_reasoning() {
    let follows = if agent.support_of.is_none() {
      &CONTROL_WORDS.agents.follows_tier
    } else {
      &CONTROL_WORDS.agents.follows_base
    };

    return Ok(Err(fill_marks(follows, &[("agent", agent.called(name))])));
  }

  if !thinking.is_null() && !thinking.is_boolean() {
    return Ok(Err(CONTROL_WORDS.agents.thinking.clone()));
  }

  if !effort.is_null() && !effort.as_str().is_some_and(|level| REASONING.holds(level)) {
    return Ok(Err(fill_marks(
      &CONTROL_WORDS.agents.effort,
      &[("levels", &REASONING.levels.join(", "))],
    )));
  }

  run(
    db,
    r#"UPDATE "agents" SET "thinking" = $1, "effort" = $2 WHERE "name" = $3"#,
    vec![
      thinking.as_bool().map(i64::from).into(),
      effort.as_str().map(str::to_owned).into(),
      name.into(),
    ],
  )
  .await?;

  Ok(Ok(agent.kind))
}

/// The role an agent holds from now on. Its next record reads it; nothing running is stopped for it.
pub struct AgentRole;

impl Route for AgentRole {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/agent-role";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let db = call.store();
    let name = asked_name(&body);
    let refuse = |fault: String| Err(Refused::new(StatusCode::BAD_REQUEST, fault));

    let Some(agent) = Agent::find(db, &name).await? else {
      return refuse(not_found(&name));
    };
    let Some(role) = body.get("role").and_then(Value::as_str).filter(|role| ROLES.holds(role)) else {
      return refuse(fill_marks(&CONTROL_WORDS.agents.role, &[("roles", &ROLES.labels().join(", "))]));
    };

    run(
      db,
      r#"UPDATE "agents" SET "role" = $1 WHERE "name" = $2"#,
      vec![role.to_owned().into(), agent.name.clone().into()],
    )
    .await?;

    Ok(answer(StatusCode::OK, json!({ "ok": true, "agent": name, "role": role })))
  }
}

/// An agent's gates, reasoning, role or voice changed, answered as the agent then stands.
pub struct ChangeAgent;

impl Route for ChangeAgent {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/agents/{agentName}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let name = call.param("agentName").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let db = call.store();

    if let Some(gates) = body.get("gates").and_then(Value::as_object) {
      for (gate, on) in gates {
        if let Err(fault) = switch_gate(db, &name, &json!(gate), on).await? {
          return Err(Refused::new(StatusCode::BAD_REQUEST, fault));
        }
      }
    }

    if let Some(reasoning) = body.get("reasoning").filter(|reasoning| reasoning.is_object()) {
      let thinking = reasoning.get("thinking").cloned().unwrap_or(Value::Null);
      let effort = reasoning.get("effort").cloned().unwrap_or(Value::Null);

      if let Err(fault) = set_reasoning(db, &name, &thinking, &effort).await? {
        return Err(Refused::new(StatusCode::BAD_REQUEST, fault));
      }
    }

    if let Some(role) = body.get("role").and_then(Value::as_str) {
      if !ROLES.holds(role) {
        return Err(Refused::new(
          StatusCode::BAD_REQUEST,
          fill_marks(&CONTROL_WORDS.agents.role, &[("roles", &ROLES.labels().join(", "))]),
        ));
      }
      run(
        db,
        r#"UPDATE "agents" SET "role" = $1 WHERE "name" = $2"#,
        vec![role.to_owned().into(), name.clone().into()],
      )
      .await?;
    }

    if let Some(voice) = body.get("voice") {
      run(
        db,
        r#"UPDATE "agents" SET "voice" = $1 WHERE "name" = $2"#,
        vec![voice.as_str().map(str::to_owned).into(), name.clone().into()],
      )
      .await?;
    }

    if let Some(model) = body.get("model").filter(|model| model.is_object())
      && let Err(fault) = set_model(db, &name, model).await?
    {
      return Err(Refused::new(StatusCode::BAD_REQUEST, fault));
    }

    let db = call.store().clone();
    let ground = call.shared_ground();
    let machine = call.machine().clone();

    Ok(answer(StatusCode::OK, super::agents::view_named(&db, &ground, &machine, &name).await?))
  }
}

crate::moved!(ChangeAgent);

/// An agent's model method (docs/model-tiers.md, 「A supervisor's own model」): `route` answers on the
/// supervisor key's road, its pin cleared, and `fixed` keeps the agent on a registered model. Every chat
/// agent is a supervisor and selects its model either way; any other agent takes no model of its own.
pub async fn set_model(
  db: &DatabaseConnection,
  name: &str,
  asked: &Value,
) -> Result<Result<(), String>, Refused> {
  let Some(agent) = Agent::find(db, name).await? else {
    return Ok(Err(not_found(name)));
  };

  match asked.get("method").and_then(Value::as_str) {
    Some("route") => {
      if agent.kind != "chat" {
        return Ok(Err(fill_marks(
          &CONTROL_WORDS.agents.model_route_only,
          &[("agent", agent.called(name))],
        )));
      }

      run(
        db,
        r#"UPDATE "agents" SET "pinned_provider" = NULL, "pinned_model" = NULL WHERE "name" = $1"#,
        vec![agent.name.clone().into()],
      )
      .await?;

      Ok(Ok(()))
    }
    Some("fixed") => {
      let Some(registration) = asked
        .get("registration")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
      else {
        return Ok(Err(CONTROL_WORDS.agents.model_registration_missing.clone()));
      };

      if registry::registration(db, registration).await?.is_none() {
        return Ok(Err(fill_marks(&CONTROL_WORDS.agents.model_registration, &[("name", registration)])));
      }

      run(
        db,
        r#"UPDATE "agents" SET "pinned_provider" = $1, "pinned_model" = $2 WHERE "name" = $3"#,
        vec![registry::PROVIDER.into(), registration.to_owned().into(), agent.name.clone().into()],
      )
      .await?;

      Ok(Ok(()))
    }
    _ => Ok(Err(CONTROL_WORDS.agents.model_method.clone())),
  }
}
