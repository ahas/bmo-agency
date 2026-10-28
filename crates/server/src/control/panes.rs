//! The panes standing on this machine (docs/agent-control.md, 「A pane nobody uses」): the count, the
//! memory their process trees hold, and which names the agents table does not know.

use axum::http::Method;
use axum::response::Response;

use super::{Call, Route, StatusCode, answer};

/// Every standing pane, its memory and whether it is known or still at work.
pub struct Panes;

impl Route for Panes {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/panes";

  async fn answer(&self, call: Call) -> Result<Response, super::Refused> {
    let summary = crate::places::panes::summary(call.ground()).await;

    Ok(answer(StatusCode::OK, summary))
  }
}

crate::moved!(Panes);
