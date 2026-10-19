//! The common co-worker pool as the routes read it, and the limits the owner sets on it
//! (docs/model-tiers.md, 「Common co-worker capacity」): its roles, every tier's limit on every provider,
//! the requests and reservations standing in it, and a limit's change letting in the seats it frees.

use harness_providers::ARCHIVE_PROVIDERS;
use policy::research::RESEARCH;
use policy::tiers::TIERS;
use sea_orm::{ConnectionTrait, DatabaseConnection, QueryResult, TransactionTrait};
use serde::Serialize;

use super::Refused;
use super::harness::{named, provider_order};
use store::{all, one, run};

/// One co-worker role as the pool holds it.
#[derive(Clone, Debug)]
pub struct Role {
  pub name: String,
  pub description: String,
  pub enabled: bool,
  pub pooled: bool,
}

/// One tier's limit on one provider.
#[derive(Clone, Debug)]
pub struct Limit {
  pub provider: String,
  pub tier: String,
  pub capacity: i64,
}

/// One request or seat standing in the pool.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
  pub sequence: i64,
  pub token: String,
  pub request_key: String,
  pub owner: String,
  pub source: String,
  pub role: String,
  pub work_key: String,
  pub generation: i64,
  pub ended_at: Option<i64>,
  pub outcome: Option<String>,
  pub reservation: Option<String>,
  pub provider: String,
  pub model: String,
  pub effort: Option<String>,
  pub tier: String,
  pub capacity: i64,
  pub state: String,
  pub enqueued_at: i64,
  pub leased_at: Option<i64>,
  pub child_id: Option<String>,
  pub yielding: bool,
  pub place_key: Option<String>,
  pub turn: bool,
}

/// One owner's reservation.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reservation {
  pub token: String,
  pub reservation_key: String,
  pub state: String,
  pub owner: String,
  pub source: String,
  pub role: String,
  /// The work key the reservation was resolved under, which its seats are kept under.
  pub work_key: String,
  pub provider: String,
  pub model: String,
  /// None where the mapping it froze names no effort.
  pub effort: Option<String>,
  pub tier: String,
  pub wanted: i64,
  pub created_at: i64,
}

/// The last ping one reservation sent to one place.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ping {
  pub place: String,
  pub at: i64,
  pub outcome: String,
  pub answered_at: Option<i64>,
}

/// One provider's co-worker tier, with what its waiting, active and reserved children hold.
#[derive(Clone, Debug)]
pub struct TierStatus {
  pub provider: String,
  pub tier: String,
  pub capacity: i64,
  pub default_capacity: i64,
  pub snapshot_capacities: Vec<i64>,
  pub snapshot_models: Vec<(String, Option<String>)>,
  pub active: i64,
  pub reserved: i64,
  pub waiting: i64,
  pub yielding_active: i64,
  pub yielding_waiting: i64,
}

/// A refusal of a pool setting, said as the pool's own configuration error is.
pub struct PoolRefusal(pub String);

fn effort_of(stored: String) -> Option<String> {
  (!stored.is_empty()).then_some(stored)
}

pub(crate) fn request_of(row: &QueryResult) -> Result<Request, Refused> {
  Ok(Request {
    sequence: row.try_get("", "sequence")?,
    token: row.try_get("", "token")?,
    request_key: row.try_get("", "request_key")?,
    owner: row.try_get("", "owner")?,
    source: row.try_get("", "source")?,
    role: row.try_get("", "role")?,
    work_key: row.try_get("", "work_key")?,
    generation: row.try_get("", "generation")?,
    ended_at: row.try_get("", "ended_at")?,
    outcome: row.try_get("", "outcome")?,
    reservation: row.try_get("", "reservation")?,
    provider: row.try_get("", "provider")?,
    model: row.try_get("", "model")?,
    effort: effort_of(row.try_get("", "effort")?),
    tier: row.try_get("", "tier")?,
    capacity: row.try_get("", "capacity")?,
    state: row.try_get("", "state")?,
    enqueued_at: row.try_get("", "enqueued_at")?,
    leased_at: row.try_get("", "leased_at")?,
    child_id: row.try_get("", "child_id")?,
    yielding: row.try_get::<String>("", "class")? == "yielding",
    place_key: row.try_get("", "place_key")?,
    turn: row.try_get::<i64>("", "turn")? == 1,
  })
}

