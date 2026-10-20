#![allow(
  dead_code,
  unused_imports,
  unused_variables,
  clippy::needless_lifetimes,
  clippy::let_and_return,
  clippy::unnecessary_to_owned
)]
//! What a job's run asks of its worker's runtime across the boundary (docs/api.md, 「Workers」): a session
//! handed over, a run noted, the bench made ready, the servers its jobs left swept, the documents a job is
//! composed with, and which of a change's files a session's prefix is built from.

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

// The worker the path names.
fn worker_of(call: &Call) -> Result<String, Refused> {
  call
    .param("workerName")
    .filter(|worker| !worker.is_empty())
    .map(str::to_owned)
    .ok_or_else(|| bad("a worker's name is required"))
}

async fn read_body<T: DeserializeOwned>(call: &mut Call, what: &str) -> Result<T, Refused> {
  serde_json::from_value(call.body().await?).map_err(|error| bad(format!("{what}: {error}")))
}

/// A handover as the job's runner asks it.
#[derive(Deserialize)]
struct Handover {
  model: String,
  cwd: String,
  thread: Option<String>,
  session: Option<String>,
  note: Option<String>,
}

/// A worker's session handed over: the one named, or the kept one on the model with its record dropped.
pub struct HandOver;

impl Route for HandOver {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/workers/{worker}/sessions/handover";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let worker = worker_of(&call)?;
    let asked: Handover = read_body(&mut call, "a handover").await?;
    let ground = call.ground();
    let db = &ground.db;

    if !AgentRow::named(db, &worker).await?.is_some_and(|row| row.kind == "worker") {
      return Err(bad(format!("no worker is named {worker}")));
    }

    // With none named, the session kept on the model under the login its newest record names, while it may
    // still be resumed; its record goes once it is handed over.
    let (held, kept_under) = match asked.session {
      Some(session) => (session, None),
      None => {
        let kept = records::sessions(db, &worker)
          .await?
          .into_iter()
          .filter(|session| session.model == asked.model)
          .max_by_key(|session| session.last_reply_at);
        let Some(kept) = kept else { return Ok(StatusCode::NO_CONTENT.into_response()) };
        let ttl = session_ttl_ms(ground, &asked.model).await;
        let resumable = records::resumable(db, &worker, &asked.model, &kept.account, ttl).await?;
        let Some(held) = resumable else { return Ok(StatusCode::NO_CONTENT.into_response()) };

        (held, Some(kept.account))
      }
    };
    let line =
      worker_run(ground, &Trees::of(ground), Some(&worker), None, &asked.model, &asked.cwd).await?;
    let run = SessionRun {
      mode: PROVIDER,
      line,
      prompt: String::new(),
      resume: Some(held),
      account: None,
      placement: None,
      thread: asked.thread,
    };

    held::hand_over(ground, &worker, &run, asked.note.as_deref()).await?;

    if let Some(account) = kept_under {
      records::forget(db, &worker, &asked.model, Some(&account)).await?;
    }

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

/// A run of the worker's noted on its row.
pub struct NoteRun;

impl Route for NoteRun {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/workers/{worker}/runs";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    records::note_run(call.store(), &worker_of(&call)?).await?;

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

/// The worker's bench made ready for a job: its workbench, with what a job before it asked of its dev server
/// taken back, and its dev pane brought up where it has a dev command.
pub struct MakeBench;

impl Route for MakeBench {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/workers/{worker}/bench";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let worker = worker_of(&call)?;
    let ground = call.ground();

    if let Some(row) = AgentRow::named(&ground.db, &worker).await? {
      dev::job_workbench(ground, &worker);

      if matches!(dev::dev_command(ground, &row).await, Ok(Some(_))) {
        dev::dev_pane(ground, &worker).await;
      }
    }

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

/// The servers the worker's jobs left listening ended, its job read as over.
pub struct SweepServers;

impl Route for SweepServers {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/workers/{workerName}/servers";

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
  const PATH: &'static str = "/workers/{workerName}/definition";

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
    let skills = shown.agent_dir("worker").join("plugin").join("skills").to_string_lossy().into_owned();
    let plain = shown.rulebook_path("plain-writing").to_string_lossy().into_owned();
    let trailer = render(
      &read_required(&documents.join("trailer.md"))?,
      &[("graph", &graph), ("skills", &skills), ("plainWriting", &plain)],
    );
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
