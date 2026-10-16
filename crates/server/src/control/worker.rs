//! A job worker added, changed or removed (docs/worker-slots.md; docs/api.md, 「Workers」). The store is
//! the catalogue, so the rows and the checks of the request are answered here, a pin judged against what
//! the providers that would run it declare among them, and the pinned workers' programs stood again after
//! a pin moved.

use std::path::Path;
use std::sync::Arc;

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use futures_util::future::BoxFuture;
use harness_providers::Machine;
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;

use super::sessions::running_workers;
use super::{Call, Refused, Route, answer};
use crate::places::{Ground, Places};
use crate::queue::worker_rules::{self, Adding};
use crate::queue::workers::{self, Changes, NewWorker, Worker};
use crate::queue::{paths, pins};
use crate::verbs::agents::{self, AgentsHands, Stand};

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

// A string the body carries, trimmed, where it carries one with something in it.
fn words<'a>(body: &'a Value, field: &str) -> Option<&'a str> {
  body.get(field).and_then(Value::as_str).map(str::trim).filter(|said| !said.is_empty())
}

// One of the two workbench fields as the row keeps it: a string emptied is none, and anything that is
// not a string is the field left alone.
fn bench_text(body: &Value, field: &str) -> Option<Option<String>> {
  let text = body.get(field)?.as_str()?.trim();

  Some((!text.is_empty()).then(|| text.to_owned()))
}

/// A worker added, changed or removed.
pub struct WorkerChange;

impl Route for WorkerChange {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/worker";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let action = body.get("action").and_then(Value::as_str);
    let name = body.get("name").and_then(Value::as_str).map(str::trim).unwrap_or_default();

    if name.is_empty() {
      return Err(bad("name (string) is required"));
    }

    let hands = Hands::of(&call);
    let source = hands.ground.source.clone();
    let held = workers::workers(&hands.ground.db, &source)
      .await?
      .into_iter()
      .find(|worker| worker.name == name);

    match action {
      Some("add") => add(&hands, &body, name).await,
      Some("update") => update(&hands, &body, name, held, &source).await,
      Some("remove") => remove(&hands, name, held.as_ref()).await,
      _ => Err(bad("action must be one of add, update, remove")),
    }
  }
}

crate::moved!(WorkerChange);

/// The pinned workers' programs stood again (docs/agent-control.md, 「A worker standing with nothing to
/// do」): what is asked where a change — a worker's own sampling saved, a `workers` word — asks for the
/// pass. Refused where the server stands no places; answered at once, the pass not waited on, a lock still
/// held waited on for the seconds named.
pub struct WorkersStand;

impl Route for WorkersStand {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/workers/stand";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await.unwrap_or(Value::Null);
    let wait = body.get("lockWaitSeconds").and_then(Value::as_u64).map(std::time::Duration::from_secs);
    let Some(places) = call.places().cloned() else {
      return Err(Refused::new(
        StatusCode::CONFLICT,
        "this server stands no places to stand the workers' programs in",
      ));
    };

    tokio::spawn(async move { crate::places::standby::restand_all(&places.ground, wait).await });

    Ok(ok())
  }
}

crate::moved!(WorkersStand);

/// Where a worker's checkouts stand on the nodes (docs/work-nodes.md; docs/worker-slots.md): a base's
/// checkout on a node added or removed, or a worker of its own stood on a node with its path there, or
/// brought back to this machine at a path here. Each refusal is said as the command line says it.
pub struct WorkerNode;

impl Route for WorkerNode {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/worker/node";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let action = body.get("action").and_then(Value::as_str);
    let Some(name) = words(&body, "name") else {
      return Err(bad("name (string) is required"));
    };
    let node = words(&body, "node");
    let path = words(&body, "path");
    let hands = Hands::of(&call);
    let home = crate::verbs::home();
    let stand = Stand {
      ground: &hands.ground,
      machine: &hands.machine,
      hands: &hands,
      home: &home,
    };

    match action {
      Some(asked @ ("add-checkout" | "remove-checkout")) => {
        let Some(node) = node else {
          return Err(bad("node (string) is required"));
        };
        let outcome = if asked == "add-checkout" {
          agents::add_node_checkout(&stand, name, node, path).await
        } else {
          agents::remove_node_checkout(&stand, name, node, &hands.running().await).await
        };
        let (worker, cwd) = outcome.map_err(Refused::failed)?.map_err(bad)?;

        hands.restand();

        Ok(answer(StatusCode::OK, json!({ "ok": true, "worker": worker, "cwd": cwd })))
      }
      Some("place") => place(&stand, &hands, name, node, path).await,
      _ => Err(bad("action must be one of add-checkout, remove-checkout, place")),
    }
  }
}

