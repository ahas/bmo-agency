//! The co-worker pool's reserved places warmed on agency's own clock (docs/coworker-pool.md, 「Reserving
//! places and keepalive」): the pool keeps alive the prefixes its held reservations will use, and nothing
//! else. Three parties answer three questions. A kind of place says where the prefixes a reservation will
//! use stand, when each was last written, whether work is in one now and what run reads it again. A
//! provider says how long its cache lives, whether a rest of the account holds its pings, and how its own
//! program sends one. The pool asks the keepalive schedule which places are due and records every ping, so
//! a restart neither forgets a reservation nor pings a place a second time inside its idle stretch. Whether
//! pings go at all is the owner's warming switch's (docs/keepalive-schedule.md).

pub mod branch;
pub mod native;
pub mod roads;
pub mod schedule;

use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use store::{one, run};

use self::schedule::Prefix;
use crate::control::claude_accounts::Judging;
use crate::control::pool::{self, Reservation};
use crate::door::record::Reply;
use crate::keepalive::switch;
use crate::lines::fault;
use crate::lines::worker::WorkerRun;
use crate::order_run::dispatch::held_reservations;
use crate::places::Ground;
use crate::places::home::Home;
use crate::rest;
use crate::{Refusal, now_ms};

/// A kind's or a place's answer being read.
pub type Boxed<'a, T> = Pin<Box<dyn Future<Output = Result<T, Refusal>> + Send + 'a>>;

/// The run a ping reads a place's prefix again with: the worker's line in a standing branch checkout,
/// whose own program a Claude Code ping is a turn of; the agent the place stands as and the home it stands
/// in, none where this machine names no herdr.
pub struct PlaceRun {
  pub line: Box<WorkerRun>,
  pub agent: String,
  pub home: Option<Box<Home>>,
}

/// One prefix a reservation will use, as its kind names it.
pub struct WarmPlace {
  /// Unique across kinds: the ping record and the one-ping-per-pass rule are keyed by it.
  pub key: String,
  pub provider: String,
  pub model: String,
  pub effort: Option<String>,
  /// The upstream that took the place's last turn, on a provider that routes to upstreams.
  pub upstream: Option<String>,
  /// When the prefix was last written or read, in milliseconds.
  pub last_write_at: i64,
  pub run: PlaceRun,
  pub work_key: String,
  pub name: String,
  pub title: String,
  /// Whose spend the ping is recorded as.
  pub usage_name: String,
  /// The place's own lock, asked for without waiting: a place a task is working in is that task's.
  pub lock: PathBuf,
  pub timeout: Duration,
}

/// One place as its kind keeps it: the prefix, whether work is in it now, and what an answered ping does
/// to its own clock.
pub trait Place: Send + Sync {
  fn warm(&self) -> &WarmPlace;

  /// Whether work is in the place right now; a ping never queues behind it.
  fn busy<'a>(&'a self, ground: &'a Ground) -> Boxed<'a, bool>;

  /// A ping answered at `at`, in milliseconds: the place's own clock starts again.
  fn warmed<'a>(&'a self, ground: &'a Ground, at: i64) -> Boxed<'a, ()>;
}

/// A kind of place, answering for the reservations of the sources it is brought by.
pub trait PlaceKind: Sync {
  fn answers(&self, source: &str) -> bool;

  /// Why a reservation of this kind has nothing that can be warmed, where that is so.
  fn refusal(&self) -> Option<&str> {
    None
  }

  fn places<'a>(
    &'a self,
    ground: &'a Ground,
    reservation: &'a Reservation,
  ) -> Boxed<'a, Vec<Box<dyn Place>>>;
}

// Every kind agency's pool holds, each answering for its own sources.
fn kinds() -> [&'static dyn PlaceKind; 2] {
  [&branch::BranchPlaces, &native::NativePlaces]
}

/// How one ping came out.
pub enum Pinged {
  Answered(Box<Reply>),
  /// Work holds the place, or its lock.
  Busy,
  Failed(String),
}

/// The pool's places of one pass.
pub struct Pool<'a> {
  pub ground: &'a Ground,
}

pub use cache_warmer::first_units;

