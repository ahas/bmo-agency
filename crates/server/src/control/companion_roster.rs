#![allow(
  dead_code,
  unused_imports,
  unused_variables,
  clippy::needless_lifetimes,
  clippy::let_and_return,
  clippy::unnecessary_to_owned
)]
//! A companion added on the console and removed there (docs/companions.md, 「Where a companion and its rooms
//! come from」): agency holds the agent's half — its row, written once every field has passed, then its own
//! brain asked of the brain, and, as it is taken out, its two ranges reset first and then everything of its
//! own the reset keeps. The room it answers, its number there and the room's answerers are the consumer's
//! that holds the console's rooms.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use harness_machine::companion_prompt::{self, Roots};
use policy::companions::COMPANIONS;
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use regex::Regex;
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, TransactionTrait};
use serde_json::{Value, json};
use store::{one, run};

use super::companion_reset::{reset_whole, rooms_named, to_its_end};
use super::companions::roots;
use super::fault::{Fault, refused};
use super::sessions::{agent_row, asked_by};
use super::{Call, Refused, Route, answer};
use crate::conversational::Settings as Conversation;
use crate::lines::companion::{carries_answers, pinned_provider};
use crate::lines::place::Place;
use crate::now_ms;
use crate::peers::brain::{Brain, Made};
use crate::places::Ground;
use crate::places::home::Home;

static NAME: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(&COMPANIONS.name_pattern).expect("the companion name pattern reads"));

// What a companion is for: it talks in rooms (docs/agent-roles.md).
const ROLE: &str = "chat";

/// The tables keyed by the agent alone whose rows a removal takes after the reset: its settings, its base
/// prompt choice and the asks of its session resets.
const AGENT_TABLES: [&str; 3] = ["companion_settings", "agent_base_prompts", "session_resets"];

/// The tables keyed by a worker whose rows go with it: its sessions and the words said to it.
const WORKER_TABLES: [&str; 2] = ["worker_sessions", "worker_words"];

/// One field of the body, trimmed; empty where it is missing or no text.
pub(super) fn text(body: &Value, field: &str) -> String {
  body.get(field).and_then(Value::as_str).map(str::trim).unwrap_or_default().to_owned()
}

/// A companion asked for, every field as the body gave it.
struct Asked {
  name: String,
  label: String,
  provider: String,
  model: String,
  worker: String,
}

impl Asked {
  fn of(body: &Value) -> Self {
    Self {
      name: text(body, "name"),
      label: text(body, "label"),
      provider: text(body, "provider"),
      model: text(body, "model"),
      worker: text(body, "worker"),
    }
  }
}

/// Every field of the ask checked, in the order the dialog shows them; nothing is written until all have
/// passed. The pin is judged as a companion's turn judges it before it goes out.
async fn checked(written: &DatabaseTransaction, ground: &Ground, asked: &Asked) -> Result<(), Fault> {
  let words = &CONTROL_WORDS.companion_roster;

  if !NAME.is_match(&asked.name) {
    return refused("name", words.name.as_str());
  }

  if agent_row(written, &asked.name).await?.is_some() {
    return refused("name", fill_marks(&words.name_taken, &[("name", &asked.name)]));
  }

  if asked.label.is_empty() {
    return refused("label", words.label.as_str());
  }

  pin_checked(written, ground, &asked.provider, &asked.model, &asked.worker).await
}

/// A conversational agent's pin and the worker its items go to, judged as its turn judges the pin before it
/// goes out.
pub(super) async fn pin_checked(
  written: &DatabaseTransaction,
  ground: &Ground,
  provider: &str,
  model: &str,
  worker: &str,
) -> Result<(), Fault> {
  let words = &CONTROL_WORDS.companion_roster;

  if pinned_provider(ground, given(provider), given(model)).await.is_err() {
    let carries = harness_providers::provider(provider).is_some_and(carries_answers);

    return if carries {
      refused("model", words.model.as_str())
    } else {
      refused("provider", fill_marks(&words.provider, &[("provider", provider)]))
    };
  }

  let found = one(
    written,
    "SELECT name FROM agents WHERE name = $1 AND kind = 'worker'",
    vec![worker.into()],
  )
  .await?;

  if found.is_none() {
    return refused("worker", fill_marks(&words.worker, &[("worker", worker)]));
  }

  Ok(())
}

