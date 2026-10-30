#![allow(
  dead_code,
  unused_imports,
  unused_variables,
  clippy::needless_lifetimes,
  clippy::let_and_return,
  clippy::unnecessary_to_owned
)]
//! The co-worker pool's routes (docs/api.md, 「Co-worker pool」; docs/coworker-pool.md): a branch's seat or a
//! builder turn's place asked of the pool, let in at the head of its line, bound to the unit it launched
//! and given back with what it came to; the seats an owner reserves and gives back; and the standing
//! checkouts a branch's place is taken, laid, recorded and taken down in. The labels and keys of a
//! consumer's work arrive with each request, and agency reads none of its orders.

use std::path::Path;

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bmo_chat::{ErrorCode, WireError};
use serde_json::{Value, json};
use store::run;

use super::{Call, Refused, Route, answer, pool, whole};
use crate::now_ms;
use crate::order_run::dispatch::{
  self, Asked, COWORK, Seats, Wanted, acquire, bind, enqueue_seat, enqueue_turn, finish, held_reservations,
  release_reservation,
};
use crate::order_run::places::{self, Held};
use crate::places::dev;
use crate::watches::links::checkout_shadow;

// The source a builder turn's place is asked under.
const TURN: &str = "turn";

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

// A place named in a path: a checkout path is one segment of it, percent-encoded.
fn place_named(call: &Call) -> String {
  percent_encoding::percent_decode_str(call.param("placeId").unwrap_or_default())
    .decode_utf8_lossy()
    .into_owned()
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
    work_key: optional(body, "workKey"),
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
  const PATH: &'static str = "/seat-requests";

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
    // A builder turn's place is the one source of its own; any other label is a branch's seat, kept as sent.
    let request = match body.get("source").and_then(Value::as_str).unwrap_or(COWORK) {
      TURN => enqueue_turn(call.ground(), &asked, async |_, _| Ok(())).await,
      source => enqueue_seat(call.store(), &asked, source, async |_, _| Ok(())).await,
    }
    .map_err(refused)?;

    Ok(answer(StatusCode::CREATED, shown(request)?))
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
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/seat-requests/{token}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let token = call.param("token").unwrap_or_default().to_owned();
    let held = dispatch::request(call.store(), &token).await.map_err(refused)?.ok_or_else(|| {
      Refused::new(StatusCode::NOT_FOUND, format!("no pool request holds the token {token}"))
    })?;
    let move_to = body.get("state").or_else(|| body.get("action")).and_then(Value::as_str);

    match move_to {
      Some("bound") => {
        let child = text(&body, "childId")?;
        let bound = bind(call.store(), &held, child).await.map_err(refused)?;
        Ok(answer(StatusCode::OK, shown(bound)?))
      }
      Some("finished") | Some("finish") => {
        let outcome = body.get("outcome").and_then(Value::as_str).unwrap_or_default();
        let cancel = body.get("cancel").and_then(Value::as_bool).unwrap_or(false);
        let held = finish(call.store(), &token, outcome, cancel).await.map_err(refused)?;
        Ok(answer(StatusCode::OK, json!({ "held": held })))
      }
      _ => {
        let admitted = acquire(call.store(), &held).await.map_err(refused)?;
        Ok(answer(StatusCode::OK, json!({ "request": shown(admitted)? })))
      }
    }
  }
}

crate::moved!(PoolAcquire);

/// Seats of one tier on one provider held for an owner.
pub struct PoolReserve;

impl Route for PoolReserve {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/reservations";

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

    Ok(answer(StatusCode::CREATED, shown(reservation)?))
  }
}

crate::moved!(PoolReserve);

/// A reservation given back.
pub struct PoolReservationRelease;

impl Route for PoolReservationRelease {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/reservations/{token}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let token = call.param("token").unwrap_or_default().to_owned();
    let held = release_reservation(
      call.store(),
      &token,
      body.get("outcome").and_then(Value::as_str).unwrap_or_default(),
    )
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
  const PATH: &'static str = "/reservations";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let owner = call.query("owner");
    let held = held_reservations(call.store(), owner.as_deref()).await.map_err(refused)?;

    Ok(answer(StatusCode::OK, json!({ "items": shown(held)?, "more": false })))
  }
}

crate::moved!(PoolReservations);

// The request states that are still open: asked, or let in.
const OPEN: [&str; 2] = ["waiting", "active"];

/// The request a token holds, whatever its state.
pub struct PoolRequestRead;

impl Route for PoolRequestRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/seat-requests/{token}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let token = call.param("token").unwrap_or_default();

    Ok(match dispatch::request(call.store(), token).await.map_err(refused)? {
      Some(request) => answer(StatusCode::OK, shown(request)?),
      None => WireError::new(ErrorCode::NotFound, format!("no pool request holds the token {token}"))
        .into_response(),
    })
  }
}

