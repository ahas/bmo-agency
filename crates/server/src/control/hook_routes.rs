//! A consumer's hooks registered across the boundary (docs/api.md, 「Hooks」): at a point named, an address
//! agency posts each ask to, under a token where one is given; taken away again, the point answers its
//! default. The points a consumer may register at are listed with whether one is registered.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::hooks::{Remote, wired, wired_named};

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

fn point_named(body: &Value) -> Result<&'static crate::hooks::Wired, Refused> {
  let name = body
    .get("point")
    .and_then(Value::as_str)
    .ok_or_else(|| bad("point (string) is required"))?;

  wired_named(name)
    .ok_or_else(|| Refused::new(StatusCode::NOT_FOUND, format!("no hook point is named {name}")))
}

/// A consumer's address registered at a point.
pub struct HookRegister;

impl Route for HookRegister {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/hooks";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let point = point_named(&body)?;
    let url = body.get("url").and_then(Value::as_str).ok_or_else(|| bad("url (string) is required"))?;

    reqwest::Url::parse(url).map_err(|why| bad(format!("{url} is not an address: {why}")))?;

    let token = body.get("token").and_then(Value::as_str).filter(|token| !token.is_empty());

    (point.register)(
      &call.ground().hooks,
      Remote {
        url: url.to_owned(),
        token: token.map(str::to_owned),
      },
    );

    Ok(answer(StatusCode::OK, json!({ "ok": true })))
  }
}

crate::moved!(HookRegister);

/// A point's registration taken away.
pub struct HookRemove;

impl Route for HookRemove {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/hooks/remove";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;

    (point_named(&body)?.unregister)(&call.ground().hooks);

    Ok(answer(StatusCode::OK, json!({ "ok": true })))
  }
}

crate::moved!(HookRemove);

/// The points a consumer may register at.
pub struct HookPoints;

impl Route for HookPoints {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/hooks";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let mut points: Vec<Value> = wired()
      .map(|point| json!({ "point": point.name, "registered": (point.registered)(&call.ground().hooks) }))
      .collect();

    points.sort_by(|a, b| a["point"].as_str().cmp(&b["point"].as_str()));

    Ok(answer(StatusCode::OK, json!({ "points": points })))
  }
}

crate::moved!(HookPoints);
