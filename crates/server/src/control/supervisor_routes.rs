//! The supervisors' routes (docs/api.md, 「supervisors」; docs/supervisors.md): every supervisor read, one
//! registered with its own order and given its brain, changed — its order, warming, threshold, repository,
//! gates, sampling and voice — and its registration ended with everything of its own but its brain where the
//! caller keeps it.

use std::collections::BTreeMap;

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bmo_agency_client::BrainBirth;
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::roles::ROLES;
use policy::supervisors::SUPERVISORS;
use regex::Regex;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use std::sync::LazyLock;
use store::{one, run};

use super::agent_sampling;
use super::claude_accounts::Judging;
use super::sessions::asked_by;
use super::supervisor_removal::{Left, remove};
use super::supervisor_reset::{reset_whole, rooms_named, to_its_end};
use super::{Call, Refused, Route, answer, set_setting, setting};
use crate::conversational::{Scope, Settings as Conversation};
use crate::now_ms;
use crate::order::{self, Order};
use crate::peers::brain::{Brain, Made};
use crate::places::Ground;
use crate::places::locks::{agent_lock_path, held};
use crate::queue::settings;
use crate::repositories::Pin;
use crate::road::{self, Ask, Row};
use crate::supervisors::{self, Supervisor};

static NAME: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(&SUPERVISORS.name_pattern).expect("the supervisor name pattern reads"));

// The line every supervisor's turns go out on.
const CHAT_LINE: &str = "chat";

fn bad(field: &str, error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error).with_field(field)
}

fn not_found(name: &str) -> Refused {
  Refused::new(StatusCode::NOT_FOUND, format!("There is no supervisor {name}."))
}

// The session scope a body names: none where it names none, so the caller's own default stands, or the
// scope its word names, refused by the field where the word is no scope a supervisor carries.
fn session_scope_of(body: &Value) -> Result<Option<Scope>, Refused> {
  match body.get("sessionScope") {
    None => Ok(None),
    Some(Value::String(word)) => Scope::named(word)
      .map(Some)
      .ok_or_else(|| bad("sessionScope", CONTROL_WORDS.supervisor_roster.scope.as_str())),
    Some(_) => Err(bad("sessionScope", CONTROL_WORDS.supervisor_roster.scope.as_str())),
  }
}

/// The gates a supervisor's turn passes, once each with the first part it stands in, with its own switch, on
/// unless it holds off.
async fn gates_view(db: &DatabaseConnection, name: &str) -> Result<Value, Refused> {
  let turn = crate::gate_kinds::turn(&crate::gate_kinds::declared(db).await?);
  let mut views = vec![];

  for (part, key) in turn.gates() {
    let on = setting(db, &format!("agent_gate:{name}:{key}")).await?.as_deref() != Some("off");

    views.push(json!({ "part": part, "key": key, "on": on }));
  }

  Ok(json!(views))
}

/// One supervisor as the routes answer it: every field of its row, its gates, sampling and voice, and what it
/// stands on now.
pub async fn view(ground: &Ground, supervisor: &Supervisor) -> Result<Value, Refused> {
  let db = &ground.db;
  let resolution =
    road::resolve(ground, &Ask::of(Row::Supervisor(&supervisor.name), CHAT_LINE, &supervisor.order))
      .await
      .map_err(Refused::failed)?;
  let waiting_for = resolution.road().is_none().then(|| resolution.sentence());
  let sampling = agent_sampling::view(db, &supervisor.name).await.map(|view| view["values"].clone()).ok();

  Ok(json!({
    "name": supervisor.name,
    "label": supervisor.label,
    "ownerAccount": supervisor.owner_account,
    "aliases": supervisor.aliases,
    "icon": ROLES.icon("supervisor"),
    "sessionScope": supervisor.session_scope,
    "turnShape": supervisor.turn_shape,
    "brainFolder": supervisor.brain_folder,
    "repository": supervisor.repository,
    "order": supervisor.order.entries,
    "quotaShare": supervisor.order.quota_share,
    "active": supervisor.order.forced.as_ref().map(Pin::view),
    "warming": supervisor.warming,
    "foldRatio": supervisor.fold_ratio,
    "cardRank": supervisor.card_rank,
    "gates": gates_view(db, &supervisor.name).await?,
    "voice": supervisor.voice,
    "sampling": sampling,
    "provider": supervisor.last_mode,
    "model": supervisor.last_model,
    "effort": supervisor.last_effort,
    "waitingFor": waiting_for,
    "seeded": supervisor.builtin,
    "resetPoint": crate::supervisor_turn::overtaken::point(db, &supervisor.name).await,
  }))
}

