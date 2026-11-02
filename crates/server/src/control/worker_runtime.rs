#![allow(
  dead_code,
  unused_imports,
  unused_variables,
  clippy::needless_lifetimes,
  clippy::let_and_return,
  clippy::unnecessary_to_owned
)]
//! What a job's run asks of its repository across the boundary (docs/api.md, 「repositories, prefix-checks and
//! room-holdings」): the servers its jobs left swept, the documents a job is composed with, and which of a
//! change's files a session's prefix is built from.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;

use super::{Call, Refused, Route, answer};
use crate::lines::prefix::touches_prefix;
use crate::lines::standing::session_ttl_ms;
use crate::lines::worker::{documents_of, worker_run};
use crate::lines::{Trees, read_required, render};
use crate::places::{AgentRow, dev, servers};
use crate::worker_session::held::{self, SessionRun};
use crate::worker_session::{PROVIDER, records};

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

// The repository the path names.
fn worker_of(call: &Call) -> Result<String, Refused> {
  call
    .param("repositoryName")
    .filter(|repository| !repository.is_empty())
    .map(str::to_owned)
    .ok_or_else(|| bad("a repository's name is required"))
}

async fn read_body<T: DeserializeOwned>(call: &mut Call, what: &str) -> Result<T, Refused> {
  serde_json::from_value(call.body().await?).map_err(|error| bad(format!("{what}: {error}")))
}

/// A handover as the job's runner asks it.
#[derive(Deserialize)]
struct Handover {
  model: String,
  cwd: String,
  session: Option<String>,
}

/// The servers the worker's jobs left listening ended, its job read as over.
pub struct SweepServers;

impl Route for SweepServers {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/repositories/{repositoryName}/servers";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let worker = worker_of(&call)?;
    let lines = servers::sweep(call.ground(), Some(std::slice::from_ref(&worker)), true).await?;

    Ok(answer(StatusCode::OK, json!({ "lines": lines })))
  }
}

crate::moved!(SweepServers);

/// What a worker's agent definition gives a job: the graph text, the trailer with its paths filled in as the
/// deployment shows them, and the worker's own handoff instruction where its folder holds one.
pub struct JobDocuments;

impl Route for JobDocuments {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/repositories/{repositoryName}/definition";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let worker = worker_of(&call)?;
    let ground = call.ground();
    let trees = Trees::of(ground);
    let documents = trees.agent_dir("worker");
    let graph = read_required(&documents.join("graph.md"))?;
    // The trailer names the paths by the deployment's link, the documents being read out of the job's tree.
    let shown = Trees {
      code: ground.shown_code_root(),
      ..trees.clone()
    };
    let trailer = crate::lines::worker::job_trailer(&trees, &shown)?;
    let own = trees.agent_dir(&documents_of(ground, &worker).await?).join("handoff.md");
    let handoff = std::fs::read_to_string(own).ok().map(|text| text.trim().to_owned());

    Ok(answer(
      StatusCode::OK,
      json!({ "graph": graph, "trailer": trailer, "handoff": handoff }),
    ))
  }
}

crate::moved!(JobDocuments);

/// The files a change moved, as the job's runner names them.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Moved {
  #[serde(default)]
  worker_name: Option<String>,
  files: Vec<String>,
}

/// Which of a change's files a worker session's prefix is built from.
pub struct PrefixTouched;

impl Route for PrefixTouched {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/prefix-checks";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let moved: Moved = read_body(&mut call, "a change's files").await?;
    let worker = moved.worker_name.clone().unwrap_or_default();

    if !AgentRow::named(call.store(), &worker).await?.is_some_and(|row| row.kind == "worker") {
      return Err(bad(format!("no worker is named {worker}")));
    }

    Ok(answer(StatusCode::OK, json!({ "touched": touches_prefix(&moved.files) })))
  }
}

crate::moved!(PrefixTouched);
