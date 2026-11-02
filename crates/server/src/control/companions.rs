//! The companions' settings as a caller reads and changes them: what each one's system prompt is made of,
//! its persona, whether its turns are given their reference, how its answers are generated and the
//! registration it is pinned to, a save checked whole before anything is written and overwriting what it
//! changes. The prompt its next turn sends is composed by the harness-machine crate
//! (docs/harness-machine.md, 「A companion's system prompt」), its persona read from the prompt folder its
//! row names.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use harness_machine::companion_prompt::{self, Base, Companion, Composition, Roots};
use policy::companions::{COMPANIONS, GENERATION, GenerationSetting};
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::roles::ROLES;
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, QueryResult, TransactionTrait};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use super::fault::{Fault, refused};
use super::models::{entries, latest_uses};
use super::registry::registration_health;
use super::registry::{PROVIDER, registration, registrations};
use super::sessions::{TELL, agent_row, ask_agent_resets, asked_by, carry_out_waiting, last_done, waiting};
use super::{Call, Refused, Route, answer};
use crate::conversational::{Settings as Conversation, Shape, prompt_folder};
use crate::hooks::Hooks;
use crate::now_ms;
use crate::places::Ground;
use crate::places::follow::RoomTitle;
use crate::text::{grouped, length};
use store::{all, one, run};

/// The settings fields a save may change, by the names a save marks as saved.
const FIELDS: [&str; 2] = ["referenceOn", "persona"];

/// Every field a save's body may name: the companion, who saves, its settings, its pin and its thinking.
const BODY_FIELDS: [&str; 7] =
  ["agent", "by", "referenceOn", "persona", "registration", "generation", "thinking"];

/// One companion's settings as its row holds them, every default where it holds none.
#[derive(Clone, Debug)]
pub struct Settings {
  agent: String,
  pub reference_on: bool,
  persona: Option<String>,
  generation: Map<String, Value>,
  // The names of every setting a save or the seed has ever written on the row.
  saved: Vec<String>,
  updated_at: Option<i64>,
  updated_by: Option<String>,
}

// A JSON text a column holds, read as JSON where it is JSON: anything else is none.
fn parsed(text: Option<String>) -> Option<Value> {
  serde_json::from_str(&text?).ok()
}

impl Settings {
  pub async fn of<C: ConnectionTrait>(db: &C, agent: &str) -> Result<Self, Refused> {
    let Some(row) =
      one(db, "SELECT * FROM companion_settings WHERE agent = $1", vec![agent.into()]).await?
    else {
      return Ok(Self {
        agent: agent.to_owned(),
        reference_on: COMPANIONS.reference_on,
        persona: None,
        generation: Map::new(),
        saved: vec![],
        updated_at: None,
        updated_by: None,
      });
    };
    let text = |column: &str| row.try_get::<Option<String>>("", column);
    let object = |read: Option<Value>, keep: fn(&Value) -> bool| -> Map<String, Value> {
      match read {
        Some(Value::Object(read)) => read.into_iter().filter(|(_, value)| keep(value)).collect(),
        _ => Map::new(),
      }
    };

    Ok(Self {
      agent: row.try_get("", "agent")?,
      reference_on: row
        .try_get::<Option<i64>>("", "reference_on")?
        .map_or(COMPANIONS.reference_on, |on| on != 0),
      persona: text("persona")?,
      generation: object(parsed(text("generation")?), |value| value.is_number() || value.is_boolean()),
      saved: parsed(text("saved_settings")?)
        .and_then(|read| serde_json::from_value(read).ok())
        .unwrap_or_default(),
      updated_at: row.try_get("", "updated_at")?,
      updated_by: text("updated_by")?,
    })
  }

  fn shown(&self) -> Value {
    json!({
      "agent": self.agent,
      "referenceOn": self.reference_on,
      "persona": self.persona,
      "generation": self.generation,
      "updatedAt": self.updated_at,
      "updatedBy": self.updated_by,
    })
  }

