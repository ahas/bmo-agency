//! Documents a session relies on, moved (docs/sessions.md, 「Document reload」): the paths named published
//! as `documents-moved` for the consumer that holds the rooms to tell the sessions concerned, with whether
//! the notice is due at once. Agency tells no session itself.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde::Serialize;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::events::Event;

/// Documents moved: their paths, as the reload named them, and whether the notice is due at once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DocumentsMoved {
  pub paths: Vec<String>,
  pub now: bool,
}

impl Event for DocumentsMoved {
  const NAME: &'static str = "documents-moved";
}

/// The documents a reload names, published.
pub struct DocumentsReload;

impl Route for DocumentsReload {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/documents/reload";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let paths: Vec<String> = body
      .get("paths")
      .and_then(Value::as_array)
      .map(|paths| paths.iter().filter_map(Value::as_str).map(str::to_owned).collect())
      .unwrap_or_default();

    if paths.is_empty() {
      return Err(Refused::new(StatusCode::BAD_REQUEST, "paths (strings) are required"));
    }

    let now = body.get("now").and_then(Value::as_bool).unwrap_or(false);

    call.events().publish(DocumentsMoved {
      paths: paths.clone(),
      now,
    });

    Ok(answer(StatusCode::OK, json!({ "paths": paths, "now": now })))
  }
}

crate::moved!(DocumentsReload);
