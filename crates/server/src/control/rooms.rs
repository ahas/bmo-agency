//! A room forgotten as its owner removes its registration (docs/api.md, 「Sessions」, forgetting a room): a
//! dry run answers what would go and what stays, table by table; the forgetting answers what went.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::Value;

use super::{Call, Refused, Route, answer, whole};

/// A room's rows forgotten, or counted.
pub struct RoomForget;

impl Route for RoomForget {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/rooms/forget";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let Some(room) = body.get("chatId").and_then(whole) else {
      return Err(Refused::new(StatusCode::BAD_REQUEST, "chatId (a whole number) is required"));
    };
    let dry_run = body.get("dryRun").and_then(Value::as_bool).unwrap_or(false);
    let forgotten = crate::rooms::forget(call.store(), room, dry_run).await?;

    Ok(answer(StatusCode::OK, serde_json::to_value(forgotten).map_err(Refused::failed)?))
  }
}

crate::moved!(RoomForget);