  // One field as a save reads it.
  fn field(&self, name: &str) -> Value {
    self.shown()[name].clone()
  }

  /// The generation settings the row holds, in the order they were saved.
  pub fn generation(&self) -> &Map<String, Value> {
    &self.generation
  }

  pub fn composition(&self) -> Composition {
    Composition {
      persona: self.persona.clone(),
    }
  }
}

// A setting's value as a save compares it.
fn compared(value: &Value) -> Option<String> {
  (!value.is_null()).then(|| value.to_string())
}

/// The row of an agent of the companion's shape of turn, where it is one.
async fn companion_row<C: ConnectionTrait>(db: &C, agent: &str) -> Result<QueryResult, Fault> {
  let row = agent_row(db, agent).await?;
  let pieces = |row: &QueryResult| {
    Conversation::of(row)
      .is_ok_and(|settings| settings.is_some_and(|settings| settings.shape == Shape::Pieces))
  };

  match row {
    Some(row) if pieces(&row) => Ok(row),
    _ => refused("agent", fill_marks(&CONTROL_WORDS.companions.not_companion, &[("agent", agent)])),
  }
}

// A supervisor's order as its row holds it.
fn order_of(row: &QueryResult) -> Result<crate::order::Order, sea_orm::DbErr> {
  Ok(crate::order::Order::of_row(
    &row.try_get::<String>("", "provider_order")?,
    row.try_get("", "quota_share")?,
  ))
}

/// The registration a companion's order runs on at the registry's provider, where it names one.
fn pinned_registration(row: &QueryResult) -> Result<Option<String>, sea_orm::DbErr> {
  Ok(order_of(row)?.entry(PROVIDER).map(|entry| entry.model.clone()))
}

fn thinking_of(row: &QueryResult) -> Result<Option<bool>, sea_orm::DbErr> {
  Ok(order_of(row)?.first().and_then(crate::order::Entry::thinks))
}

// A number as a sentence writes it: whole numbers grouped by thousands.
fn figure(number: f64) -> String {
  if number.fract() == 0.0 && number.abs() < 1e15 {
    let grouped = grouped(number.abs() as usize);

    if number < 0.0 { format!("-{grouped}") } else { grouped }
  } else {
    number.to_string()
  }
}

/// Why a value is not one the setting takes, or none where it takes it. The window is the
/// registration's, which `max_tokens` stays under.
pub fn generation_refusal(
  setting: &GenerationSetting,
  value: &Value,
  window: Option<i64>,
) -> Option<String> {
  let Some(bounds) = &setting.bounds else {
    return (!value.is_boolean())
      .then(|| fill_marks(&CONTROL_WORDS.generation.switch, &[("name", &setting.name)]));
  };
  let max = bounds.max.or_else(|| window.map(|window| (window - 1) as f64));
  let name = &setting.name;
  let least = if bounds.above_min {
    fill_marks(&CONTROL_WORDS.generation.above, &[("min", &bounds.min.to_string())])
  } else {
    fill_marks(&CONTROL_WORDS.generation.least, &[("min", &figure(bounds.min))])
  };
  let span = match max {
    Some(max) => fill_marks(&CONTROL_WORDS.generation.most, &[("least", &least), ("max", &figure(max))]),
    None => least,
  };
  let Some(number) = value.as_f64().filter(|number| number.is_finite()) else {
    return Some(fill_marks(&CONTROL_WORDS.generation.number, &[("name", name)]));
  };

  if bounds.whole && number.fract() != 0.0 {
    return Some(fill_marks(&CONTROL_WORDS.generation.whole, &[("name", name)]));
  }

  if number < bounds.min
    || (bounds.above_min && number == bounds.min)
    || max.is_some_and(|max| number > max)
  {
    return Some(fill_marks(&CONTROL_WORDS.generation.range, &[("name", name), ("span", &span)]));
  }

  None
}