crate::moved!(WorkerNode);

// A worker of its own stood on a node, or brought back to this machine at the path given here.
async fn place(
  stand: &Stand<'_>,
  hands: &Hands,
  name: &str,
  node: Option<&str>,
  path: Option<&str>,
) -> Result<Response, Refused> {
  let fault = match (node, path) {
    (None, None) => Some(CONTROL_WORDS.workers.back_here_path.clone()),
    _ => agents::worker_node_fault(stand, name, node, path).await.map_err(Refused::failed)?,
  };

  if let Some(fault) = fault {
    return Err(bad(fault));
  }

  let mut changes = Changes {
    node: Some(node.map(str::to_owned)),
    ..Changes::default()
  };

  match (node, path) {
    (None, Some(path)) => {
      changes.cwd = Some(paths::resolve(path).to_string_lossy().into_owned());

      let held = workers::workers(&stand.ground.db, &stand.ground.source)
        .await?
        .into_iter()
        .find(|worker| worker.name == name);

      if let Some(fault) = held.as_ref().and_then(|held| worker_rules::change_fault(held, &changes)) {
        return Err(bad(fault));
      }
    }
    (Some(node), Some(path)) => {
      changes.cwd =
        Some(agents::on_node_home(&stand.ground.db, node, path).await.map_err(Refused::failed)?);
    }
    _ => {}
  }

  if !workers::update_worker(&stand.ground.db, name, &changes).await? {
    return Err(bad(CONTROL_WORDS.workers.nothing.as_str()));
  }

  hands.restand();

  Ok(ok())
}

/// What a worker's change stands on, taken off the call before anything is awaited: agency's ground, the
/// machine, and the places the server stands where it stands them.
struct Hands {
  ground: Arc<Ground>,
  machine: Machine,
  places: Option<Places>,
}

impl Hands {
  fn of(call: &Call) -> Self {
    Self {
      ground: call.shared_ground(),
      machine: call.machine().clone(),
      places: call.places().cloned(),
    }
  }

  // The pinned workers' programs stood again after a pin moved, not waited on.
  fn restand(&self) {
    if let Some(places) = self.places.clone() {
      tokio::spawn(async move { crate::places::standby::restand_all(&places.ground, None).await });
    }
  }

  // The worker's workbench, its folder's trust and its dev server brought to what its row now says,
  // with nothing waiting on it.
  fn settle(&self, name: &str) {
    if let Some(places) = self.places.clone() {
      let name = name.to_owned();

      tokio::spawn(async move { crate::places::dev::settle(&places.ground, &name).await });
    }
  }
}

/// What the agents verb's steps ask of the process they run in, answered by the server standing here.
impl AgentsHands for Hands {
  fn settle<'a>(&'a self, worker: &'a str) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      if self.places.is_some() {
        crate::places::dev::settle(&self.ground, worker).await;
      }
    })
  }

  fn tell_workers(&self) -> BoxFuture<'_, ()> {
    self.restand();

    Box::pin(async {})
  }

  fn running(&self) -> BoxFuture<'_, Option<Vec<String>>> {
    Box::pin(running_workers(&self.ground))
  }

  fn standing_input(&self) -> String {
    String::new()
  }
}

fn ok() -> Response {
  answer(StatusCode::OK, json!({ "ok": true }))
}

async fn add(hands: &Hands, body: &Value, name: &str) -> Result<Response, Refused> {
  let db = &hands.ground.db;
  let label = words(body, "label").unwrap_or(name);
  // No path given is a directory of the worker's own, made once the rest is accepted.
  let given = body.get("path").and_then(Value::as_str).map(str::trim).filter(|path| !path.is_empty());
  let (cwd, made) = match given {
    Some(path) => (paths::resolve(path), false),
    None => (hands.ground.roots.data.join("worker-homes").join(name), true),
  };
  let cwd = cwd.to_string_lossy().into_owned();
  let preamble = bench_text(body, "preamble").flatten();
  let dev_command = bench_text(body, "devCommand").flatten();
  let adding = Adding {
    name,
    label,
    cwd: &cwd,
    made,
    preamble: preamble.as_deref(),
    dev_command: dev_command.as_deref(),
  };

  if let Some(fault) = worker_rules::new_worker_fault(&adding, &workers::taken_names(db).await?) {
    return Err(bad(fault));
  }

  if made {
    tokio::fs::create_dir_all(&cwd).await.map_err(Refused::failed)?;
  }

  workers::add_worker(
    db,
    &NewWorker {
      name,
      label,
      cwd: &cwd,
      preamble: preamble.as_deref(),
      dev_command: dev_command.as_deref(),
    },
  )
  .await?;
  hands.settle(name);

  Ok(answer(
    StatusCode::OK,
    json!({ "ok": true, "worker": { "name": name, "label": label, "cwd": cwd } }),
  ))
}

