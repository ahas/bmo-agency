//! The co-worker pool's routes (docs/api.md, 「Co-worker pool」; docs/coworker-pool.md): a branch's seat or a
//! builder turn's place asked of the pool, let in at the head of its line, bound to the unit it launched
//! and given back with what it came to; the seats an owner reserves and gives back; and the standing
//! checkouts a branch's place is taken, laid, recorded and taken down in. The labels and keys of a
//! consumer's work arrive with each request, and agency reads none of its orders.

use std::path::Path;

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer, whole};
use crate::now_ms;
use crate::order_run::dispatch::{
  self, Asked, COWORK, Seats, Wanted, acquire, bind, enqueue_coworker, enqueue_turn, finish,
  held_reservations, release_reservation,
};
use crate::order_run::places::{self, Held};

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

// A pool step that did not go through, in the dispatch's own words.
fn refused(fault: dispatch::Fault) -> Refused {
  Refused::new(StatusCode::UNPROCESSABLE_ENTITY, fault)
}

fn text<'a>(body: &'a Value, field: &str) -> Result<&'a str, Refused> {
  body
    .get(field)
    .and_then(Value::as_str)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| bad(format!("{field} (string) is required")))
}

fn optional<'a>(body: &'a Value, field: &str) -> Option<&'a str> {
  body.get(field).and_then(Value::as_str).filter(|value| !value.is_empty())
}

fn wanted(body: &Value) -> Result<Wanted<'_>, Refused> {
  Ok(Wanted {
    role: text(body, "role")?,
    selection: text(body, "selection")?,
    requested_tier: optional(body, "tier"),
    provider: optional(body, "provider"),
  })
}

fn held(body: &Value) -> Vec<Held> {
  body
    .get("held")
    .and_then(Value::as_array)
    .map(|held| held.iter().filter_map(|one| serde_json::from_value(one.clone()).ok()).collect())
    .unwrap_or_default()
}

fn shown(value: impl serde::Serialize) -> Result<Value, Refused> {
  serde_json::to_value(value).map_err(Refused::failed)
}

/// A seat or a place asked of the pool: a branch's, as a child of its order, or a builder turn's, as the
/// worker itself on its checkout.
pub struct PoolEnqueue;

impl Route for PoolEnqueue {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/pool/enqueue";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let asked = Asked {
      request_key: text(&body, "requestKey")?.to_owned(),
      owner: text(&body, "owner")?.to_owned(),
      wanted: wanted(&body)?,
      generation: body.get("generation").and_then(whole).unwrap_or(1),
      yielding: body.get("yielding").and_then(Value::as_bool).unwrap_or(false),
      token: optional(&body, "token"),
      place_key: optional(&body, "placeKey"),
    };
    let request = match body.get("source").and_then(Value::as_str).unwrap_or(COWORK) {
      COWORK => enqueue_coworker(call.store(), &asked, async |_, _| Ok(())).await,
      "turn" => enqueue_turn(call.store(), &asked, async |_, _| Ok(())).await,
      other => return Err(bad(format!("a request's source is cowork or turn, not {other}"))),
    }
    .map_err(refused)?;

    Ok(answer(StatusCode::OK, shown(request)?))
  }
}

crate::moved!(PoolEnqueue);

async fn request_named(
  db: &sea_orm::DatabaseConnection,
  body: &Value,
) -> Result<crate::control::pool::Request, Refused> {
  let token = text(body, "token")?;

  dispatch::request(db, token)
    .await
    .map_err(refused)?
    .ok_or_else(|| Refused::new(StatusCode::NOT_FOUND, format!("no pool request holds the token {token}")))
}

/// A request let in where its turn has come, or none where it waits on.
pub struct PoolAcquire;

impl Route for PoolAcquire {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/pool/acquire";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let held = request_named(call.store(), &body).await?;
    let admitted = acquire(call.store(), &held).await.map_err(refused)?;

    Ok(answer(StatusCode::OK, json!({ "request": shown(admitted)? })))
  }
}

crate::moved!(PoolAcquire);

/// The unit a launch carries a lease into, written once.
pub struct PoolBind;

impl Route for PoolBind {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/pool/bind";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let held = request_named(call.store(), &body).await?;
    let bound = bind(call.store(), &held, text(&body, "childId")?).await.map_err(refused)?;

    Ok(answer(StatusCode::OK, shown(bound)?))
  }
}