/// Every companion's generation settings with their bounds, `max_tokens` held under the window.
pub fn generation_settings(window: Option<i64>) -> Vec<Value> {
  GENERATION
    .settings
    .iter()
    .map(|setting| {
      let mut shown = json!({ "name": setting.name });

      if setting.template {
        shown["template"] = true.into();
      }

      let Some(bounds) = &setting.bounds else {
        shown["switch"] = true.into();
        return shown;
      };

      shown["min"] = bounds.min.into();
      shown["max"] = bounds.max.or_else(|| window.map(|window| (window - 1) as f64)).into();

      if bounds.above_min {
        shown["aboveMin"] = true.into();
      }

      if bounds.whole {
        shown["whole"] = true.into();
      }

      shown
    })
    .collect()
}

/// The roots a companion's prompt is composed out of on the front: agency's assets and its data root.
pub fn roots(call: &Call) -> Roots {
  Roots {
    assets: call.asset_root(),
    data: call.data_root(),
  }
}

/// The system prompt a companion's next turn sends on the provider and model named, or an empty one where it
/// cannot be composed: its persona read out of the prompt folder its row names, never by its name.
async fn system<C: ConnectionTrait>(
  db: &C,
  roots: &Roots,
  agent: &str,
  prompts: Option<&str>,
  settings: &Settings,
  (provider, model): (&str, &str),
) -> Result<String, Refused> {
  let chosen = one(
    db,
    "SELECT p.text FROM agent_base_prompts c JOIN base_prompts p ON p.name = c.prompt WHERE c.agent = $1",
    vec![agent.into()],
  )
  .await?
  .map(|row| row.try_get::<String>("", "text"))
  .transpose()?;
  let base = chosen.map(|text| Base {
    text,
    model: model.to_owned(),
  });

  let companion = Companion {
    name: agent,
    prompts,
  };

  let platform = crate::lines::companion::takes_harness(provider);

  Ok(
    companion_prompt::system(roots, companion, &settings.composition(), base.as_ref(), platform)
      .unwrap_or_default(),
  )
}

// The provider and model a supervisor's order names first.
async fn route_of(db: &DatabaseConnection, agent: &str) -> Option<(String, String)> {
  let held = crate::supervisors::named(db, agent).await.ok()??;
  let first = held.order.first()?;

  Some((first.provider.clone(), first.model.clone()))
}

