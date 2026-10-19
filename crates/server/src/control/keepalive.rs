//! The owner's warming switch (docs/api.md, 「Co-worker pool」): read, and thrown. While it stands every
//! keepalive ping of the deployment's stops (docs/keepalive-schedule.md, 「Whether pings go」).

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::keepalive::switch;

/// Whether the warming switch stands.
pub struct KeepaliveRead;

impl Route for KeepaliveRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/keepalive";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let stopped = switch::stopped(call.store()).await?;

    Ok(answer(StatusCode::OK, json!({ "stopped": stopped })))
  }
}

crate::moved!(KeepaliveRead);

/// The warming switch thrown to the `stopped` the body names.
pub struct KeepaliveWrite;

impl Route for KeepaliveWrite {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/keepalive";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let Some(stopped) = body.get("stopped").and_then(Value::as_bool) else {
      return Err(Refused::new(StatusCode::BAD_REQUEST, "stopped (a boolean) is required"));
    };

    switch::set_stopped(call.store(), stopped).await?;

    let stopped = switch::stopped(call.store()).await?;

    Ok(answer(StatusCode::OK, json!({ "stopped": stopped })))
  }
}

crate::moved!(KeepaliveWrite);