crate::moved!(PoolBind);

/// A lease ended with what it came to, released or cancelled where it never ran.
pub struct PoolFinish;

impl Route for PoolFinish {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/pool/finish";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let cancel = body.get("cancel").and_then(Value::as_bool).unwrap_or(false);
    let held = finish(call.store(), text(&body, "token")?, text(&body, "outcome")?, cancel)
      .await
      .map_err(refused)?;

    Ok(answer(StatusCode::OK, json!({ "held": held })))
  }
}

crate::moved!(PoolFinish);

/// Seats of one tier on one provider held for an owner.
pub struct PoolReserve;

impl Route for PoolReserve {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/pool/reserve";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let seats = Seats {
      reservation_key: text(&body, "reservationKey")?.to_owned(),
      owner: text(&body, "owner")?.to_owned(),
      source: text(&body, "source")?,
      wanted: wanted(&body)?,
      count: body
        .get("count")
        .and_then(whole)
        .ok_or_else(|| bad("count (a whole number) is required"))?,
    };
    let reservation = dispatch::reserve(call.store(), &seats).await.map_err(refused)?;

    Ok(answer(StatusCode::OK, shown(reservation)?))
  }
}

crate::moved!(PoolReserve);

/// A reservation given back.
pub struct PoolReservationRelease;

impl Route for PoolReservationRelease {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/pool/reservations/release";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let held = release_reservation(call.store(), text(&body, "token")?, text(&body, "outcome")?)
      .await
      .map_err(refused)?;

    Ok(answer(StatusCode::OK, json!({ "held": held })))
  }
}

crate::moved!(PoolReservationRelease);

/// The reservations an owner holds, or every one held where none is named.
pub struct PoolReservations;

impl Route for PoolReservations {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/pool/reservations";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let owner = call.query("owner");
    let held = held_reservations(call.store(), owner.as_deref()).await.map_err(refused)?;

    Ok(answer(StatusCode::OK, json!({ "reservations": shown(held)? })))
  }
}

crate::moved!(PoolReservations);

/// A branch's place in its order's pool of standing checkouts, taken.
pub struct PlaceTake;

impl Route for PlaceTake {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/places/take";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let asked = places::Asked {
      cwd: text(&body, "cwd")?,
      selection: text(&body, "selection")?,
      role: text(&body, "role")?,
      tier: optional(&body, "tier"),
    };
    let slot = places::take_slot(call.ground(), &asked, &held(&body), now_ms())
      .await
      .map_err(refused)?;
    let cwd = places::slot_cwd(call.ground(), asked.cwd, slot).await;

    Ok(answer(StatusCode::OK, json!({ "slot": slot, "cwd": cwd.to_string_lossy() })))
  }
}

crate::moved!(PlaceTake);

/// A place laid to its order's checkout: its head and its open change.
pub struct PlacePrepare;

impl Route for PlacePrepare {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/places/prepare";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;

    places::prepare_slot(text(&body, "source")?, Path::new(text(&body, "cwd")?))
      .await
      .map_err(refused)?;

    Ok(answer(StatusCode::OK, json!({ "ok": true })))
  }
}

crate::moved!(PlacePrepare);

/// What a finished branch left in its place, recorded.
pub struct PlaceRemember;

impl Route for PlaceRemember {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/places/remember";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let slot = body
      .get("slot")
      .and_then(whole)
      .ok_or_else(|| bad("slot (a whole number) is required"))?;

    places::remember_slot(
      call.ground(),
      text(&body, "cwd")?,
      slot,
      text(&body, "role")?,
      optional(&body, "mode"),
      optional(&body, "model"),
      optional(&body, "effort"),
      optional(&body, "upstream"),
    )
    .await
    .map_err(refused)?;

    Ok(answer(StatusCode::OK, json!({ "ok": true })))
  }
}

crate::moved!(PlaceRemember);

/// What stands in herdr for the places an ended order worked in that no task still sits in, taken down.
pub struct PlacesRetire;

impl Route for PlacesRetire {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/places/retire";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let used: Vec<i64> = body
      .get("used")
      .and_then(Value::as_array)
      .map(|used| used.iter().filter_map(whole).collect())
      .unwrap_or_default();

    places::retire_places(call.ground(), text(&body, "cwd")?, &used, &held(&body)).await;

    Ok(answer(StatusCode::OK, json!({ "ok": true })))
  }
}

crate::moved!(PlacesRetire);
