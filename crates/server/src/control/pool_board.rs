//! The live picture of the common co-worker pool a caller draws as tiles (docs/coworker-pool.md): a tile
//! for every place a tier has on a provider that is turned on and lends co-workers, and one for every
//! worker's own turn. Every tile's words are its request's own — its owner, its key, its source, its role
//! and the run it is bound to — so nothing of a consumer's work is read; a worker whose checkout's job hold
//! is held with no turn of the pool shows working with no words. It is read from the store on every ask,
//! and nothing here writes.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use policy::tiers::TIERS;
use serde_json::{Value, json};
use store::all;

use super::pool::{self, Request, coworker_providers};
use super::{Call, Refused, Route, answer};
use crate::now_ms;
use crate::order_run::dispatch::{COWORK, NATIVE};
use crate::places::locks;

/// What a tile's run is called and stopped by: the unit it is bound to, where it is bound to one.
fn run_of(request: &Request) -> (Value, Value) {
  match &request.child_id {
    Some(run) => (json!({ "run": run }), json!({ "kind": "unit", "id": run })),
    None => (Value::Null, Value::Null),
  }
}

/// The kind a request's work is shown as: a worker's own turn, a co-work branch, a native call, or any
/// other source as it names itself.
pub fn kind_of(request: &Request) -> String {
  if request.turn {
    "turn".to_owned()
  } else if request.source == COWORK {
    "cowork".to_owned()
  } else if request.source == NATIVE {
    "native".to_owned()
  } else {
    request.source.clone()
  }
}

/// A request's work as a tile shows it, in the request's own words.
pub fn work_of(request: &Request) -> Value {
  let since = if request.state == "waiting" { Some(request.enqueued_at) } else { request.leased_at };
  let (log, stop) = run_of(request);

  json!({
    "kind": kind_of(request),
    "role": request.role,
    "tier": request.tier,
    "provider": request.provider,
    "model": request.model,
    "effort": request.effort,
    "since": since,
    "title": request.owner,
    "detail": request.request_key,
    "workKey": request.work_key,
    "stop": stop,
    "log": log,
  })
}

fn tile_of(request: &Request) -> Value {
  json!({ "state": if request.state == "active" { "working" } else { "held" }, "work": work_of(request) })
}

fn idle() -> Value {
  json!({ "state": "idle", "work": null })
}

/// What holds each place of the pool right now, for a caller's tiles.
pub struct PoolBoard;

impl Route for PoolBoard {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/coworker-pool";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let db = call.store();
    let data_root = call.data_root();
    let now = now_ms();
    let live = pool::requests(db, &["waiting", "active", "reserved"]).await?;
    let limits = pool::limits(db).await?;
    let mut buckets = Vec::new();

    for provider in coworker_providers(db).await? {
      for tier in TIERS.coworker_tiers() {
        let capacity = limits
          .iter()
          .find(|one| one.provider == provider && one.tier == tier)
          .map_or(0, |one| one.capacity);
        let mine: Vec<&Request> = live
          .iter()
          .filter(|one| !one.turn && one.provider == provider && one.tier == tier)
          .collect();

        if capacity == 0 && mine.is_empty() {
          continue;
        }

        let mut tiles = Vec::new();
        let mut waiting = Vec::new();

        for request in &mine {
          if request.state == "waiting" {
            waiting.push(work_of(request));
          } else {
            tiles.push(tile_of(request));
          }
        }

        while (tiles.len() as i64) < capacity {
          tiles.push(idle());
        }

        buckets.push(json!({ "provider": provider, "tier": tier, "capacity": capacity, "tiles": tiles, "waiting": waiting }));
      }
    }

    let turns: Vec<&Request> = live.iter().filter(|one| one.turn).collect();
    let mut workers = Vec::new();
    let rows = all(
      db,
      r#"SELECT a.name, a.label, a.cwd FROM agents a WHERE a.kind = 'worker' AND a.resting_since IS NULL
         ORDER BY COALESCE((SELECT b.builtin FROM agents b WHERE b.name = a.support_of), a.builtin) DESC,
           COALESCE(a.support_of, a.name), COALESCE(a.support_slot, 1)"#,
      vec![],
    )
    .await?;

    for row in rows {
      let name: String = row.try_get("", "name")?;
      let cwd: Option<String> = row.try_get("", "cwd")?;
      let mine: Vec<&&Request> =
        turns.iter().filter(|one| one.place_key.is_some() && one.place_key == cwd).collect();
      let job_held = cwd.as_deref().is_some_and(|cwd| locks::held(&locks::job_hold_path(&data_root, cwd)));
      let tile = match mine.iter().find(|one| one.state == "active") {
        Some(request) => tile_of(request),
        None if job_held => json!({ "state": "working", "work": null }),
        None => idle(),
      };
      let waiting: Vec<Value> = mine
        .iter()
        .filter(|one| one.state == "waiting")
        .map(|request| work_of(request))
        .collect();

      workers.push(json!({
        "label": row.try_get::<Option<String>>("", "label")?.unwrap_or_else(|| name.clone()),
        "name": name,
        "tile": tile,
        "waiting": waiting,
      }));
    }

    Ok(answer(StatusCode::OK, json!({ "readAt": now, "buckets": buckets, "workers": workers })))
  }
}

crate::moved!(PoolBoard);