/// `GET /supervisors`: every supervisor.
pub struct SupervisorList;

impl Route for SupervisorList {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/supervisors";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let mut items = vec![];

    for supervisor in supervisors::every(call.store()).await? {
      items.push(view(call.ground(), &supervisor).await?);
    }

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": false })))
  }
}

crate::moved!(SupervisorList);

/// `GET /supervisors/{supervisorName}`: one supervisor.
pub struct SupervisorRead;

impl Route for SupervisorRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/supervisors/{supervisorName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("supervisorName").unwrap_or_default().to_owned();
    let supervisor = supervisors::named(call.store(), &name).await?.ok_or_else(|| not_found(&name))?;

    Ok(answer(StatusCode::OK, view(call.ground(), &supervisor).await?))
  }
}

crate::moved!(SupervisorRead);

/// `GET /supervisors/{supervisorName}/reset-point`: a supervisor's reset point alone, read from the store the
/// reset writes it to.
pub struct SupervisorResetPoint;

impl Route for SupervisorResetPoint {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/supervisors/{supervisorName}/reset-point";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("supervisorName").unwrap_or_default().to_owned();

    if supervisors::named(call.store(), &name).await?.is_none() {
      return Err(not_found(&name));
    }

    let point = crate::supervisor_turn::overtaken::point(call.store(), &name).await;

    Ok(answer(StatusCode::OK, json!({ "supervisor": name, "resetPoint": point })))
  }
}

crate::moved!(SupervisorResetPoint);

/// `GET /supervisors/{supervisorName}/rooms`: every room the supervisor's turns went into, as the run ledger
/// records them, each with its latest run there, the latest first.
pub struct SupervisorRooms;

impl Route for SupervisorRooms {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/supervisors/{supervisorName}/rooms";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("supervisorName").unwrap_or_default().to_owned();

    if supervisors::named(call.store(), &name).await?.is_none() {
      return Err(not_found(&name));
    }

    let rows = store::all(
      call.store(),
      r#"SELECT "r"."chat_id", "counted"."runs", "r"."id", "r"."unit", "r"."started_at", "r"."finished_at"
         FROM (SELECT "chat_id", COUNT(*) AS "runs", MAX("id") AS "last" FROM "runs"
               WHERE "agent" = $1 AND "chat_id" IS NOT NULL GROUP BY "chat_id") AS "counted"
         JOIN "runs" AS "r" ON "r"."id" = "counted"."last"
         ORDER BY "r"."started_at" DESC, "r"."id" DESC"#,
      vec![name.into()],
    )
    .await?;
    let mut items = vec![];

    for row in &rows {
      items.push(json!({
        "roomId": row.try_get::<i64>("", "chat_id")?,
        "runs": row.try_get::<i64>("", "runs")?,
        "lastRunId": row.try_get::<i64>("", "id")?,
        "lastRunUnit": row.try_get::<String>("", "unit")?,
        "lastStartedAt": row.try_get::<i64>("", "started_at")?,
        "lastFinishedAt": row.try_get::<Option<i64>>("", "finished_at")?,
      }));
    }

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": false })))
  }
}

crate::moved!(SupervisorRooms);

// An order a body names, judged against the chat line.
async fn order_of(ground: &Ground, body: &Value) -> Result<Option<Order>, Refused> {
  let Some(entries) = body.get("order") else { return Ok(None) };

  judged(ground, entries, body.get("quotaShare")).await.map(Some)
}

// Entries judged as an order against the chat line, with the share beside them.
async fn judged(ground: &Ground, entries: &Value, share: Option<&Value>) -> Result<Order, Refused> {
  order::judge(Judging::of(ground), &ground.machine(), CHAT_LINE, entries, share, order::Empty::Refused)
    .await
    .map_err(|refusal| bad(&refusal.field, refusal.error))
}

