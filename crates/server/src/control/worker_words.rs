//! Words said to a worker across the boundary (docs/api.md, 「Workers」): written down with their line in the
//! worker's log and offered to the worker's session at once, whether or not a turn of its own is working.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use super::{Call, Refused, Route};
use crate::places::AgentRow;
use crate::worker_words::said;

/// Words the owner said to one worker.
pub struct SayWords;

impl Route for SayWords {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/workers/{worker}/words";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let worker = call.param("worker").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let Some(text) =
      body.get("text").and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty())
    else {
      return Err(Refused::new(StatusCode::BAD_REQUEST, "the words are a text that is not empty"));
    };

    if !AgentRow::named(call.store(), &worker).await?.is_some_and(|row| row.kind == "worker") {
      return Err(Refused::new(StatusCode::BAD_REQUEST, format!("no worker is named {worker}")));
    }

    match call.words() {
      Some(words) => words.say(&worker, text).await?,
      // With no backend to offer them to, the words wait for the server's round.
      None => said::say(call.store(), &worker, text).await?,
    };

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(SayWords);