crate::moved!(PoolRequestRead);

/// The requests still open, in the order they came, only an owner's where one is named.
pub struct PoolOpenRequests;

impl Route for PoolOpenRequests {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/seat-requests";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let owner = call.query("owner");
    let open: Vec<_> = dispatch::requests(call.store(), &OPEN)
      .await
      .map_err(refused)?
      .into_iter()
      .filter(|request| owner.as_ref().is_none_or(|owner| &request.owner == owner))
      .collect();

    Ok(answer(StatusCode::OK, json!({ "items": shown(open)?, "more": false })))
  }
}

crate::moved!(PoolOpenRequests);

/// Every role the pool knows, with the tier its assignment names.
pub struct PoolRoles;

impl Route for PoolRoles {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/roles";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let db = call.store();
    let mut listed = vec![];

    for role in pool::roles(db, false).await? {
      let tier = dispatch::role_tier(db, &role.name).await.unwrap_or_default();

      listed.push(json!({
        "name": role.name,
        "description": role.description,
        "enabled": role.enabled,
        "pooled": role.pooled,
        "tier": tier,
      }));
    }

    Ok(answer(StatusCode::OK, json!({ "items": listed, "more": false })))
  }
}

crate::moved!(PoolRoles);

/// How many children of one co-worker tier may be alive on one provider, written, and the places it frees
/// let in at once (docs/coworker-pool.md, 「Concurrency limits」). A capacity that is no whole number is
/// handed on as one below nothing, so the refusal is the one the limit's own range gives.
pub struct PoolLimitChange;

impl Route for PoolLimitChange {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/pool/limits";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let provider = optional(&body, "provider").unwrap_or_default();
    let tier = optional(&body, "tier").unwrap_or_default();
    let capacity = body.get("capacity").and_then(whole).unwrap_or(-1);

    match pool::set_limit(call.store(), provider, tier, capacity, now_ms()).await? {
      Ok(()) => Ok(answer(
        StatusCode::OK,
        json!({ "provider": provider, "tier": tier, "capacity": capacity }),
      )),
      Err(refusal) => Err(Refused::new(StatusCode::UNPROCESSABLE_ENTITY, refusal.0)),
    }
  }
}

crate::moved!(PoolLimitChange);

/// What a place's line was last written with, the place keeping the latest moment.
pub struct PlaceWrite;

impl Route for PlaceWrite {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/pool/places/{place}/write";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let place = call.param("place").unwrap_or_default().to_owned();
    let body = call.body().await?;

    if place.is_empty() {
      return Err(bad("a place's key is required"));
    }