// The order a registration names: its order whole, or the one entry of the provider and model it names, which
// must stand first where both are named.
async fn registered_order(ground: &Ground, body: &Value) -> Result<Order, Refused> {
  let text = |field: &str| body.get(field).and_then(Value::as_str).map(str::trim).unwrap_or_default();
  let (provider, model) = (text("provider"), text("model"));
  let order = match body.get("order") {
    Some(entries) => judged(ground, entries, body.get("quotaShare")).await?,
    None if !provider.is_empty() || !model.is_empty() => {
      judged(ground, &json!([{ "provider": provider, "model": model }]), body.get("quotaShare")).await?
    }
    None => return Err(bad("order", "A supervisor names its order.")),
  };
  let disagrees = (!provider.is_empty() || !model.is_empty())
    && order.first().is_none_or(|first| first.provider != provider || first.model != model);

  if disagrees {
    return Err(bad(
      "order",
      "The order's first entry is not the provider and model the registration names.",
    ));
  }

  Ok(order)
}

// A compaction threshold a body names: a fraction above 0 and at most 1.
fn fold_ratio_of(body: &Value) -> Result<Option<f64>, Refused> {
  match body.get("foldRatio") {
    None => Ok(None),
    Some(ratio) => ratio
      .as_f64()
      .filter(|ratio| *ratio > 0.0 && *ratio <= 1.0)
      .map(Some)
      .ok_or_else(|| bad("foldRatio", "A compaction threshold is a fraction above 0 and at most 1.")),
  }
}

// A repository a body names, which the store must hold; null clears it.
async fn repository_of(db: &DatabaseConnection, body: &Value) -> Result<Option<Option<String>>, Refused> {
  match body.get("repository") {
    None => Ok(None),
    Some(Value::Null) => Ok(Some(None)),
    Some(Value::String(name)) if crate::repositories::named(db, name).await?.is_some() => {
      Ok(Some(Some(name.clone())))
    }
    Some(_) => Err(bad("repository", "A supervisor's repository is one the store holds.")),
  }
}

/// `POST /supervisors`: a supervisor registered as one added on the console is, with its own order, and given
/// its brain.
pub struct SupervisorCreate;

impl Route for SupervisorCreate {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/supervisors";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let roster = &CONTROL_WORDS.supervisor_roster;
    let text =
      |field: &str| body.get(field).and_then(Value::as_str).map(str::trim).unwrap_or_default().to_owned();
    let name = text("name");
    let label = text("label");

    if !NAME.is_match(&name) {
      return Err(bad("name", roster.name.as_str()));
    }

    if supervisors::named(call.store(), &name).await?.is_some() {
      let taken = fill_marks(&roster.name_taken, &[("name", &name)]);

      return Err(Refused::new(StatusCode::CONFLICT, taken).with_code("name-taken"));
    }

    if label.is_empty() {
      return Err(bad("label", roster.label.as_str()));
    }

    let order = registered_order(call.ground(), &body).await?;
    let scope = session_scope_of(&body)?;
    let repository = repository_of(call.store(), &body).await?.flatten();
    let owner = match body.get("ownerAccount") {
      None | Some(Value::Null) => None,
      Some(owner) => {
        Some(owner.as_i64().ok_or_else(|| bad("ownerAccount", CONTROL_WORDS.agents.owner.as_str()))?)
      }
    };
    let fold_ratio = fold_ratio_of(&body)?.unwrap_or_else(|| order.default_fold_ratio());
    let conversation = scope.map_or_else(Conversation::console_added, Conversation::of_scope);
    let warming: BTreeMap<&str, bool> =
      order.entries.iter().map(|entry| (entry.provider.as_str(), true)).collect();
    let now = now_ms();

    run(
      call.store(),
      r#"INSERT INTO "supervisors" ("name", "label", "owner_account", "icon", "brain_folder", "session_scope",
           "turn_shape", "repository", "provider_order", "quota_share", "warming", "fold_ratio", "created_at",
           "updated_at")
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $13)"#,
      vec![
        name.clone().into(),
        label.clone().into(),
        owner.into(),
        SUPERVISORS.added_icon.clone().into(),
        format!("agents/{name}").into(),
        conversation.scope.word().into(),
        conversation.shape.word().into(),
        repository.into(),
        order.entries_text().into(),
        order.quota_share.into(),
        serde_json::to_string(&warming).map_err(Refused::failed)?.into(),
        fold_ratio.into(),
        now.into(),
      ],
    )
    .await?;

    let brain = born(call.ground().peers.brain.as_ref(), &name, &label, owner).await;
    let supervisor = supervisors::named(call.store(), &name).await?.ok_or_else(|| not_found(&name))?;

    eprintln!("agency: the supervisor {name} was registered");

    Ok(answer(
      StatusCode::CREATED,
      json!({ "supervisor": view(call.ground(), &supervisor).await?, "brain": brain }),
    ))
  }
}

