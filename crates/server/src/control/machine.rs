//! This machine's room across the boundary (docs/api.md, 「Workers」, the machine's room): what holds a
//! job's launch now, for the launcher to ask before it launches, and the launcher's notice that it held
//! launches, which agency keeps until the room returns.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer, whole};
use crate::watches::machine::{LaunchHeld, room, tell_launch_held};

/// What holds a job's launch on this machine now, with the counts the job-count limits are judged on.
pub struct MachineRoom;

impl Route for MachineRoom {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/machine/room";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let ground = call.ground();
    let room = room(&ground.db, ground.peers.node.as_ref()).await?;

    Ok(answer(StatusCode::OK, json!(room)))
  }
}

crate::moved!(MachineRoom);

/// The launcher's notice that launches are held and on what, or its withdrawal where it holds nothing.
pub struct LaunchHeldTold;

impl Route for LaunchHeldTold {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/machine/launch-held";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let bad = |error: &str| Refused::new(StatusCode::BAD_REQUEST, error);
    let at = body.get("at").and_then(whole).ok_or_else(|| bad("at (a whole number) is required"))?;
    let held = body
      .get("held")
      .and_then(Value::as_array)
      .ok_or_else(|| bad("held (a list of strings) is required"))?
      .iter()
      .map(|reason| reason.as_str().map(str::to_owned))
      .collect::<Option<Vec<_>>>()
      .ok_or_else(|| bad("held (a list of strings) is required"))?;

    tell_launch_held(call.store(), &LaunchHeld { at, held }).await?;

    Ok(answer(StatusCode::OK, json!({ "ok": true })))
  }
}

crate::moved!(LaunchHeldTold);