pub async fn roles(db: &DatabaseConnection, enabled_pooled_only: bool) -> Result<Vec<Role>, Refused> {
  let filter = if enabled_pooled_only { " WHERE enabled = 1 AND pooled = 1" } else { "" };

  all(
    db,
    &format!("SELECT name, description, enabled, pooled FROM coworker_pool_roles{filter} ORDER BY name"),
    vec![],
  )
  .await?
  .iter()
  .map(|row| {
    Ok(Role {
      name: row.try_get("", "name")?,
      description: row.try_get("", "description")?,
      enabled: row.try_get::<i64>("", "enabled")? == 1,
      pooled: row.try_get::<i64>("", "pooled")? == 1,
    })
  })
  .collect()
}

pub async fn role(db: &DatabaseConnection, name: &str) -> Result<Option<Role>, Refused> {
  Ok(roles(db, false).await?.into_iter().find(|one| one.name == name))
}

fn position(list: &[&str], value: &str) -> usize {
  list.iter().position(|one| *one == value).unwrap_or(usize::MAX)
}

/// Every tier's limit, by provider and then by the ladder.
pub async fn limits(db: &DatabaseConnection) -> Result<Vec<Limit>, Refused> {
  let mut limits: Vec<Limit> =
    all(db, "SELECT provider, tier, capacity FROM coworker_pool_tier_limits", vec![])
      .await?
      .iter()
      .map(|row| {
        Ok(Limit {
          provider: row.try_get("", "provider")?,
          tier: row.try_get("", "tier")?,
          capacity: row.try_get("", "capacity")?,
        })
      })
      .collect::<Result<_, Refused>>()?;
  let ladder: Vec<&str> = TIERS.ladder.iter().map(String::as_str).collect();

  limits.sort_by_key(|one| (position(&ARCHIVE_PROVIDERS, &one.provider), position(&ladder, &one.tier)));

  Ok(limits)
}

/// The requests standing in the given states, in the order they came.
pub async fn requests(db: &DatabaseConnection, states: &[&str]) -> Result<Vec<Request>, Refused> {
  let marks: Vec<String> = (1..=states.len()).map(|at| format!("${at}")).collect();
  let sql = format!(
    "SELECT * FROM coworker_pool_requests WHERE state IN ({}) ORDER BY sequence",
    marks.join(",")
  );

  all(db, &sql, states.iter().map(|state| (*state).into()).collect())
    .await?
    .iter()
    .map(request_of)
    .collect()
}

/// The reservations still held, oldest first.
pub async fn held_reservations(db: &DatabaseConnection) -> Result<Vec<Reservation>, Refused> {
  all(
    db,
    "SELECT * FROM coworker_pool_reservations WHERE state = 'held' ORDER BY created_at, token",
    vec![],
  )
  .await?
  .iter()
  .map(reservation_of)
  .collect()
}

pub(crate) fn reservation_of(row: &QueryResult) -> Result<Reservation, Refused> {
  Ok(Reservation {
    token: row.try_get("", "token")?,
    reservation_key: row.try_get("", "reservation_key")?,
    state: row.try_get("", "state")?,
    owner: row.try_get("", "owner")?,
    source: row.try_get("", "source")?,
    role: row.try_get("", "role")?,
    work_key: row.try_get("", "work_key")?,
    provider: row.try_get("", "provider")?,
    model: row.try_get("", "model")?,
    effort: effort_of(row.try_get("", "effort")?),
    tier: row.try_get("", "tier")?,
    wanted: row.try_get("", "wanted")?,
    created_at: row.try_get("", "created_at")?,
  })
}

/// The live seats of one reservation, oldest first.
pub async fn seats(db: &DatabaseConnection, token: &str) -> Result<Vec<Request>, Refused> {
  all(
    db,
    "SELECT * FROM coworker_pool_requests WHERE reservation = $1 AND state IN ('waiting','reserved') ORDER BY sequence",
    vec![token.into()],
  )
  .await?
  .iter()
  .map(request_of)
  .collect()
}

/// The pings one reservation sent, one to a place.
pub async fn pings(db: &DatabaseConnection, reservation: &str) -> Result<Vec<Ping>, Refused> {
  all(
    db,
    "SELECT place, at, outcome, answered_at FROM coworker_pool_pings WHERE reservation = $1 ORDER BY place",
    vec![reservation.into()],
  )
  .await?
  .iter()
  .map(|row| {
    Ok(Ping {
      place: row.try_get("", "place")?,
      at: row.try_get("", "at")?,
      outcome: row.try_get("", "outcome")?,
      answered_at: row.try_get("", "answered_at")?,
    })
  })
  .collect()
}

/// Where every co-worker tier's limit starts.
fn default_capacity(tier: &str) -> i64 {
  TIERS.coworker.iter().find(|one| one.tier == tier).map_or(0, |one| one.capacity)
}

