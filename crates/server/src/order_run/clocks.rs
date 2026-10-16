//! The co-worker pool's own clocks (docs/api.md, 「Co-worker pool」, the clocks): every five seconds a
//! reservation's head place raised into a place another process gave back, and a native call whose caller
//! ended reclaimed; every ten minutes what a job's end left in herdr swept; and on the keepalive pass's
//! period the held reservations' places warmed. Each runs for as long as the process stands, never beside
//! itself.

use std::sync::Arc;
use std::time::Duration;

use policy::keepalive::KEEPALIVE;
use sea_orm::DatabaseConnection;

use super::dispatch::reclaim_native;
use crate::clock::every;
use crate::control::pool::promote_reservations;
use crate::keepalive::pool::Pool;
use crate::now_ms;
use crate::places::Ground;
use crate::queue::leftovers::sweep_everywhere;

/// How often the pool raises its reservations' head places and reclaims what an ended caller held.
pub const POOL_PASS: Duration = Duration::from_secs(5);

/// How often what a job's end left in herdr is swept beside the sweep each job's end asks for.
pub const LEFTOVERS_PASS: Duration = Duration::from_secs(10 * 60);

/// The pool's clock: a reservation's seat at the head of its line let into a place given back, and a native
/// call whose caller ended giving back the place a co-work branch may be waiting for.
pub async fn keep_pool(db: DatabaseConnection) {
  every("co-worker pool", Duration::ZERO, POOL_PASS, move || {
    let db = db.clone();

    async move {
      promote_reservations(&db, now_ms()).await.map_err(|refused| refused.error)?;
      reclaim_native(&db).await?;

      Ok(())
    }
  })
  .await;
}

/// The leftovers' clock: what a job's end left in herdr swept on this machine and every ready node.
pub async fn keep_leftovers(ground: Arc<Ground>) {
  every("leftover sweep", LEFTOVERS_PASS, LEFTOVERS_PASS, move || {
    let ground = ground.clone();

    async move { sweep_everywhere(&ground).await.map_err(|refusal| refusal.detail) }
  })
  .await;
}

/// The warming clock: the places the held reservations will use kept warm, first after the keepalive
/// pass's start wait and then once each of its periods, as far as the cache warmer's schedule lets pings
/// go.
pub async fn keep_warm(ground: Arc<Ground>) {
  let first = Duration::from_secs(KEEPALIVE.start_delay_seconds);
  let period = Duration::from_secs(KEEPALIVE.tick_seconds);

  every("pool warming", first, period, move || {
    let ground = ground.clone();

    async move { Pool { ground: &ground }.warm().await.map_err(|refusal| refusal.detail) }
  })
  .await;
}