/// One companion as a screen shows it, each room by the title the room's owner answers.
async fn view(db: &DatabaseConnection, hooks: &Hooks, roots: &Roots, agent: &str) -> Result<Value, Fault> {
  let row = companion_row(db, agent).await?;
  let prompts = prompt_folder(db, agent).await?;
  let registration_name = pinned_registration(&row)?;
  let pinned_provider: Option<String> = None;
  let method = "route";
  let route = match method {
    "route" => route_of(db, agent).await,
    _ => None,
  };
  let health = match &registration_name {
    Some(name) => registration_health(db, &roots.data, name).await?,
    None => None,
  };
  let listed = entries(db).await?;
  let serving = health
    .and_then(|health| health.model)
    .and_then(|model| listed.iter().find(|one| one.file == model).map(|one| one.name.clone()));
  let behind: Vec<_> = match &registration_name {
    Some(name) => listed.iter().filter(|one| one.registration.as_ref() == Some(name)).collect(),
    None => vec![],
  };
  let engine = behind.iter().find_map(|one| one.engine.clone());
  let model_use = match &engine {
    Some(engine) => latest_uses(db)
      .await?
      .into_iter()
      .map(|one| one.shown)
      .find(|shown| shown["engine"] == *engine),
    None => None,
  };
  let settings = Settings::of(db, agent).await?;
  let window = match &registration_name {
    Some(name) => registration(db, name).await?.map(|held| held.context_tokens),
    None => None,
  };
  let pinned_model: Option<String> = None;
  let (provider_now, model_now) = match &route {
    Some((provider, model)) => (provider.clone(), model.clone()),
    None => (pinned_provider.clone().unwrap_or_default(), pinned_model.clone().unwrap_or_default()),
  };
  let text = system(db, roots, agent, prompts.as_deref(), &settings, (&provider_now, &model_now)).await?;
  let mut sessions = vec![];

  for session in
    all(db, "SELECT * FROM room_sessions WHERE agent = $1 ORDER BY chat_id", vec![agent.into()]).await?
  {
    let chat_id: i64 = session.try_get("", "chat_id")?;
    let title = hooks.ask::<RoomTitle>(chat_id).await;

    sessions.push(json!({
      "chatId": chat_id,
      "title": title,
      "generation": session.try_get::<i64>("", "generation")?,
      "systemResetAt": session.try_get::<Option<i64>>("", "system_reset_at")?,
      "resetWaiting": waiting(db, agent, Some(chat_id)).await?.is_some(),
      "lastResetAt": last_done(db, agent, Some(chat_id)).await?.and_then(|done| done.done_at),
    }));
  }

  let base_prompt = one(db, "SELECT prompt FROM agent_base_prompts WHERE agent = $1", vec![agent.into()])
    .await?
    .map(|row| row.try_get::<String>("", "prompt"))
    .transpose()?;
  let base_prompts: Vec<Value> = all(db, "SELECT name, label FROM base_prompts ORDER BY name", vec![])
    .await?
    .iter()
    .map(|row| {
      Ok(
        json!({ "name": row.try_get::<String>("", "name")?, "label": row.try_get::<String>("", "label")? }),
      )
    })
    .collect::<Result<_, sea_orm::DbErr>>()?;
  let offered: Vec<Value> = registrations(db)
    .await?
    .into_iter()
    .filter(|one| one.offered())
    .map(|one| json!({ "name": one.name, "label": one.label }))
    .collect();
  let label: Option<String> = row.try_get("", "label")?;

  Ok(json!({
    "agent": agent,
    "label": label.unwrap_or_else(|| agent.to_owned()),
    "icon": ROLES.icon("chat").map(str::to_owned).or(row.try_get::<Option<String>>("", "icon")?),
    "builtin": row.try_get::<i64>("", "builtin")? == 1,
    "method": method,
    "provider": match (&pinned_provider, &route) {
      (Some(provider), _) => Some(provider.as_str()),
      (None, Some((provider, _))) => Some(provider.as_str()),
      (None, None) => None,
    },
    "model": match (&pinned_model, &route) {
      (Some(model), _) => Some(model.clone()),
      (None, Some((_, model))) => Some(model.clone()),
      (None, None) => None,
    },
    "registration": registration_name,
    "registrations": offered,
    "entries": behind.iter().map(|one| json!({
      "name": one.name,
      "label": one.shown["label"],
      "file": one.file,
      "engine": one.engine,
    })).collect::<Vec<_>>(),
    "serving": serving,
    "modelUse": model_use,
    "settings": settings.shown(),
    "generationSettings": generation_settings(window),
    "thinking": thinking_of(&row)?,
    "personaFile": roots.persona(prompts.as_deref(), None).unwrap_or_default(),
    "basePrompt": base_prompt,
    "basePrompts": base_prompts,
    "system": {
      "characters": length(&text),
      "hash": format!("{:x}", Sha256::digest(text.as_bytes())),
      "text": text,
    },
    "sessions": sessions,
  }))
}

/// Every companion, in the agent list's order.
pub async fn companions(
  db: &DatabaseConnection,
  hooks: &Hooks,
  roots: &Roots,
) -> Result<Vec<Value>, Fault> {
  let names = crate::conversational::agents_where(db, |settings| settings.shape == Shape::Pieces).await?;
  let mut shown = vec![];

  for name in names {
    shown.push(view(db, hooks, roots, &name).await?);
  }

  Ok(shown)
}