/// One entry per provider and co-worker tier, with what its waiting, active and reserved children hold.
pub async fn status(db: &DatabaseConnection) -> Result<Vec<TierStatus>, Refused> {
  let mut places: Vec<TierStatus> = limits(db)
    .await?
    .into_iter()
    .map(|limit| TierStatus {
      default_capacity: default_capacity(&limit.tier),
      provider: limit.provider,
      tier: limit.tier,
      capacity: limit.capacity,
      snapshot_capacities: Vec::new(),
      snapshot_models: Vec::new(),
      active: 0,
      reserved: 0,
      waiting: 0,
      yielding_active: 0,
      yielding_waiting: 0,
    })
    .collect();

  for request in requests(db, &["waiting", "active", "reserved"]).await? {
    let Some(place) = places
      .iter_mut()
      .find(|one| !request.turn && one.provider == request.provider && one.tier == request.tier)
    else {
      continue;
    };

    match request.state.as_str() {
      "active" => place.active += 1,
      "reserved" => place.reserved += 1,
      _ => place.waiting += 1,
    }

    if request.yielding && request.state == "active" {
      place.yielding_active += 1;
    }

    if request.yielding && request.state == "waiting" {
      place.yielding_waiting += 1;
    }

    if !place.snapshot_capacities.contains(&request.capacity) {
      place.snapshot_capacities.push(request.capacity);
    }

    if !place
      .snapshot_models
      .iter()
      .any(|(model, effort)| *model == request.model && *effort == request.effort)
    {
      place.snapshot_models.push((request.model.clone(), request.effort.clone()));
    }
  }

  for place in &mut places {
    place.snapshot_capacities.sort_unstable();
  }

  Ok(places)
}

/// The providers the pool shows, counts, admits and warms, in the provider order: those turned on that
/// lend co-workers.
pub async fn coworker_providers(db: &DatabaseConnection) -> Result<Vec<String>, Refused> {
  let mut lending = Vec::new();

  for entry in provider_order(db).await? {
    let declared = named(&entry.name)?.declared();

    if entry.enabled && declared.lends_coworkers {
      lending.push(entry.name);
    }
  }

  Ok(lending)
}

// The seat standing at the head of a bucket's foreground line, where one stands.
pub(crate) async fn foreground_head<C: ConnectionTrait>(
  db: &C,
  provider: &str,
  tier: &str,
) -> Result<Option<(String, Option<String>, i64)>, Refused> {
  let row = one(
    db,
    r#"SELECT token, reservation, capacity FROM coworker_pool_requests AS waiting
       WHERE provider = $1 AND tier = $2 AND state = 'waiting' AND class = 'foreground' AND turn = 0
         AND (place_key IS NULL OR NOT EXISTS (SELECT 1 FROM coworker_pool_requests AS held
           WHERE held.place_key = waiting.place_key AND held.state = 'active'))
       ORDER BY sequence LIMIT 1"#,
    vec![provider.into(), tier.into()],
  )
  .await?;

  row
    .map(|row| {
      Ok((row.try_get("", "token")?, row.try_get("", "reservation")?, row.try_get("", "capacity")?))
    })
    .transpose()
}

// How many places of a bucket are taken.
pub(crate) async fn used<C: ConnectionTrait>(db: &C, provider: &str, tier: &str) -> Result<i64, Refused> {
  let row = one(
    db,
    "SELECT COUNT(*) AS count FROM coworker_pool_requests WHERE provider = $1 AND tier = $2 AND state IN ('active','reserved') AND turn = 0",
    vec![provider.into(), tier.into()],
  )
  .await?;

  Ok(row.map(|row| row.try_get::<i64>("", "count")).transpose()?.unwrap_or(0))
}

/// Lets every seat standing at the head of its line into a place that has come free. Only a seat at
/// the head of the foreground is let in: a request standing ahead of it keeps its turn.
pub async fn promote_reservations<C: ConnectionTrait>(db: &C, now: i64) -> Result<i64, Refused> {
  let buckets = all(
    db,
    "SELECT DISTINCT provider, tier FROM coworker_pool_requests WHERE state = 'waiting' AND reservation IS NOT NULL",
    vec![],
  )
  .await?;
  let mut promoted = 0;

  for bucket in buckets {
    let provider: String = bucket.try_get("", "provider")?;
    let tier: String = bucket.try_get("", "tier")?;

    promoted += promote_bucket(db, &provider, &tier, now).await?;
  }

  Ok(promoted)
}

