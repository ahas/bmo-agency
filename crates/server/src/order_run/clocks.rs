//! The co-worker pool's own clocks (docs/api.md, 「Co-worker pool」, the clocks): every five seconds a
//! reservation's head place raised into a place another process gave back, and a native call whose caller
//! ended reclaimed; every minute from the start a lease whose launched child ended given back; every ten
//! minutes what a job's end left in herdr swept; and on the keepalive pass's period the held reservations'
//! places warmed. Beside them, each reader of the accounts' usage windows
//! passes on the cadence it declares (docs/usage-windows.md). Each runs for as long as the process stands,
//! never beside itself.

use std::sync::Arc;
use std::time::Duration;

use policy::keepalive::KEEPALIVE;
use sea_orm::DatabaseConnection;

use super::dispatch::{reclaim_bound, reclaim_native};
use crate::clock::every;
use crate::control::pool::promote_reservations;
use crate::keepalive::pool::Pool;
use crate::now_ms;
use crate::places::Ground;
use crate::queue::leftovers::sweep_everywhere;
use crate::queue::running::Running;

/// How often the pool raises its reservations' head places and reclaims what an ended caller held.
pub const POOL_PASS: Duration = Duration::from_secs(5);

/// How often the leases bound to a launched child are held to whether that child still stands.
pub const BOUND_PASS: Duration = Duration::from_secs(60);

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

/// The bound leases' clock, from the process's start: a lease whose launched child has ended — its unit,
/// its pane and every open execution naming it gone — given back, so a turn whose release never came does
/// not hold its place for good.
pub async fn keep_bound(db: DatabaseConnection, running: Running) {
  every("co-worker pool bound leases", Duration::ZERO, BOUND_PASS, move || {
    let db = db.clone();
    let running = running.clone();

    async move {
      let released = reclaim_bound(&db, &running).await?;

      if released > 0 {
        println!("co-worker pool: {released} lease(s) whose launched child ended given back");
      }

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

/// The usage windows' clocks: each reader the keeping knows passing at once and then on its own cadence,
/// every reading it takes replacing its slot's last.
pub async fn keep_usage_windows(ground: Arc<Ground>) {
  let clocks = ground.windows.readers().iter().cloned().map(|reader| {
    let ground = ground.clone();

    async move {
      every("usage windows", Duration::ZERO, reader.cadence(), move || {
        let ground = ground.clone();
        let reader = reader.clone();

        async move {
          reader.pass(&ground.windows, now_ms()).await;

          Ok(())
        }
      })
      .await;
    }
  });

  futures_util::future::join_all(clocks).await;
}

/// The warming clock: the places the held reservations will use kept warm, first after the keepalive
/// pass's start wait and then once each of its periods, as far as the keepalive schedule lets pings go.
pub async fn keep_warm(ground: Arc<Ground>) {
  let first = Duration::from_secs(KEEPALIVE.start_delay_seconds);
  let period = Duration::from_secs(KEEPALIVE.tick_seconds);

  every("pool warming", first, period, move || {
    let ground = ground.clone();

    async move { Pool { ground: &ground }.warm().await.map_err(|refusal| refusal.detail) }
  })
  .await;
}