/// The registration a body names, where it has an address; none otherwise.
async fn offered<C: ConnectionTrait>(db: &C, named: &Value) -> Result<Option<String>, Refused> {
  let Some(name) = named.as_str() else {
    return Ok(None);
  };

  Ok(registration(db, name).await?.filter(|held| held.offered()).map(|held| held.name))
}

/// What a save changes of the settings row, each field it names.
#[derive(Default)]
struct Change {
  fields: Map<String, Value>,
}

impl Change {
  fn set(&mut self, field: &str, value: Value) {
    self.fields.insert(field.to_owned(), value);
  }

  fn get(&self, field: &str) -> Option<&Value> {
    self.fields.get(field)
  }
}

/// The generation settings a save names, laid over the held ones and each checked against its bounds.
fn generation_change(
  body: &Value,
  held: &Map<String, Value>,
  window: Option<i64>,
) -> Result<Map<String, Value>, Fault> {
  let Some(named) = body.as_object() else {
    return refused("generation", CONTROL_WORDS.companions.generation_pairs.as_str());
  };
  let mut next = held.clone();

  for (name, value) in named {
    let Some(setting) = GENERATION.setting(name) else {
      return refused(name, fill_marks(&CONTROL_WORDS.companions.not_generation, &[("name", name)]));
    };

    if value.is_null() {
      next.remove(name);
      continue;
    }

    if let Some(refusal) = generation_refusal(setting, value, window) {
      return refused(name, refusal);
    }

    next.insert(name.clone(), value.clone());
  }

  Ok(next)
}

/// A save checked whole: the change of the settings row, the registration it pins and the thinking it
/// sets. Nothing is written until every part has passed.
struct Checked {
  change: Change,
  /// What the save does to the pin: kept where it names none, cleared by a null, set by a registration.
  pin: Option<Option<String>>,
  thinking: Option<bool>,
}

async fn checked<C: ConnectionTrait>(
  db: &C,
  row: &QueryResult,
  body: &Value,
  agent: &str,
) -> Result<Checked, Fault> {
  let mut change = Change::default();

  if let Some(field) = body
    .as_object()
    .and_then(|named| named.keys().find(|key| !BODY_FIELDS.contains(&key.as_str())))
  {
    return refused(field, fill_marks(&CONTROL_WORDS.companions.not_field, &[("field", field)]));
  }

  if let Some(reference) = body.get("referenceOn") {
    if !reference.is_boolean() {
      return refused("referenceOn", CONTROL_WORDS.companions.reference.as_str());
    }

    change.set("referenceOn", reference.clone());
  }

  let held = Settings::of(db, agent).await?;

  if let Some(persona) = body.get("persona") {
    match persona {
      Value::Null => change.set("persona", Value::Null),
      Value::String(text) if length(text) <= COMPANIONS.persona_most => {
        change.set("persona", if text.trim().is_empty() { Value::Null } else { persona.clone() });
      }
      _ => {
        return refused(
          "persona",
          fill_marks(&CONTROL_WORDS.companions.persona, &[("most", &grouped(COMPANIONS.persona_most))]),
        );
      }
    }
  }

  let mut pinned = pinned_registration(row)?;
  let mut pin: Option<Option<String>> = None;

  if let Some(named) = body.get("registration") {
    if named.is_null() {
      pinned = None;
      pin = Some(None);
    } else {
      let Some(name) = offered(db, named).await? else {
        return refused("registration", CONTROL_WORDS.companions.registration.as_str());
      };

      pinned = Some(name.clone());
      pin = Some(Some(name));
    }
  }

  if let Some(generation) = body.get("generation") {
    let window = match &pinned {
      Some(name) => registration(db, name).await?.map(|held| held.context_tokens),
      None => None,
    };

    change.set("generation", Value::Object(generation_change(generation, &held.generation, window)?));
  }

  let thinking = match body.get("thinking") {
    None => thinking_of(row)?,
    Some(Value::Null) => None,
    Some(Value::Bool(thinking)) => Some(*thinking),
    Some(_) => return refused("thinking", CONTROL_WORDS.companions.thinking.as_str()),
  };

  Ok(Checked {
    change,
    pin,
    thinking,
  })
}

