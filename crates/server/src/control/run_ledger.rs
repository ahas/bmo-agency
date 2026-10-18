//! The run ledger across the boundary (docs/api.md, 「Run log and run records」): a run opened, read by its id
//! and closed, in the ledger's column names. A run the ledger holds no row of is answered in the base chat
//! library's code for nothing kept at the address a path names.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bmo_chat::{ErrorCode, WireError};
use serde::de::DeserializeOwned;
use serde_json::json;

use super::{Call, Refused, Route, answer};
use crate::run_ledger::{self, Closing, NotClosed, Opening};

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