// A field the body gave, none where it gave it empty.
pub(super) fn given(field: &str) -> Option<&str> {
  Some(field).filter(|field| !field.is_empty())
}

/// A checked companion's row written as the console adds one: the settings a companion added there takes,
/// the chat role, the console's mark, its pin, the worker its items go to and its brain folder.
async fn write(written: &DatabaseTransaction, asked: &Asked) -> Result<(), Fault> {
  let added = Conversation::companion_added();

  run(
    written,
    "INSERT INTO agents (name, kind, session_scope, turn_shape, default_answerer, role, label, icon, pinned_provider,
       pinned_model, item_worker, builtin, brain_folder, last_run_at)
     VALUES ($1, 'chat', $2, $3, 0, $4, $5, $6, $7, $8, $9, 0, $10, $11)",
    vec![
      asked.name.clone().into(),
      added.scope.word().into(),
      added.shape.word().into(),
      ROLE.into(),
      asked.label.clone().into(),
      COMPANIONS.added_icon.clone().into(),
      asked.provider.clone().into(),
      asked.model.clone().into(),
      asked.worker.clone().into(),
      format!("agents/{}", asked.name).into(),
      (now_ms() / 1000).into(),
    ],
  )
  .await?;

  Ok(())
}

/// A companion's agent row stood up with the console's settings.
pub struct CompanionAdd;

impl Route for CompanionAdd {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/companions";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;

    if !body.is_object() {
      return Err(Refused::new(StatusCode::BAD_REQUEST, CONTROL_WORDS.request.action.as_str()));
    }

    let asked = Asked::of(&body);
    let ground = call.shared_ground();
    let written = ground.db.begin().await?;
    let made = async {
      checked(&written, &ground, &asked).await?;
      write(&written, &asked).await
    }
    .await;

    if let Err(fault) = made {
      return fault.answer();
    }

    written.commit().await?;
    given_a_brain(ground.peers.brain.as_ref(), &asked).await;

    Ok(answer(StatusCode::CREATED, json!({ "ok": true })))
  }
}

crate::moved!(CompanionAdd);

/// The added companion's own brain asked of the brain, under its name and with its label. The companion is
/// added whatever the brain answers, so what it could not do is logged and refuses nothing.
async fn given_a_brain(brain: &dyn Brain, asked: &Asked) {
  match brain.make(&asked.name, &asked.label, None).await {
    Ok(Made::Made) => eprintln!("agency: the companion {} was given its brain", asked.name),
    Ok(Made::Kept | Made::Undeployed) => {}
    Err(fault) => eprintln!("agency: the companion {} was added without its brain: {fault}", asked.name),
  }
}

/// The companion a removal is asked for, refused where it is none or where the deployment seeds it.
async fn removable<C: ConnectionTrait>(db: &C, agent: &str) -> Result<(), Fault> {
  let words = &CONTROL_WORDS.companion_roster;
  let companion = |row: &sea_orm::QueryResult| {
    Conversation::of(row).is_ok_and(|settings| settings.is_some_and(|settings| settings.companion()))
  };
  let Some(row) = agent_row(db, agent).await?.filter(companion) else {
    return refused("agent", fill_marks(&words.not_companion, &[("agent", agent)]));
  };

  if row.try_get::<i64>("", "builtin")? == 1 {
    let label = row.try_get::<Option<String>>("", "label")?.unwrap_or_else(|| agent.to_owned());

    return refused("agent", fill_marks(&words.seeded, &[("label", &label)]));
  }

  Ok(())
}

/// What the reset keeps of an agent taken out, in one write, its row last.
pub(super) async fn remove(written: &DatabaseTransaction, agent: &str) -> Result<(), Fault> {
  for table in AGENT_TABLES {
    run(written, &format!("DELETE FROM {table} WHERE agent = $1"), vec![agent.into()]).await?;
  }

  for table in WORKER_TABLES {
    run(written, &format!("DELETE FROM {table} WHERE worker = $1"), vec![agent.into()]).await?;
  }

  // Its own switches of its gates, its sampling and its reset point.
  run(
    written,
    "DELETE FROM settings WHERE key LIKE $1 OR key = $2 OR key = $3",
    vec![
      format!("agent_gate:{agent}:%").into(),
      format!("agent_sampling:{agent}").into(),
      store::reset_point::key(agent).into(),
    ],
  )
  .await?;
  run(written, "DELETE FROM agents WHERE name = $1", vec![agent.into()]).await?;

  Ok(())
}

