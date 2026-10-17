//! Whether a stand-in answers an agent while the rest closes Claude Code's road (docs/api.md, 「Models,
//! registry, tiers, roads and fallback」): the consumer that gates rooms on the rest asks it before it lets a
//! room's turn wait for Claude Code.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::json;

use super::{Call, Refused, Route, answer};
use crate::stand_in::answers;

/// Whether a stand-in answers the agent named now.
pub struct StandInRoute;

impl Route for StandInRoute {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/stand-in";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let Some(agent) = call
      .query("agent")
      .map(|agent| agent.trim().to_owned())
      .filter(|agent| !agent.is_empty())
    else {
      return Err(Refused::new(StatusCode::BAD_REQUEST, "agent is required"));
    };
    let ground = call.ground();
    let answers = answers(ground, ground.peers.cache_siren.as_ref(), &ground.machine(), &agent).await;

    Ok(answer(StatusCode::OK, json!({ "answers": answers })))
  }
}

crate::moved!(StandInRoute);