impl Pool<'_> {
  // Whether pings go now: unless the owner's warming switch stands.
  async fn scheduled(&self) -> Result<bool, Refusal> {
    switch::pings_go(&self.ground.db).await.map_err(|why| fault(why.detail))
  }

  async fn lending(&self, provider: &str) -> Result<bool, Refusal> {
    Ok(
      pool::coworker_providers(&self.ground.db)
        .await
        .map_err(|refused| fault(refused.error))?
        .iter()
        .any(|one| one == provider),
    )
  }

  // What one ping to one place came to, written down. A place's last answer stays through later outcomes.
  async fn note(
    &self,
    reservation: &Reservation,
    place: &str,
    model: &str,
    outcome: &str,
  ) -> Result<(), Refusal> {
    let at = now_ms();
    let answered = (outcome == "answered").then_some(at);

    run(
      &self.ground.db,
      r#"INSERT INTO "coworker_pool_pings" ("reservation", "place", "provider", "model", "at", "outcome", "answered_at")
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT ("reservation", "place") DO UPDATE SET "provider" = excluded."provider", "model" = excluded."model",
           "at" = excluded."at", "outcome" = excluded."outcome",
           "answered_at" = COALESCE(excluded."answered_at", "coworker_pool_pings"."answered_at")"#,
      vec![
        reservation.token.clone().into(),
        place.into(),
        reservation.provider.clone().into(),
        model.into(),
        at.into(),
        outcome.into(),
        answered.into(),
      ],
    )
    .await?;

    Ok(())
  }

  // When a place last answered a ping, whichever reservation sent it.
  async fn last_answered(&self, place: &str) -> Result<Option<i64>, Refusal> {
    let row = one(
      &self.ground.db,
      r#"SELECT MAX("answered_at") AS "at" FROM "coworker_pool_pings" WHERE "place" = $1"#,
      vec![place.into()],
    )
    .await?;

    Ok(row.and_then(|row| row.try_get::<Option<i64>>("", "at").ok().flatten()))
  }

  /// Every held reservation's due places pinged, closest to going cold first, as many as the reservation
  /// holds seats; a place two reservations both want is pinged once a pass.
  pub async fn warm(&self) -> Result<(), Refusal> {
    let ground = self.ground;
    let db = &ground.db;
    let mut touched = HashSet::new();

    for reservation in held_reservations(db, None).await.map_err(fault)? {
      let seats = pool::seats(db, &reservation.token).await.map_err(|refused| fault(refused.error))?.len();

      if seats == 0 {
        continue;
      }

      let Some(kind) = kinds().into_iter().find(|kind| kind.answers(&reservation.source)) else {
        continue;
      };

      // Nothing is pinged, or written down, while the schedule holds the pings.
      if !self.scheduled().await? {
        continue;
      }

      if let Some(refusal) = kind.refusal() {
        self.note(&reservation, "*", &reservation.model, &format!("refused: {refusal}")).await?;
        continue;
      }

      // Only a turned-on provider that lends co-workers has places to warm.
      if !self.lending(&reservation.provider).await? {
        continue;
      }

      let Some(provider) = harness_providers::provider(&reservation.provider) else {
        continue;
      };

      if provider.rests_with_account() && rest::resting(Judging::of(ground), now_ms() / 1000).await? {
        continue;
      }

      let now = now_ms();
      let mut due = Vec::new();

      for place in kind.places(ground, &reservation).await? {
        let warm = place.warm();

        if touched.contains(&warm.key) || place.busy(ground).await? {
          continue;
        }

        let cache = match provider.place_cache(&warm.model, warm.upstream.as_deref()) {
          Ok(cache) => cache,
          Err(refusal) => {
            self.note(&reservation, &warm.key, &warm.model, &format!("refused: {refusal}")).await?;
            continue;
          }
        };
        let prefix = Prefix {
          last_write_at: warm.last_write_at,
          published_ms: cache.ttl_minutes * 60_000,
          idle_stretch_ms: cache.idle_minutes * 60_000,
          last_answered_at: self.last_answered(&warm.key).await?,
        };

        // Not yet worth a read, already gone — whoever writes a cold prefix pays the same — or pinged
        // within its stretch.
        if prefix.due(now) {
          due.push(place);
        }
      }

      due.sort_by_key(|place| place.warm().last_write_at);

      for place in due.into_iter().take(seats) {
        let warm = place.warm();

        touched.insert(warm.key.clone());

        // The schedule or the provider may have moved while an earlier ping was out.
        if !self.scheduled().await? || !self.lending(&reservation.provider).await? {
          return Ok(());
        }

        let outcome = match roads::ping(ground, warm).await {
          Ok(Pinged::Answered(_)) => {
            place.warmed(ground, now_ms()).await?;
            "answered".to_owned()
          }
          Ok(Pinged::Busy) => "busy".to_owned(),
          Ok(Pinged::Failed(reason)) => format!("failed: {}", first_units(&reason, 400)),
          Err(thrown) => format!("failed: {}", first_units(&first_units(&thrown.detail, 300), 400)),
        };

        self.note(&reservation, &warm.key, &warm.model, &outcome).await?;
      }
    }

    Ok(())
  }
}