/// A checked save written in one transaction: the pin, the thinking, and the settings, each overwriting the
/// value it held with the names of those it changed marked as saved, and a reset of every room where it
/// changed how answers are generated. Answers whether agency is to carry out the resets.
async fn write(
  db: &DatabaseTransaction,
  row: &QueryResult,
  agent: &str,
  saved: Checked,
  by: &str,
) -> Result<bool, Fault> {
  let at = now_ms() / 1000;
  let mut fields_changed = vec![];
  let mut order = order_of(row)?;
  let held_registration = order.entry(PROVIDER).map(|entry| entry.model.clone());

  // A registration named stands as the registry's entry of the companion's order, at its head; none takes the
  // registry's entry out.
  match &saved.pin {
    Some(Some(registration)) if held_registration.as_ref() != Some(registration) => {
      order.entries.retain(|entry| entry.provider != PROVIDER);
      order.entries.insert(
        0,
        crate::order::Entry {
          provider: PROVIDER.to_owned(),
          model: registration.clone(),
          effort: None,
          thinking: None,
          answer_tokens: None,
        },
      );
    }
    Some(None) if held_registration.is_some() && order.entries.len() > 1 => {
      order.entries.retain(|entry| entry.provider != PROVIDER);
    }
    _ => {}
  }

  if saved.thinking != thinking_of(row)? {
    for entry in &mut order.entries {
      entry.thinking = saved.thinking.map(|on| if on { "on" } else { "off" }.to_owned());
    }

    fields_changed.push("thinking".to_owned());
  }

  run(
    db,
    r#"UPDATE "supervisors" SET "provider_order" = $1 WHERE "name" = $2"#,
    vec![order.entries_text().into(), row.try_get::<String>("", "name")?.into()],
  )
  .await?;

  let held = Settings::of(db, agent).await?;
  let next = |field: &str| saved.change.get(field).cloned().unwrap_or_else(|| held.field(field));
  let changed: Vec<&str> = FIELDS
    .iter()
    .copied()
    .filter(|field| compared(&held.field(field)) != compared(&next(field)))
    .collect();
  let next_generation = match next("generation") {
    Value::Object(generation) => generation,
    _ => Map::new(),
  };
  let mut names: Vec<&String> = held.generation.keys().collect();

  names.extend(next_generation.keys().filter(|name| !held.generation.contains_key(*name)));

  let generation: Vec<String> = names
    .into_iter()
    .filter(|name| held.generation.get(*name) != next_generation.get(*name))
    .cloned()
    .collect();

  if !changed.is_empty() || !generation.is_empty() {
    let mut marked: Vec<String> = held.saved.clone();

    marked.extend(changed.iter().map(|field| (*field).to_owned()));
    marked.extend(generation.iter().cloned());
    marked.sort();
    marked.dedup();
    run(
      db,
      "INSERT INTO companion_settings (agent, reference_on, pieces, own_pieces, persona, generation, saved_settings,
       updated_at, updated_by) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
       ON CONFLICT (agent) DO UPDATE SET reference_on = excluded.reference_on, pieces = excluded.pieces,
       own_pieces = excluded.own_pieces, persona = excluded.persona, generation = excluded.generation,
       saved_settings = excluded.saved_settings, updated_at = excluded.updated_at, updated_by = excluded.updated_by",
      vec![
        agent.into(),
        i64::from(next("referenceOn") != Value::Bool(false)).into(),
        Option::<String>::None.into(),
        "{}".to_owned().into(),
        next("persona").as_str().map(str::to_owned).into(),
        Value::Object(next_generation.clone()).to_string().into(),
        json!(marked).to_string().into(),
        at.into(),
        by.into(),
      ],
    )
    .await?;

    fields_changed.extend(generation);
  }

  // A change to how its answers are generated starts every room afresh, so no conversation goes on
  // under settings it was not begun with.
  if fields_changed.is_empty() {
    return Ok(false);
  }

  Ok(ask_agent_resets(db, agent, by, &CONTROL_WORDS.companions.reset_reason).await?)
}