// One bucket's seats let in while its head is a reservation's and a place stands free.
pub(crate) async fn promote_bucket<C: ConnectionTrait>(
  db: &C,
  provider: &str,
  tier: &str,
  now: i64,
) -> Result<i64, Refused> {
  let mut promoted = 0;

  while let Some((token, Some(_), capacity)) = foreground_head(db, provider, tier).await? {
    if used(db, provider, tier).await? >= capacity {
      break;
    }

    run(
      db,
      "UPDATE coworker_pool_requests SET state = 'reserved', leased_at = $1 WHERE token = $2 AND state = 'waiting'",
      vec![now.into(), token.into()],
    )
    .await?;
    promoted += 1;
  }

  Ok(promoted)
}

/// Places a stopped order waited on given back, each cancelled with why, one after another.
pub async fn cancel_waiting(
  db: &DatabaseConnection,
  requests: &[String],
  why: &str,
) -> Result<(), Refused> {
  for request in requests {
    give_back(db, request, why, true, crate::now_ms()).await?;
  }

  Ok(())
}

/// A place in the pool given back — released, or cancelled where it never ran — with what it came to,
/// and the seat at the head of its bucket's line let into it; whether the place was still held.
pub async fn give_back(
  db: &DatabaseConnection,
  token: &str,
  outcome: &str,
  cancel: bool,
  now: i64,
) -> Result<bool, Refused> {
  let outcome = outcome.trim();

  if outcome.is_empty() || outcome.chars().count() > 500 || outcome.contains('\0') {
    return Err(Refused::new(axum::http::StatusCode::BAD_REQUEST, "Invalid pool outcome"));
  }

  let written = db.begin().await?;
  let changed = run(
    &written,
    "UPDATE coworker_pool_requests SET state = $1, ended_at = $2, outcome = $3
     WHERE token = $4 AND state IN ('waiting','active','reserved')",
    vec![
      (if cancel { "cancelled" } else { "released" }).into(),
      now.into(),
      outcome.into(),
      token.into(),
    ],
  )
  .await?;

  // Rolled back here rather than left to its drop, which only queues the rollback on the pooled
  // connection and holds the row's lock until it runs.
  if changed != 1 {
    written.rollback().await?;

    return Ok(false);
  }

  let bucket = one(
    &written,
    "SELECT provider, tier FROM coworker_pool_requests WHERE token = $1",
    vec![token.into()],
  )
  .await?;

  if let Some(bucket) = bucket {
    let provider: String = bucket.try_get("", "provider")?;
    let tier: String = bucket.try_get("", "tier")?;

    promote_bucket(&written, &provider, &tier, now).await?;
  }

  written.commit().await?;

  Ok(true)
}

/// One tier's limit on one provider written, and the seats it frees let in.
pub async fn set_limit(
  db: &DatabaseConnection,
  provider: &str,
  tier: &str,
  capacity: i64,
  now: i64,
) -> Result<Result<(), PoolRefusal>, Refused> {
  if !ARCHIVE_PROVIDERS.contains(&provider) {
    return Ok(Err(PoolRefusal("Unknown pool provider".into())));
  }

  if !TIERS.holds(tier) {
    return Ok(Err(PoolRefusal("Unknown pool tier".into())));
  }

  if !TIERS.lends(tier) {
    return Ok(Err(PoolRefusal(format!(
      "{tier} is not a co-worker tier; a co-worker is borrowed at {}",
      TIERS.coworker_tiers().join(", ")
    ))));
  }

  if !(0..=RESEARCH.pool_capacity_most).contains(&capacity) {
    return Ok(Err(PoolRefusal("Pool capacity must be a whole number from 0 to 128".into())));
  }

  let written = db.begin().await?;

  run(
    &written,
    r#"INSERT INTO coworker_pool_tier_limits (provider, tier, capacity, set_at) VALUES ($1, $2, $3, $4)
       ON CONFLICT (provider, tier) DO UPDATE SET capacity = excluded.capacity, set_at = excluded.set_at"#,
    vec![provider.into(), tier.into(), capacity.into(), now.into()],
  )
  .await?;
  promote_reservations(&written, now).await?;
  written.commit().await?;

  Ok(Ok(()))
}

/// Every tier limit set back to where it starts, and the seats that frees let in. Admitted requests keep
/// their snapshots.
pub async fn reset_limits(db: &DatabaseConnection, now: i64) -> Result<(), Refused> {
  let written = db.begin().await?;

  for tier in &TIERS.coworker {
    run(
      &written,
      "UPDATE coworker_pool_tier_limits SET capacity = $1, set_at = $2 WHERE tier = $3",
      vec![tier.capacity.into(), now.into(), tier.tier.clone().into()],
    )
    .await?;
  }

  written.commit().await?;

  let promoting = db.begin().await?;

  promote_reservations(&promoting, now).await?;
  promoting.commit().await?;

  Ok(())
}