crate::moved!(SupervisorCreate);

/// The registered supervisor's own brain asked of the brain, under its name, with its label, filed under its
/// owner. The supervisor stands whatever the brain answers, so what it could not do is logged.
async fn born(brain: &dyn Brain, name: &str, label: &str, owner: Option<i64>) -> BrainBirth {
  match brain.make(name, label, owner).await {
    Ok(Made::Made) => BrainBirth::Made,
    Ok(Made::Kept) => BrainBirth::Kept,
    Ok(Made::Undeployed) => BrainBirth::None,
    Err(fault) => {
      eprintln!("agency: the supervisor {name} was registered without its brain: {fault}");

      BrainBirth::Failed
    }
  }
}

/// `PATCH /supervisors/{supervisorName}`: the fields the body names changed, each judged as the registration
/// judges it.
pub struct SupervisorChange;

impl Route for SupervisorChange {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/supervisors/{supervisorName}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let name = call.param("supervisorName").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let held = supervisors::named(call.store(), &name).await?.ok_or_else(|| not_found(&name))?;
    let scope = session_scope_of(&body)?;
    let (session_scope, turn_shape) = scope.map_or_else(
      || (held.session_scope.clone(), held.turn_shape.clone()),
      |scope| (scope.word().to_owned(), scope.shape().word().to_owned()),
    );
    let order = order_of(call.ground(), &body).await?.unwrap_or(held.order.clone());
    let active = match body.get("active") {
      None => held.order.forced.clone(),
      Some(Value::Null) => None,
      Some(named @ Value::Object(_)) => Some(
        order::judge_one(Judging::of(call.ground()), &call.ground().machine(), CHAT_LINE, named)
          .await
          .map_err(|why| bad("active", why))?,
      ),
      Some(_) => {
        return Err(bad("active", "A forced provider is a provider and a model, or null for none."));
      }
    };
    let fold_ratio = fold_ratio_of(&body)?.unwrap_or(held.fold_ratio);
    let repository = repository_of(call.store(), &body).await?.unwrap_or(held.repository.clone());
    let label = match body.get("label") {
      None => held.label.clone(),
      Some(Value::String(label)) if !label.trim().is_empty() => Some(label.trim().to_owned()),
      Some(_) => return Err(bad("label", CONTROL_WORDS.supervisor_roster.label.as_str())),
    };
    let warming = match body.get("warming") {
      None => held.warming.clone(),
      Some(Value::Object(switches)) => {
        let mut warming = held.warming.clone();

        for (provider, on) in switches {
          let on = on.as_bool().ok_or_else(|| bad("warming", "A provider's warming is on or off."))?;

          warming.insert(provider.clone(), on);
        }

        warming
      }
      Some(_) => return Err(bad("warming", "Warming is an object from a provider to on or off.")),
    };
    let voice = match body.get("voice") {
      None => held.voice.clone(),
      Some(Value::Null) => None,
      Some(Value::String(voice)) => Some(voice.clone()),
      Some(_) => return Err(bad("voice", "A voice is a name, or null for none.")),
    };

    let card_rank = match body.get("cardRank") {
      None => held.card_rank,
      Some(Value::Null) => None,
      Some(rank) => Some(rank.as_u64().and_then(|rank| u32::try_from(rank).ok()).ok_or_else(|| {
        bad("cardRank", "A card rank is a whole number of zero or more, or null for none.")
      })?),
    };

    if let Some(Value::Object(gates)) = body.get("gates") {
      for (gate, on) in gates {
        let on = on.as_bool().ok_or_else(|| bad("gates", CONTROL_WORDS.agents.gate_switch.as_str()))?;

        set_setting(call.store(), &format!("agent_gate:{name}:{gate}"), if on { "on" } else { "off" })
          .await?;
      }
    }

