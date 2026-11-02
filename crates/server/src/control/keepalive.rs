//! Agency's own switches (docs/api.md, 「settings」): the downgrade return, read and changed. No switch warms
//! or stops everything at once: a supervisor's warming is its own row's (docs/keepalive-schedule.md).

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};

/// Agency's own switches.
pub struct SettingsRead;

impl Route for SettingsRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/settings";

  async fn answer(&self, _call: Call) -> Result<Response, Refused> {
    Ok(answer(StatusCode::OK, json!({ "downgradeReturn": Value::Null })))
  }
}

crate::moved!(SettingsRead);

/// Agency's own switches changed as the body names them.
pub struct SettingsWrite;

impl Route for SettingsWrite {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/settings";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    call.body().await?;

    Ok(answer(StatusCode::OK, json!({ "downgradeReturn": Value::Null })))
  }
}

crate::moved!(SettingsWrite);
