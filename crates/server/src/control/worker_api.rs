//! The workers as resources (docs/api.md, 「Workers」): the list, one worker, its checkout, node, role,
//! sampling and runs, and the worker added, changed or removed.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use sea_orm::QueryResult;
use serde_json::{Value, json};
use store::{all, one, run};

use policy::roles::ROLES;

use super::agent_sampling;
use super::fault::Fault;
use super::{Call, Refused, Route, answer};

fn faulted(fault: Fault) -> Refused {
  match fault {
    Fault::Field(field, error) => Refused::new(StatusCode::BAD_REQUEST, format!("{field}: {error}")),
    Fault::Failed(refused) => refused,
  }
}

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

const COLUMNS: &str = r#""name", "label", "cwd", "icon", "builtin", "node", "role", "resting_since",
  "session_id", "last_run_at", "runs", "prefix_documents""#;

/// The prefix documents a body names (docs/worker-bench.md, 「Prefix documents」): none where it names
/// none, and the list as the store keeps it, null for an empty one, where it names a list of paths none of
/// which is empty or climbs with `..`.
fn prefix_documents(body: &Value) -> Result<Option<sea_orm::Value>, Refused> {
  let Some(listed) = body.get("prefixDocuments") else {
    return Ok(None);
  };
  let paths = listed.as_array().ok_or_else(|| bad("prefixDocuments is a list of paths"))?;
  let mut kept = Vec::with_capacity(paths.len());

  for path in paths {
    let path = path.as_str().map(str::trim).filter(|path| !path.is_empty());
    let Some(path) = path else {
      return Err(bad("prefixDocuments: every path is a string that is not empty"));
    };

    if std::path::Path::new(path)
      .components()
      .any(|part| part == std::path::Component::ParentDir)
    {
      return Err(bad(format!("prefixDocuments: {path} climbs with ..")));
    }

    kept.push(path.to_owned());
  }

  Ok(Some(sea_orm::Value::String(
    (!kept.is_empty()).then(|| Box::new(Value::from(kept).to_string())),
  )))
}

async fn member(db: &sea_orm::DatabaseConnection, name: &str) -> Result<Value, Refused> {
  let sql = format!(r#"SELECT {COLUMNS} FROM "agents" WHERE "name" = $1 AND "kind" = 'worker'"#);
  let row = one(db, &sql, vec![name.into()])
    .await?
    .ok_or_else(|| Refused::new(StatusCode::NOT_FOUND, format!("no worker is named {name}")))?;
  member_of(db, &row).await
}

async fn member_of(db: &sea_orm::DatabaseConnection, row: &QueryResult) -> Result<Value, Refused> {
  let name: String = row.try_get("", "name")?;
  let resting: Option<i64> = row.try_get("", "resting_since")?;
  let sampling = match agent_sampling::sampling_owner(db, &name).await {
    Ok(owner) => agent_sampling::view(db, &owner)
      .await
      .map(|view| view["values"].clone())
      .unwrap_or(Value::Null),
    Err(_) => Value::Null,
  };

  // A worker is working while a job of its is starting or running, a restart under the job among them.
  let working = one(
    db,
    r#"SELECT "id", "item_id" FROM "jobs" WHERE "worker_name" = $1 AND "state" IN ('starting', 'running')
       ORDER BY "id" DESC LIMIT 1"#,
    vec![name.clone().into()],
  )
  .await?;
  let job = working.as_ref().and_then(|row| row.try_get::<i64>("", "id").ok());
  let item = working.as_ref().and_then(|row| row.try_get::<i64>("", "item_id").ok());
  let state = match (resting, job) {
    (Some(_), _) => "resting",
    (None, Some(_)) => "working",
    (None, None) => "idle",
  };

  Ok(json!({
    "name": name,
    "label": row.try_get::<Option<String>>("", "label")?.unwrap_or_else(|| row.try_get::<String>("", "name").unwrap_or_default()),
    "cwd": row.try_get::<Option<String>>("", "cwd")?.unwrap_or_default(),
    "icon": ROLES.icon("worker").map(str::to_owned).or(row.try_get::<Option<String>>("", "icon")?),
    "builtin": row.try_get::<i64>("", "builtin")? != 0,
    "nodeName": row.try_get::<Option<String>>("", "node")?.filter(|node| !node.is_empty()),
    "role": row.try_get::<Option<String>>("", "role")?,
    "state": state,
    "jobId": job,
    "itemId": item,
    "sampling": sampling,
    "restingSince": resting,
    "sessionId": row.try_get::<Option<String>>("", "session_id")?,
    "runCount": row.try_get::<i64>("", "runs")?,
    "lastRunAt": row.try_get::<Option<i64>>("", "last_run_at")?,
    "prefixDocuments": row
      .try_get::<Option<String>>("", "prefix_documents")?
      .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
      .unwrap_or_default(),
  }))
}

/// Every worker.
pub struct ListWorkers;

impl Route for ListWorkers {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/workers";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let sql = format!(r#"SELECT {COLUMNS} FROM "agents" WHERE "kind" = 'worker' ORDER BY "name""#);
    let rows = all(call.store(), &sql, vec![]).await?;
    let mut items = Vec::with_capacity(rows.len());

    for row in &rows {
      items.push(member_of(call.store(), row).await?);
    }

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": false })))
  }
}