    if let Some(sampling) = body.get("sampling") {
      let key = agent_sampling::key(&name);
      let kept = agent_sampling::held_values(settings::get(call.store(), &key).await?);
      let next = agent_sampling::changed(sampling, kept).map_err(|fault| match fault {
        super::fault::Fault::Field(_, error) => Refused::failed(error),
        super::fault::Fault::Failed(failed) => failed,
      })?;

      settings::set(call.store(), &key, &Value::Object(next).to_string()).await?;
    }

    run(
      call.store(),
      r#"UPDATE "supervisors" SET "label" = $2, "provider_order" = $3, "quota_share" = $4, "warming" = $5,
           "fold_ratio" = $6, "repository" = $7, "voice" = $8, "updated_at" = $9, "active_provider" = $10,
           "active_model" = $11, "card_rank" = $12, "session_scope" = $13, "turn_shape" = $14
         WHERE "name" = $1"#,
      vec![
        name.clone().into(),
        label.into(),
        order.entries_text().into(),
        order.quota_share.into(),
        serde_json::to_string(&warming).map_err(Refused::failed)?.into(),
        fold_ratio.into(),
        repository.into(),
        voice.into(),
        now_ms().into(),
        active.as_ref().map(|active| active.provider.clone()).into(),
        active.as_ref().map(|active| active.model.clone()).into(),
        card_rank.and_then(|rank| i32::try_from(rank).ok()).into(),
        session_scope.into(),
        turn_shape.into(),
      ],
    )
    .await?;

    let supervisor = supervisors::named(call.store(), &name).await?.ok_or_else(|| not_found(&name))?;

    Ok(answer(StatusCode::OK, view(call.ground(), &supervisor).await?))
  }
}

crate::moved!(SupervisorChange);

// Whether the query switches a flag on.
fn switched(call: &Call, name: &str) -> bool {
  call.query(name).is_some_and(|value| value == "true")
}

/// `DELETE /supervisors/{supervisorName}`: a registration ended, with everything of the supervisor's own but
/// its brain where the caller keeps it.
pub struct SupervisorDelete;

impl Route for SupervisorDelete {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/supervisors/{supervisorName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let words = &CONTROL_WORDS.agents;
    let name = call.param("supervisorName").unwrap_or_default().to_owned();
    let force = switched(&call, "force");
    let forget_brain = switched(&call, "forgetBrain");
    let rooms = rooms_named(call.query("rooms").map(Value::String).as_ref());
    let supervisor = supervisors::named(call.store(), &name).await?.ok_or_else(|| not_found(&name))?;
    let label = supervisor.label.clone().unwrap_or_else(|| name.clone());

    if supervisor.builtin {
      let seeded = fill_marks(&words.seeded, &[("label", &label)]);

      return Err(Refused::new(StatusCode::CONFLICT, seeded).with_code("seeded"));
    }

    if !force && held(&agent_lock_path(&call.data_root(), &name)) {
      let running = fill_marks(&words.running, &[("label", &label)]);

      return Err(Refused::new(StatusCode::CONFLICT, running).with_code("running"));
    }

    // A supervisor's conversation is reset first, so the rooms it answered keep their record and only its own
    // rows go; the reset cuts off a turn still holding it.
    let reset = reset_whole(&call, &name, rooms, &asked_by(&Value::Null), forget_brain);
    let left = Left::of(&call, &name);

    to_its_end(async move {
      reset.await?;

      let written = sea_orm::TransactionTrait::begin(&left.db).await?;

      if let Err(fault) = remove(&written, &name).await {
        return fault.answer();
      }

      written.commit().await?;
      left.taken(&name, forget_brain).await;
      eprintln!("agency: the registration of the supervisor {name} was ended");

      Ok(StatusCode::NO_CONTENT.into_response())
    })
    .await
  }
}

crate::moved!(SupervisorDelete);

#[allow(dead_code)]
async fn held_row(db: &DatabaseConnection, name: &str) -> Result<bool, Refused> {
  Ok(
    one(db, r#"SELECT 1 AS "one" FROM "supervisors" WHERE "name" = $1"#, vec![name.into()])
      .await?
      .is_some(),
  )
}
