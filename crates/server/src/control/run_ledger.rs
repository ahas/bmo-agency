#![allow(
  dead_code,
  unused_imports,
  unused_variables,
  clippy::needless_lifetimes,
  clippy::let_and_return,
  clippy::unnecessary_to_owned
)]
//! The run ledger across the boundary (docs/api.md, 「Runs and log lines」): a run opened, read by its id,
//! listed by the search it carried, closed, and its runner launched. A run the ledger holds no row of is
//! answered in the base chat library's code for nothing kept at the address a path names.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bmo_chat::{ErrorCode, WireError};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use store::{all, one};

use super::companion_reset::to_its_end;
use super::{Call, Refused, Route, answer};
use crate::run_ledger::{self, Closing, Launch, NotClosed, NotLaunched, Opening, SearchId};

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

/// One run as the boundary answers it.
fn member(run: &run_ledger::Run) -> Value {
  json!({
    "id": run.id,
    "name": format!("{}:{}", run.agent, run.unit),
    "agentName": run.agent.clone(),
    "unitName": run.unit.clone(),
    "workerName": Value::Null,
    "jobId": Value::Null,
    "state": if run.finished_at.is_some() { "closed" } else { "open" },
    "startedAt": run.started_at * 1000,
    "endedAt": run.finished_at.map(|at| at * 1000),
    "outcome": run.outcome.clone(),
    "capEndsAt": Value::Null,
    "keptUntil": Value::Null,
    "contextTokens": run.context_tokens,
    "spentTokens": Value::Null,
    "counts": {},
  })
}

// The run the path names by its id.
fn id_of(call: &Call) -> Result<i64, Refused> {
  call
    .param("runId")
    .and_then(|id| id.parse().ok())
    .ok_or_else(|| bad("the run's id is a whole number"))
}

fn nothing_held(id: i64) -> Response {
  WireError::new(ErrorCode::NotFound, format!("the ledger holds no run {id}")).into_response()
}

async fn read_body<T: DeserializeOwned>(call: &mut Call, what: &str) -> Result<T, Refused> {
  serde_json::from_value(call.body().await?).map_err(|error| bad(format!("{what}: {error}")))
}

/// A run opened.
pub struct OpenRun;

impl Route for OpenRun {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/runs";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let opening: Opening = read_body(&mut call, "a run's opening").await?;
    let id = run_ledger::open(call.store(), &opening).await?;
    let run = run_ledger::read(call.store(), id)
      .await?
      .ok_or_else(|| Refused::failed("the run was opened but not read"))?;

    Ok(answer(StatusCode::CREATED, member(&run)))
  }
}

crate::moved!(OpenRun);

/// The runs that carried a search, by their ids.
pub struct SearchRuns;

impl Route for SearchRuns {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/runs";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let ids = match call.query("search").filter(|search| !search.trim().is_empty()) {
      Some(search) => run_ledger::of_search(call.store(), &SearchId::Text(search)).await?,
      None => {
        let after = call.query("after").and_then(|after| after.parse::<i64>().ok()).unwrap_or(0);
        let limit = call.query("limit").and_then(|limit| limit.parse::<i64>().ok()).unwrap_or(50);
        let agent = call.query("agentName").unwrap_or_default();
        let rows = all(
          call.store(),
          r#"SELECT "id" FROM "runs" WHERE "id" > $1 AND ($2 = '' OR "agent" = $2) ORDER BY "id" LIMIT $3"#,
          vec![after.into(), agent.into(), limit.into()],
        )
        .await?
        .iter()
        .map(|row| row.try_get::<i64>("", "id").map_err(Refused::from))
        .collect::<Result<Vec<_>, _>>()?;
        rows
      }
    };
    let mut items = Vec::new();
    for id in ids {
      if let Some(run) = run_ledger::read(call.store(), id).await? {
        items.push(member(&run));
      }
    }

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": false })))
  }
}

crate::moved!(SearchRuns);

/// A run read by its id.
pub struct ReadRun;

impl Route for ReadRun {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/runs/{runId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let id = id_of(&call)?;

    Ok(match run_ledger::read(call.store(), id).await? {
      Some(run) => answer(StatusCode::OK, member(&run)),
      None => nothing_held(id),
    })
  }
}

crate::moved!(ReadRun);

/// A run closed.
pub struct CloseRun;

impl Route for CloseRun {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/runs/{runId}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let id = id_of(&call)?;
    let asked = call.body().await?;
    let closing = Closing {
      finished_at: asked
        .get("finishedAt")
        .or_else(|| asked.get("finished_at"))
        .and_then(Value::as_i64)
        .unwrap_or_else(|| crate::now_ms() / 1000),
      outcome: asked
        .get("outcome")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
          asked.get("state").and_then(Value::as_str).map(|state| match state {
            "stopping" | "stopped" => "stopped".to_owned(),
            _ => "closed".to_owned(),
          })
        })
        .unwrap_or_else(|| "closed".to_owned()),
      context_tokens: asked
        .get("contextTokens")
        .or_else(|| asked.get("context_tokens"))
        .and_then(Value::as_i64),
    };

    match run_ledger::close(call.store(), id, &closing).await {
      Ok(()) => Ok(match run_ledger::read(call.store(), id).await? {
        Some(run) => answer(StatusCode::OK, member(&run)),
        None => nothing_held(id),
      }),
      Err(NotClosed::Unknown) => Ok(nothing_held(id)),
      Err(NotClosed::Closed) => Err(Refused::new(
        StatusCode::CONFLICT,
        format!("run {id} is closed already and keeps its closing"),
      )),
      Err(NotClosed::Store(why)) => Err(why.into()),
    }
  }
}

crate::moved!(CloseRun);

/// A run's runner launched in its agent's host.
pub struct LaunchRun;

impl Route for LaunchRun {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/executions";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let launch: Launch = read_body(&mut call, "a run's launch").await?;
    let id = launch.run_id.ok_or_else(|| bad("runId (a number) is required"))?;

    if launch.command.program.trim().is_empty() {
      return Err(bad("a run's launch names the program its runner is"));
    }
    let program = launch.command.program.clone();

    // Carried to its end apart from the request, so a runner the host refused never leaves its run open.
    let ground = call.shared_ground();
    let launched = to_its_end(async move { Ok(run_ledger::launch(&ground, id, launch).await) }).await?;

    match launched {
      Ok(()) => Ok(answer(
        StatusCode::CREATED,
        json!({
          "id": id,
          "runId": id,
          "command": program,
          "state": "launched",
          "cancelled": false,
        }),
      )),
      Err(NotLaunched::Unknown) => Ok(nothing_held(id)),
      Err(NotLaunched::Closed) => Err(Refused::new(
        StatusCode::CONFLICT,
        format!("run {id} is closed already, so no runner is started for it"),
      )),
      Err(NotLaunched::Refused(code)) => Err(Refused::new(
        StatusCode::SERVICE_UNAVAILABLE,
        format!("run {id}'s runner did not start ({code}), and the run is closed as an error"),
      )),
      Err(NotLaunched::Store(why)) => Err(why.into()),
    }
  }
}

crate::moved!(LaunchRun);