/// The roots a companion's settings are read under in a process on agency's ground: the assets of the
/// code tree it runs, and its data root.
pub fn process_roots(ground: &Ground) -> Roots {
  Roots {
    assets: ground.code_root(),
    data: ground.roots.data.clone(),
  }
}

// What stops a verb's read or save, as the sentence it prints.
fn said(fault: Fault) -> String {
  match fault {
    Fault::Field(_, error) => error,
    Fault::Failed(failed) => failed.error,
  }
}

/// One companion as its screen shows it, read by a verb outside a route; the sentence of what stopped it
/// otherwise.
pub async fn shown(
  db: &DatabaseConnection,
  hooks: &Hooks,
  roots: &Roots,
  agent: &str,
) -> Result<Value, String> {
  view(db, hooks, roots, agent).await.map_err(said)
}

/// A save of one companion's settings, read as a route's body is read and written whole or refused, by a
/// verb outside a route: whether agency is to carry out the resets it asked, or the sentence of what
/// stopped it.
pub async fn saved(db: &DatabaseConnection, body: &Value, by: &str) -> Result<bool, String> {
  let agent = body.get("agent").and_then(Value::as_str).unwrap_or_default().to_owned();
  let written = db.begin().await.map_err(|why| why.to_string())?;
  let made = async {
    let row = companion_row(&written, &agent).await?;
    let checked = checked(&written, &row, body, &agent).await?;

    write(&written, &row, &agent, checked, by).await
  }
  .await
  .map_err(said)?;

  written.commit().await.map_err(|why| why.to_string())?;

  Ok(made)
}

/// The word a save that asked resets is told to the server by.
pub const RESETS_TOLD: &str = TELL;

/// Every companion as its screen shows it.
pub struct Companions;

impl Route for Companions {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/companions";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    match companions(call.store(), &call.ground().hooks, &roots(&call)).await {
      Ok(shown) => Ok(answer(StatusCode::OK, json!({ "items": shown, "more": false }))),
      Err(fault) => fault.answer(),
    }
  }
}

crate::moved!(Companions);

/// A save of one companion's settings, answered with the companion as it then stands.
pub struct CompanionChange;

impl Route for CompanionChange {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/companions/{companionName}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let mut body = call.body().await?;

    if !body.is_object() && !body.is_array() {
      return Err(Refused::new(StatusCode::BAD_REQUEST, CONTROL_WORDS.request.action.as_str()));
    }

    let roots = roots(&call);
    let db = call.store();
    if body.get("agent").is_none() {
      body["agent"] = json!(call.param("companionName").unwrap_or_default());
    }
    let agent = body.get("agent").and_then(Value::as_str).unwrap_or_default().to_owned();
    let by = asked_by(&body);
    let written = db.begin().await?;
    let made = async {
      let row = companion_row(&written, &agent).await?;
      let saved = checked(&written, &row, &body, &agent).await?;

      write(&written, &row, &agent, saved, &by).await
    }
    .await;
    let tell = match made {
      Ok(tell) => tell,
      Err(fault) => return fault.answer(),
    };

    written.commit().await?;

    // A pin moved under a standing program lets it go, so the next turn stands the pin's own model and
    // address (docs/companions.md, 「The program on an ended session」).
    crate::places::standby::release_stale_companion(call.ground(), &agent).await;

    if tell {
      carry_out_waiting(&call).await;
    }

    match view(db, &call.ground().hooks, &roots, &agent).await {
      Ok(shown) => Ok(answer(StatusCode::OK, shown)),
      Err(fault) => fault.answer(),
    }
  }
}

crate::moved!(CompanionChange);
