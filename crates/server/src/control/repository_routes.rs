//! The repositories' routes (docs/api.md, 「repositories, prefix-checks and room-holdings」;
//! docs/repositories.md): every repository read with its checkouts, one added, changed — its fields, its
//! checkouts, its node, its pin and its sampling — its bench readied, and removed while no job holds a
//! checkout of it.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use policy::roles::ROLES;
use serde_json::{Value, json};
use store::{all, run};

use super::claude_accounts::Judging;
use super::{Call, Refused, Route, agent_sampling, answer, whole};
use crate::now_ms;
use crate::places::Ground;
use crate::queue::settings;
use crate::queue::workers::{self, Worker, WorkerRemoved};
use crate::repositories::{self, Repository};

fn bad(field: &str, error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error).with_field(field)
}

fn not_found(name: &str) -> Refused {
  Refused::new(StatusCode::NOT_FOUND, format!("There is no repository {name}."))
}

/// The job holding a checkout now, where one does: its id, its item and the role it runs as.
async fn holder(ground: &Ground, checkout: &str) -> Result<Option<Value>, Refused> {
  let rows = all(
    &ground.db,
    r#"SELECT "id", "item_id", "role" FROM "jobs" WHERE "worker_name" = $1 AND "state" IN ('starting', 'running')
       ORDER BY "id" DESC LIMIT 1"#,
    vec![checkout.into()],
  )
  .await?;

  Ok(rows.first().map(|row| {
    json!({
      "jobId": row.try_get::<i64>("", "id").ok(),
      "itemId": row.try_get::<i64>("", "item_id").ok(),
      "role": row.try_get::<Option<String>>("", "role").ok().flatten(),
    })
  }))
}

/// One repository as the routes answer it: every field of its row, its checkouts with whether a job holds
/// each, and its sampling.
pub async fn view(ground: &Ground, repository: &Repository) -> Result<Value, Refused> {
  let db = &ground.db;
  let mut checkouts = vec![];

  for worker in workers::workers(db, &ground.source).await? {
    if worker.name != repository.name && worker.support_of.as_deref() != Some(repository.name.as_str()) {
      continue;
    }

    checkouts.push(json!({
      "name": worker.name,
      "path": worker.cwd,
      "nodeName": worker.node,
      "job": holder(ground, &worker.name).await?,
    }));
  }

  let sampling = agent_sampling::view(db, &repository.name).await.map(|view| view["values"].clone()).ok();

  Ok(json!({
    "name": repository.name,
    "label": repository.label,
    "icon": ROLES.icon("repository"),
    "ownerAccount": repository.owner_account,
    "path": repository.path,
    "nodeName": repository.node,
    "supportCount": repository.support_count,
    "prefixDocuments": repository.prefix_documents,
    "preamble": repository.preamble,
    "rules": repository.rules,
    "devCommand": repository.dev_command,
    "pin": repository.pin.as_ref().map(|pin| json!({ "provider": pin.provider, "model": pin.model })),
    "downgradeHeal": repository.downgrade_heal,
    "sampling": sampling,
    "checkouts": checkouts,
  }))
}

/// `GET /repositories`: every repository.
pub struct RepositoryList;

impl Route for RepositoryList {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/repositories";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let mut items = vec![];

    for repository in repositories::every(call.store()).await? {
      items.push(view(call.ground(), &repository).await?);
    }

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": false })))
  }
}

crate::moved!(RepositoryList);

/// `GET /repositories/{repositoryName}`: one repository.
pub struct RepositoryRead;

impl Route for RepositoryRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/repositories/{repositoryName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("repositoryName").unwrap_or_default().to_owned();
    let repository = repositories::named(call.store(), &name).await?.ok_or_else(|| not_found(&name))?;

    Ok(answer(StatusCode::OK, view(call.ground(), &repository).await?))
  }
}

crate::moved!(RepositoryRead);

/// The fields a body names, each judged.
#[derive(Default)]
struct Fields {
  label: Option<Option<String>>,
  path: Option<String>,
  node: Option<Option<String>>,
  support_count: Option<i64>,
  prefix_documents: Option<Vec<String>>,
  preamble: Option<Option<String>>,
  rules: Option<Option<String>>,
  dev_command: Option<Option<String>>,
  pin: Option<Option<(String, String)>>,
  downgrade_heal: Option<bool>,
}

