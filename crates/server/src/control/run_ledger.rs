//! The run ledger across the boundary (docs/api.md, 「Run log and run records」): a run opened, read by its id,
//! listed by the search it carried, closed, and its runner launched, in the ledger's column names. A run the
//! ledger holds no row of is answered in the base chat library's code for nothing kept at the address a path
//! names.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bmo_chat::{ErrorCode, WireError};
use serde::de::DeserializeOwned;
use serde_json::json;

use super::companion_reset::to_its_end;
use super::{Call, Refused, Route, answer};
use crate::run_ledger::{self, Closing, Launch, NotClosed, NotLaunched, Opening, SearchId};

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

// The run the path names by its id.
fn id_of(call: &Call) -> Result<i64, Refused> {
  call
    .param("id")
    .and_then(|id| id.parse().ok())
    .ok_or_else(|| bad("the run's id is a whole number"))
}

fn nothing_held(id: i64) -> Response {
  WireError::new(ErrorCode::NothingHeld, format!("the ledger holds no run {id}")).into_response()
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

    Ok(answer(StatusCode::CREATED, json!({ "id": id })))
  }
}

crate::moved!(OpenRun);

/// The runs that carried a search, by their ids.
pub struct SearchRuns;

impl Route for SearchRuns {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/runs";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let search = call
      .query("search")
      .filter(|search| !search.trim().is_empty())
      .ok_or_else(|| bad("the search the runs carried is named"))?;
    let runs = run_ledger::of_search(call.store(), &SearchId::Text(search)).await?;

    Ok(answer(StatusCode::OK, json!({ "runs": runs })))
  }
}

crate::moved!(SearchRuns);

/// A run read by its id.
pub struct ReadRun;

impl Route for ReadRun {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/runs/{id}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let id = id_of(&call)?;

    Ok(match run_ledger::read(call.store(), id).await? {
      Some(run) => answer(StatusCode::OK, json!(run)),
      None => nothing_held(id),
    })
  }
}

crate::moved!(ReadRun);

/// A run closed.
pub struct CloseRun;

impl Route for CloseRun {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/runs/{id}/close";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let id = id_of(&call)?;
    let closing: Closing = read_body(&mut call, "a run's closing").await?;

    match run_ledger::close(call.store(), id, &closing).await {
      Ok(()) => Ok(StatusCode::NO_CONTENT.into_response()),
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
  const PATH: &'static str = "/runs/{id}/launch";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let id = id_of(&call)?;
    let launch: Launch = read_body(&mut call, "a run's launch").await?;

    if launch.command.program.trim().is_empty() {
      return Err(bad("a run's launch names the program its runner is"));
    }

    // Carried to its end apart from the request, so a runner the host refused never leaves its run open.
    let ground = call.shared_ground();
    let launched = to_its_end(async move { Ok(run_ledger::launch(&ground, id, launch).await) }).await?;

    match launched {
      Ok(()) => Ok(StatusCode::NO_CONTENT.into_response()),
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