/// What a removal reads of the call to take down what is left of the agent, read before it is awaited.
pub(super) struct Left {
  pub(super) db: DatabaseConnection,
  data_root: PathBuf,
  home: Option<Home>,
  roots: Roots,
  brain: Arc<dyn Brain>,
}

impl Left {
  pub(super) fn of(call: &Call, agent: &str) -> Self {
    Self {
      db: call.store().clone(),
      data_root: call.data_root(),
      home: call.places().and_then(|places| places.ground.home_of(agent, agent)),
      roots: roots(call),
      brain: call.ground().peers.brain.clone(),
    }
  }

  /// What stands for a removed agent and what is left under its name, taken once its row is gone: its
  /// program killed, so it writes no record of what the removal took as it goes, its workspace closed where
  /// herdr stands here, its place and home folder, its execution rows with their envelopes, the agent
  /// host's view and head records of it, its agent lock, and, where its brain is to go, everything the
  /// brain holds under its scope. Nothing here refuses the removal, which has already been written; what
  /// could not go is logged.
  pub(super) async fn taken(&self, agent: &str, forget_brain: bool) {
    let place = Place::of(&self.data_root, agent);
    let mut left = vec![];

    place.release_now().await;

    if let Some(home) = &self.home
      && let Err(fault) = home.close().await
    {
      left.push(format!("its workspace: {}", fault.message));
    }

    for folder in [place.path.clone(), companion_prompt::folder(&self.roots, agent)] {
      removed(&folder, &mut left);
    }

    match crate::door::execution::forget_agent(&self.db, &self.data_root, agent).await {
      Ok(unremoved) => left.extend(unremoved),
      Err(refusal) => left.push(format!("its execution rows: {}", refusal.detail)),
    }

    let host = agent_host::layout::Layout::new(&self.data_root).agent_files(agent);

    for path in host.into_iter().chain([crate::places::locks::agent_lock_path(&self.data_root, agent)]) {
      removed(&path, &mut left);
    }

    if forget_brain && let Err(fault) = self.brain.forget(agent).await {
      left.push(format!("its brain scope: {fault}"));
    }

    for what in left {
      eprintln!("agency: what was left of the removed agent {agent} was not removed: {what}");
    }
  }
}

// A file or a folder removed, what could not go named among what was left.
fn removed(path: &Path, left: &mut Vec<String>) {
  let gone = if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };

  if let Err(why) = gone
    && why.kind() != std::io::ErrorKind::NotFound
  {
    left.push(format!("{}: {why}", path.display()));
  }
}

/// A companion the console added taken out, with everything of its own.
pub struct CompanionRemove;

impl Route for CompanionRemove {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/companions/remove";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let agent = text(&body, "agent");
    let by = asked_by(&body);

    if let Err(fault) = removable(call.store(), &agent).await {
      return fault.answer();
    }

    let reset = reset_whole(&call, &agent, rooms_named(body.get("rooms")), &by, true);
    let left = Left::of(&call, &agent);

    to_its_end(async move {
      reset.await?;

      let written = left.db.begin().await?;

      if let Err(fault) = remove(&written, &agent).await {
        return fault.answer();
      }

      written.commit().await?;
      left.taken(&agent, true).await;
      eprintln!("agency: the companion {agent} was removed, as {by} asked");

      Ok(answer(StatusCode::OK, json!({ "ok": true })))
    })
    .await
  }
}

/// A companion removed from the deployment.
pub struct CompanionDelete;

impl Route for CompanionDelete {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/companions/{companionName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let agent = call.param("companionName").unwrap_or_default().to_owned();

    if let Err(fault) = removable(call.store(), &agent).await {
      return fault.answer();
    }

    store::run(
      call.store(),
      r#"DELETE FROM "companion_settings" WHERE "agent" = $1"#,
      vec![agent.clone().into()],
    )
    .await?;
    store::run(call.store(), r#"DELETE FROM "agents" WHERE "name" = $1"#, vec![agent.clone().into()])
      .await?;
    store::run(
      call.store(),
      r#"DELETE FROM "settings" WHERE "key" IN ($1, $2) OR "key" LIKE $3"#,
      vec![
        format!("reset_point:{agent}").into(),
        format!("agent_sampling:{agent}").into(),
        format!("agent_gate:{agent}:%").into(),
      ],
    )
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(CompanionDelete);