fn optional_text(body: &Value, field: &str) -> Result<Option<Option<String>>, Refused> {
  match body.get(field) {
    None => Ok(None),
    Some(Value::Null) => Ok(Some(None)),
    Some(Value::String(text)) if text.trim().is_empty() => Ok(Some(None)),
    Some(Value::String(text)) => Ok(Some(Some(text.clone()))),
    Some(_) => Err(bad(field, format!("{field} is text, or null for none."))),
  }
}

impl Fields {
  async fn of(ground: &Ground, body: &Value) -> Result<Self, Refused> {
    let mut fields = Self {
      label: optional_text(body, "label")?,
      node: optional_text(body, "nodeName")?,
      preamble: optional_text(body, "preamble")?,
      rules: optional_text(body, "rules")?,
      dev_command: optional_text(body, "devCommand")?,
      ..Self::default()
    };

    if let Some(path) = body.get("path") {
      match path.as_str().map(str::trim).filter(|path| !path.is_empty()) {
        Some(path) => fields.path = Some(path.to_owned()),
        None => return Err(bad("path", "A repository's path is its base checkout's directory.")),
      }
    }

    if let Some(count) = body.get("supportCount") {
      match whole(count).filter(|count| *count >= 0) {
        Some(count) => fields.support_count = Some(count),
        None => return Err(bad("supportCount", "A support count is a whole number of zero or more.")),
      }
    }

    if let Some(documents) = body.get("prefixDocuments") {
      let listed: Option<Vec<String>> = documents
        .as_array()
        .and_then(|list| list.iter().map(|one| one.as_str().map(str::to_owned)).collect());
      let Some(listed) = listed.filter(|listed| {
        listed
          .iter()
          .all(|path| !path.trim().is_empty() && !path.split('/').any(|part| part == ".."))
      }) else {
        return Err(bad(
          "prefixDocuments",
          "Prefix documents are paths, none empty and none climbing with ..",
        ));
      };

      fields.prefix_documents = Some(listed);
    }

    if let Some(pin) = body.get("pin") {
      fields.pin = Some(match pin {
        Value::Null => None,
        Value::Object(_) => {
          let entry = json!([{ "provider": pin["provider"], "model": pin["model"] }]);
          let judged = crate::order::judge(
            Judging::of(ground),
            &ground.machine(),
            crate::queue::routes::job_line(),
            &entry,
            None,
          )
          .await
          .map_err(|refusal| bad("pin", refusal.error))?;
          let first = judged
            .entries
            .into_iter()
            .next()
            .ok_or_else(|| bad("pin", "A pin names a provider and a model."))?;

          Some((first.provider, first.model))
        }
        _ => return Err(bad("pin", "A pin is a provider and a model, or null for none.")),
      });
    }

    if let Some(heal) = body.get("downgradeHeal") {
      fields.downgrade_heal =
        Some(heal.as_bool().ok_or_else(|| bad("downgradeHeal", "The downgrade heal is on or off."))?);
    }

    Ok(fields)
  }
}

/// `POST /repositories`: a repository added.
pub struct RepositoryCreate;

impl Route for RepositoryCreate {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/repositories";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let name = body.get("name").and_then(Value::as_str).map(str::trim).unwrap_or_default().to_owned();

    let shape = regex::Regex::new(&policy::workers::WORKERS.name_pattern).map_err(Refused::failed)?;

    if !shape.is_match(&name) {
      return Err(bad("name", "A repository's name is lowercase letters, digits and hyphens."));
    }

    if workers::taken_names(call.store()).await?.contains(&name) {
      return Err(
        Refused::new(StatusCode::CONFLICT, format!("The name {name} is taken.")).with_code("name-taken"),
      );
    }

    let fields = Fields::of(call.ground(), &body).await?;
    let path = fields.path.clone().ok_or_else(|| bad("path", "A repository names its path."))?;
    let pin = fields.pin.clone().flatten();
    let now = now_ms();

    run(
      call.store(),
      r#"INSERT INTO "repositories" ("name", "label", "owner_account", "path", "node", "support_count",
           "prefix_documents", "preamble", "prompts", "dev_command", "pinned_provider", "pinned_model", "downgrade_heal",
           "created_at", "updated_at")
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $14)"#,
      vec![
        name.clone().into(),
        fields.label.clone().flatten().into(),
        body.get("ownerAccount").and_then(Value::as_i64).into(),
        path.into(),
        fields.node.clone().flatten().into(),
        fields.support_count.unwrap_or(0).into(),
        serde_json::to_string(&fields.prefix_documents.clone().unwrap_or_default()).map_err(Refused::failed)?.into(),
        fields.preamble.clone().flatten().into(),
        fields.rules.clone().flatten().into(),
        fields.dev_command.clone().flatten().into(),
        pin.as_ref().map(|(provider, _)| provider.clone()).into(),
        pin.as_ref().map(|(_, model)| model.clone()).into(),
        i64::from(fields.downgrade_heal.unwrap_or(true)).into(),
        now.into(),
      ],
    )
    .await?;

    let repository = repositories::named(call.store(), &name).await?.ok_or_else(|| not_found(&name))?;

    Ok(answer(StatusCode::CREATED, view(call.ground(), &repository).await?))
  }
}