async fn update(
  hands: &Hands,
  body: &Value,
  name: &str,
  held: Option<Worker>,
  source: &Path,
) -> Result<Response, Refused> {
  let db = &hands.ground.db;
  let Some(held) = held else {
    return Err(Refused::new(
      StatusCode::NOT_FOUND,
      fill_marks(&CONTROL_WORDS.workers.not_found, &[("name", name)]),
    ));
  };
  let mut changes = Changes {
    label: words(body, "label").map(str::to_owned),
    cwd: words(body, "path").map(|path| paths::resolve(path).to_string_lossy().into_owned()),
    preamble: bench_text(body, "preamble"),
    dev_command: bench_text(body, "devCommand"),
    downgrade_heal: body.get("downgradeHeal").and_then(Value::as_bool),
    keepalive_stopped: body.get("keepaliveStopped").and_then(Value::as_bool),
    ..Changes::default()
  };

  // The one provider the worker is pinned to, or `auto` for none, as the providers that would run it
  // read it; a pin naming nothing that could is refused with the reason.
  if let Some(choice) = body.get("pinnedProvider").and_then(Value::as_str) {
    let pin = pins::pin_choice(db, choice).await?.map_err(bad)?;

    changes.pinned = Some((pin.provider, pin.model));
  }

  // The base's two support settings, judged as the pair they make once this change is written: either
  // may come alone, and the other is read off the row.
  let count = body.get("supportCount").and_then(Value::as_f64);
  let keepalive = body.get("supportKeepalive").and_then(Value::as_f64);
  let support_asked = count.is_some() || keepalive.is_some();

  if support_asked {
    let row = workers::worker_row(db, name, source).await?;
    let count = count.unwrap_or_else(|| row.as_ref().map_or(0, |row| row.support_count) as f64);
    let keepalive = keepalive.unwrap_or_else(|| row.as_ref().map_or(0, |row| row.support_keepalive) as f64);

    if let Some(fault) = worker_rules::support_fault(&held, count, keepalive) {
      return Err(bad(fault));
    }

    changes.support_count = Some(count as i64);
    changes.support_keepalive = Some(keepalive as i64);
  }

  if let Some(fault) = worker_rules::change_fault(&held, &changes) {
    return Err(bad(fault));
  }

  if !workers::update_worker(db, name, &changes).await? {
    return Err(bad(CONTROL_WORDS.workers.nothing.as_str()));
  }

  // The support rows follow the base. Where what is running cannot be read, nothing is removed.
  if support_asked || changes.label.is_some() || changes.cwd.is_some() {
    let running = running_workers(&hands.ground).await;
    let ground = &hands.ground;

    workers::settle_support(db, ground.peers.node.as_ref(), &ground.events, name, source, |worker| {
      running.as_ref().is_none_or(|running| running.iter().any(|one| one == worker))
    })
    .await?;
  }

  if changes.dev_command.is_some() {
    hands.settle(name);
  }

  if changes.pinned.is_some() {
    hands.restand();
  }

  Ok(ok())
}

async fn remove(hands: &Hands, name: &str, held: Option<&Worker>) -> Result<Response, Refused> {
  if let Some(fault) = worker_rules::remove_fault(held, name) {
    let status = if held.is_none() { StatusCode::NOT_FOUND } else { StatusCode::BAD_REQUEST };

    return Err(Refused::new(status, fault));
  }

  if !workers::remove_worker(&hands.ground.db, &hands.ground.events, name).await? {
    return Err(Refused::new(StatusCode::CONFLICT, CONTROL_WORDS.workers.not_removed.as_str()));
  }

  hands.settle(name);

  Ok(ok())
}