crate::moved!(ListWorkers);

/// A worker added.
pub struct AddWorker;

impl Route for AddWorker {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/workers";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let name = body.get("name").and_then(Value::as_str).map(str::trim).filter(|name| !name.is_empty());
    let Some(name) = name else { return Err(bad("name (a string) is required")) };
    let field =
      |key: &str| body.get(key).and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty());
    let cwd = field("cwd")
      .map(str::to_owned)
      .unwrap_or_else(|| call.ground().source.to_string_lossy().into_owned());
    let label = field("label").unwrap_or(name);
    let documents = prefix_documents(&body)?;

    run(
      call.store(),
      r#"INSERT INTO "agents" ("name", "kind", "label", "cwd", "icon", "role", "node", "builtin", "prefix_documents")
         VALUES ($1, 'worker', $2, $3, $4, $5, $6, 0, $7)
         ON CONFLICT ("name") DO UPDATE SET "label" = EXCLUDED."label", "cwd" = EXCLUDED."cwd",
           "icon" = EXCLUDED."icon", "role" = EXCLUDED."role", "node" = EXCLUDED."node",
           "prefix_documents" = EXCLUDED."prefix_documents""#,
      {
        let values: Vec<sea_orm::Value> = vec![
          name.to_owned().into(),
          label.to_owned().into(),
          cwd.into(),
          sea_orm::Value::String(field("icon").map(|value| Box::new(value.to_owned()))),
          sea_orm::Value::String(field("role").map(|value| Box::new(value.to_owned()))),
          sea_orm::Value::String(field("nodeName").map(|value| Box::new(value.to_owned()))),
          documents.unwrap_or(sea_orm::Value::String(None)),
        ];
        values
      },
    )
    .await?;

    Ok(answer(StatusCode::CREATED, member(call.store(), name).await?))
  }
}

crate::moved!(AddWorker);

/// One worker: checkout, node, role, sampling, session, runs.
pub struct WorkerRoute;

impl Route for WorkerRoute {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/workers/{workerName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    Ok(answer(
      StatusCode::OK,
      member(call.store(), call.param("workerName").unwrap_or_default()).await?,
    ))
  }
}

crate::moved!(WorkerRoute);

/// A worker changed: its fields, its sampling, its node, or stood again.
pub struct ChangeWorker;

impl Route for ChangeWorker {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/workers/{workerName}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let name = call.param("workerName").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let db = call.store();
    let documents = prefix_documents(&body)?;