crate::moved!(RepositoryCreate);

/// `PATCH /repositories/{repositoryName}`: the fields the body names changed, its sampling saved.
pub struct RepositoryChange;

impl Route for RepositoryChange {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/repositories/{repositoryName}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let name = call.param("repositoryName").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let held = repositories::named(call.store(), &name).await?.ok_or_else(|| not_found(&name))?;
    let fields = Fields::of(call.ground(), &body).await?;
    let pin = fields
      .pin
      .clone()
      .unwrap_or_else(|| held.pin.clone().map(|pin| (pin.provider, pin.model)));

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
      r#"UPDATE "repositories" SET "label" = $2, "path" = $3, "node" = $4, "support_count" = $5,
           "prefix_documents" = $6, "preamble" = $7, "prompts" = $8, "dev_command" = $9, "pinned_provider" = $10,
           "pinned_model" = $11, "downgrade_heal" = $12, "updated_at" = $13
         WHERE "name" = $1"#,
      vec![
        name.clone().into(),
        fields.label.clone().unwrap_or(held.label.clone()).into(),
        fields.path.clone().unwrap_or(held.path.clone()).into(),
        fields.node.clone().unwrap_or(held.node.clone()).into(),
        fields.support_count.unwrap_or(held.support_count).into(),
        serde_json::to_string(&fields.prefix_documents.clone().unwrap_or(held.prefix_documents.clone()))
          .map_err(Refused::failed)?
          .into(),
        fields.preamble.clone().unwrap_or(held.preamble.clone()).into(),
        fields.rules.clone().unwrap_or(held.rules.clone()).into(),
        fields.dev_command.clone().unwrap_or(held.dev_command.clone()).into(),
        pin.as_ref().map(|(provider, _)| provider.clone()).into(),
        pin.as_ref().map(|(_, model)| model.clone()).into(),
        i64::from(fields.downgrade_heal.unwrap_or(held.downgrade_heal)).into(),
        now_ms().into(),
      ],
    )
    .await?;

    // The bench a job's runner readies as the job starts (docs/worker-bench.md, 「How it is kept」): the
    // workbench made, what a job before asked of the dev server taken back, and the dev pane brought up.
    if body.get("bench").and_then(Value::as_bool) == Some(true) {
      crate::places::dev::job_workbench(call.ground(), &name);
      crate::places::dev::dev_pane(call.ground(), &name).await;
    }

    let repository = repositories::named(call.store(), &name).await?.ok_or_else(|| not_found(&name))?;

    Ok(answer(StatusCode::OK, view(call.ground(), &repository).await?))
  }
}

crate::moved!(RepositoryChange);

/// `DELETE /repositories/{repositoryName}`: a repository no job holds a checkout of, removed.
pub struct RepositoryDelete;

impl Route for RepositoryDelete {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/repositories/{repositoryName}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let name = call.param("repositoryName").unwrap_or_default().to_owned();

    repositories::named(call.store(), &name).await?.ok_or_else(|| not_found(&name))?;

    let checkouts: Vec<Worker> = workers::workers(call.store(), &call.ground().source)
      .await?
      .into_iter()
      .filter(|worker| worker.name == name || worker.support_of.as_deref() == Some(name.as_str()))
      .collect();

    for checkout in &checkouts {
      if holder(call.ground(), &checkout.name).await?.is_some() {
        return Err(
          Refused::new(
            StatusCode::CONFLICT,
            format!("A job holds {}, a checkout of {name}.", checkout.name),
          )
          .with_code("conflict"),
        );
      }
    }

    run(
      call.store(),
      r#"DELETE FROM "repository_node_checkouts" WHERE "repository" = $1"#,
      vec![name.clone().into()],
    )
    .await?;
    run(
      call.store(),
      r#"DELETE FROM "repositories" WHERE "name" = $1"#,
      vec![name.clone().into()],
    )
    .await?;
    call.events().publish(WorkerRemoved { worker: name });

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(RepositoryDelete);