    pool::place_written(
      call.store(),
      &place,
      text(&body, "provider")?,
      text(&body, "model")?,
      optional(&body, "effort"),
      optional(&body, "upstream"),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

/// A branch's place in its order's pool of standing checkouts, taken.
pub struct PlaceTake;

impl Route for PlaceTake {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/places";

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
    let path = cwd.to_string_lossy().into_owned();

    Ok(answer(
      StatusCode::CREATED,
      json!({ "id": path, "path": path, "nodeName": Value::Null, "slot": slot, "role": body.get("role").cloned(), "state": "taken", "lastWrite": Value::Null }),
    ))
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

/// The places the agency holds, by node and checkout.
pub struct PlaceList;

impl Route for PlaceList {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/places";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let node = call.query("nodeName").filter(|node| !node.trim().is_empty());
    let checkout = call.query("checkout").filter(|checkout| !checkout.trim().is_empty());

    // A checkout on a node is held here as the shadow the node's link keeps of it.
    if let Some(node) = node.as_deref() {
      let Some(checkout) = checkout.as_deref() else {
        return Err(bad("checkout is required with nodeName"));
      };
      let path = checkout_shadow(&call.ground().roots.data, node, checkout)
        .to_string_lossy()
        .into_owned();

      return Ok(answer(
        StatusCode::OK,
        json!({ "items": [place_at(&path, Some(node), None, None, Value::Null)], "more": false }),
      ));
    }

    let pool = match checkout.as_deref() {
      Some(checkout) => Some(crate::places::branch::pool_of(checkout).await),
      None => None,
    };
    let items = held_places(call.ground(), pool.as_deref()).await?;

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": false })))
  }
}

crate::moved!(PlaceList);

/// One place by its key.
pub struct ReadPlace;

impl Route for ReadPlace {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/places/{placeId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let place = place_named(&call);

    if let Some(one) = held_place(call.ground(), &place).await? {
      return Ok(answer(StatusCode::OK, one));
    }

    let row = store::one(
      call.store(),
      r#"SELECT "place", "provider", "model", "effort", "upstream", "written_at"
         FROM "coworker_pool_place_writes" WHERE "place" = $1"#,
      vec![place.clone().into()],
    )
    .await?;

    let Some(row) = row else {
      return Err(Refused::new(StatusCode::NOT_FOUND, format!("no place is named {place}")));
    };

    Ok(answer(
      StatusCode::OK,
      json!({
        "id": place.clone(),
        "path": place,
        "nodeName": Value::Null,
        "slot": Value::Null,
        "role": Value::Null,
        "state": "remembered",
        "lastWrite": {
          "provider": row.try_get::<String>("", "provider")?,
          "model": row.try_get::<String>("", "model")?,
          "effort": row.try_get::<Option<String>>("", "effort")?,
          "upstream": row.try_get::<Option<String>>("", "upstream")?,
          "writtenAt": row.try_get::<i64>("", "written_at")?,
        },
      }),
    ))
  }
}

crate::moved!(ReadPlace);

/// One place as the boundary answers it.
fn place_at(
  path: &str,
  node: Option<&str>,
  slot: Option<i64>,
  role: Option<&str>,
  last_write: Value,
) -> Value {
  json!({
    "id": path,
    "path": path,
    "nodeName": node,
    "slot": slot,
    "role": role,
    "state": if last_write.is_null() { "prepared" } else { "remembered" },
    "lastWrite": last_write,
  })
}

// A standing place as its row and its write leave it: taken where a pool request holds it, remembered
// where a branch last answered in it, prepared where only its folder stands.
fn standing_place(
  path: &str,
  slot: i64,
  role: Option<&str>,
  row: Option<&(String, String, String, i64)>,
  taken: bool,
  write: Option<&Value>,
) -> Value {
  let last_write = write.cloned().or_else(|| {
    row.map(|(mode, model, upstream, at)| {
      json!({
        "provider": mode,
        "model": model,
        "effort": Value::Null,
        "upstream": if upstream.is_empty() { Value::Null } else { json!(upstream) },
        "writtenAt": at * 1000,
      })
    })
  });
  let mut place = place_at(path, None, Some(slot), role, last_write.unwrap_or(Value::Null));

  place["state"] = json!(if taken {
    "taken"
  } else if row.is_some() {
    "remembered"
  } else {
    "prepared"
  });

  place
}

// Every place the agency holds under the data root, by pool and slot, with the row that last answered
// in it and the write it last took.
async fn held_places(ground: &crate::places::Ground, only: Option<&str>) -> Result<Vec<Value>, Refused> {
  let data = &ground.roots.data;
  let rows = store::all(
    &ground.db,
    r#"SELECT "pool", "slot", "mode", "model", "role", "upstream", "last_reply_at"
       FROM "branch_agents" ORDER BY "last_reply_at" DESC"#,
    vec![],
  )
  .await?;
  let mut written: std::collections::HashMap<String, Value> = std::collections::HashMap::new();

  for row in store::all(
    &ground.db,
    r#"SELECT "place", "provider", "model", "effort", "upstream", "written_at"
       FROM "coworker_pool_place_writes""#,
    vec![],
  )
  .await?
  {
    written.insert(
      row.try_get("", "place")?,
      json!({
        "provider": row.try_get::<String>("", "provider")?,
        "model": row.try_get::<String>("", "model")?,
        "effort": row.try_get::<Option<String>>("", "effort")?,
        "upstream": row.try_get::<Option<String>>("", "upstream")?,
        "writtenAt": row.try_get::<i64>("", "written_at")?,
      }),
    );
  }

  let mut taken = std::collections::HashSet::new();

  for row in store::all(
    &ground.db,
    r#"SELECT DISTINCT "place_key" FROM "coworker_pool_requests"
       WHERE "state" = 'active' AND "place_key" IS NOT NULL"#,
    vec![],
  )
  .await?
  {
    taken.insert(row.try_get::<String>("", "place_key")?);
  }

  let mut newest: std::collections::HashMap<(String, i64), (String, String, String, i64)> =
    std::collections::HashMap::new();
  let mut roles: std::collections::HashMap<(String, i64), String> = std::collections::HashMap::new();

  for row in &rows {
    let pool: String = row.try_get("", "pool")?;
    let slot: i64 = row.try_get("", "slot")?;
    let entry = (
      row.try_get::<String>("", "mode")?,
      row.try_get::<String>("", "model")?,
      row.try_get::<String>("", "upstream")?,
      row.try_get::<i64>("", "last_reply_at")?,
    );

    roles
      .entry((pool.clone(), slot))
      .or_insert_with(|| row.try_get("", "role").unwrap_or_default());
    newest.entry((pool, slot)).or_insert(entry);
  }

  let mut pools: Vec<String> = std::fs::read_dir(crate::places::branch::slot_root(data))
    .into_iter()
    .flatten()
    .flatten()
    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
    .filter_map(|entry| entry.file_name().into_string().ok())
    .collect();

  for (pool, _) in roles.keys() {
    if !pools.contains(pool) {
      pools.push(pool.clone());
    }
  }

  pools.sort();
  pools.retain(|pool| only.is_none_or(|only| only == pool));

  let mut items = vec![];

  for pool in pools {
    let mut slots = crate::places::branch::pool_slots(data, &pool);

    for (_, slot) in roles.keys().filter(|(held, _)| *held == pool) {
      if !slots.contains(slot) {
        slots.push(*slot);
      }
    }

    slots.sort_unstable();

    for slot in slots {
      let path = crate::places::branch::slot_dir(data, &pool, slot).to_string_lossy().into_owned();
      let row = newest.get(&(pool.clone(), slot));
      let role = roles.get(&(pool.clone(), slot)).map(String::as_str);

      items.push(standing_place(&path, slot, role, row, taken.contains(&path), written.get(&path)));
    }
  }

  Ok(items)
}

// One place by its path, where the path is one under the data root.
async fn held_place(ground: &crate::places::Ground, place: &str) -> Result<Option<Value>, Refused> {
  if let Some((pool, slot)) = crate::places::branch::place_of(&ground.roots.data, place) {
    let found = held_places(ground, Some(&pool))
      .await?
      .into_iter()
      .find(|one| one["slot"].as_i64() == Some(slot));

    return Ok(found);
  }

  // A checkout on a node is held as the shadow under the node's own directory.
  let shadow = std::path::Path::new(&ground.roots.data).join("nodes");
  let within = std::path::Path::new(place).strip_prefix(&shadow).ok();
  let mut parts = within.into_iter().flat_map(|rest| rest.components());
  let node = parts.next().and_then(|node| node.as_os_str().to_str()).map(str::to_owned);
  let kind = parts.next().and_then(|kind| kind.as_os_str().to_str()).map(str::to_owned);

  Ok(match (node, kind.as_deref()) {
    (Some(node), Some("shadow")) => Some(place_at(place, Some(&node), None, None, Value::Null)),
    _ => None,
  })
}

/// A place laid, remembered or retired, or its last write recorded.
pub struct ChangePlace;

impl Route for ChangePlace {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/places/{placeId}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let place = place_named(&call);
    let body = call.body().await?;

    match body.get("state").and_then(Value::as_str) {
      Some("prepared") => {
        places::prepare_slot(text(&body, "source")?, Path::new(text(&body, "cwd")?))
          .await
          .map_err(refused)?;
      }
      Some("remembered") => {
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
      }
      Some("retired") => {
        let used: Vec<i64> = body
          .get("used")
          .and_then(Value::as_array)
          .map(|used| used.iter().filter_map(whole).collect())
          .unwrap_or_default();
        places::retire_places(call.ground(), text(&body, "cwd")?, &used, &held(&body)).await;
      }
      _ => {
        pool::place_written(
          call.store(),
          &place,
          text(&body, "provider")?,
          text(&body, "model")?,
          optional(&body, "effort"),
          optional(&body, "upstream"),
        )
        .await?;
      }
    }

    let state = body.get("state").and_then(Value::as_str).unwrap_or("taken");

    Ok(answer(
      StatusCode::OK,
      json!({ "id": place, "path": place, "nodeName": Value::Null, "slot": Value::Null, "role": Value::Null, "state": state, "lastWrite": body.get("lastWrite").cloned().unwrap_or(Value::Null) }),
    ))
  }
}

crate::moved!(ChangePlace);

/// A worker's workbench folder, with every item's scratch folder standing under it now, in item order.
pub struct Workbench;

impl Route for Workbench {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/workers/{workerName}/workbench";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let worker = call
      .param("workerName")
      .filter(|worker| !worker.is_empty())
      .ok_or_else(|| bad("a worker is required"))?;
    let root = dev::workbench(call.ground(), worker);
    let mut scratches: Vec<(i64, String)> = std::fs::read_dir(dev::item_scratches(call.ground(), worker))
      .into_iter()
      .flatten()
      .flatten()
      .filter(|entry| entry.path().is_dir())
      .filter_map(|entry| {
        let item = entry.file_name().to_str()?.parse::<i64>().ok()?;

        Some((item, entry.path().to_string_lossy().into_owned()))
      })
      .collect();

    scratches.sort();

    let scratches: Vec<String> = scratches.into_iter().map(|(_, path)| path).collect();

    Ok(answer(
      StatusCode::OK,
      json!({ "root": root.to_string_lossy(), "scratches": scratches }),
    ))
  }
}

crate::moved!(Workbench);