    if let Some(sampling) = body.get("sampling").filter(|sampling| sampling.is_object()) {
      let agent = agent_sampling::sampling_owner(db, &name).await.map_err(faulted)?;
      let held =
        agent_sampling::held_values(crate::queue::settings::get(db, &agent_sampling::key(&agent)).await?);
      let next = agent_sampling::changed(sampling, held).map_err(faulted)?;

      crate::queue::settings::set(db, &agent_sampling::key(&agent), &Value::Object(next).to_string())
        .await?;
    }

    let field =
      |key: &str| body.get(key).and_then(Value::as_str).map(str::trim).filter(|value| !value.is_empty());

    // A worker's node is its placement, not a column: the node the node service keeps, the path the
    // worker's checkout stands at there, and the checkout this machine reads it through are judged and
    // written as one change. A body that names the node the worker already stands on leaves it alone,
    // since a PATCH carries the whole resource.
    let asked_node = body
      .get("nodeName")
      .and_then(Value::as_str)
      .map(str::trim)
      .filter(|node| !node.is_empty());
    let held_node = one(
      db,
      r#"SELECT "node" FROM "agents" WHERE "name" = $1 AND "kind" = 'worker'"#,
      vec![name.clone().into()],
    )
    .await?
    .and_then(|row| row.try_get::<Option<String>>("", "node").ok().flatten())
    .filter(|node| !node.is_empty());
    let placed =
      body.get("nodeName").is_some() && asked_node.filter(|node| *node != "none") != held_node.as_deref();

    if placed {
      let node = asked_node.filter(|node| *node != "none");
      let (ground, machine, places) =
        (call.shared_ground(), call.machine().clone(), call.places().cloned());

      crate::control::worker::place_worker(ground, machine, places, &name, node, field("cwd")).await?;
    }

    let mut sets = Vec::new();
    let mut values: Vec<sea_orm::Value> = Vec::new();
    let mut push = |column: &str, value: Option<&str>| {
      if let Some(value) = value {
        values.push(value.to_owned().into());
        sets.push(format!(r#""{column}" = ${}"#, values.len()));
      }
    };
    push("label", field("label"));
    push("cwd", if placed { None } else { field("cwd") });
    push("icon", field("icon"));
    push("role", field("role"));
    push("node", if placed { None } else { field("nodeName").or_else(|| field("node")) });

    if let Some(documents) = documents {
      values.push(documents);
      sets.push(format!(r#""prefix_documents" = ${}"#, values.len()));
    }

    if !sets.is_empty() {
      values.push(name.clone().into());
      let sql = format!(r#"UPDATE "agents" SET {} WHERE "name" = ${}"#, sets.join(", "), values.len());
      run(db, &sql, values).await?;
    }

    if !placed
      && body.get("stand").and_then(Value::as_bool) == Some(true)
      && let Some(places) = call.places().cloned()
    {
      tokio::spawn(async move { crate::places::standby::restand_all(&places.ground, None).await });
    }

    if body.get("bench").and_then(Value::as_bool) == Some(true) {
      let ground = call.ground();
      let bench = crate::places::dev::workbench(ground, &name);

      let _ = std::fs::create_dir_all(&bench);
      let _ = std::fs::remove_file(bench.join("dev-server.wanted"));
    }

    Ok(answer(StatusCode::OK, member(db, &name).await?))
  }
}

crate::moved!(ChangeWorker);

/// A worker removed.
pub struct RemoveWorker;

impl Route for RemoveWorker {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/workers/{workerName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("workerName").unwrap_or_default();
    let removed = run(
      call.store(),
      r#"DELETE FROM "agents" WHERE "name" = $1 AND "kind" = 'worker'"#,
      vec![name.into()],
    )
    .await?;

    if removed == 0 {
      return Err(Refused::new(StatusCode::NOT_FOUND, format!("no worker is named {name}")));
    }

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(RemoveWorker);
