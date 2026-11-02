//! A task's place in the common co-worker pool (docs/coworker-pool.md, 「Requests and leases」, 「Prior
//! requests and yielding requests」): the provider and tier a role resolves to, a branch's
//! seat or a builder turn's place asked of the pool, admitted in its line's order, bound to the unit
//! it launched and given back with what it came to, and the seats an owner holds for a stage. Every write
//! is the TypeScript dispatch's, statement for statement, so either side of the pool's twin check leaves the
//! pool's rows the same. The labels and keys of a consumer's work arrive with each request; nothing here
//! reads its orders.

use std::sync::LazyLock;

use harness_providers::ARCHIVE_PROVIDERS;
use policy::fill_marks;
use policy::queue_words::QUEUE_WORDS;
use policy::tiers::TIERS;
use regex::Regex;
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use store::{all, one, run};

use crate::control::Refused;
use crate::control::claude_accounts::{self, Judging};
use crate::control::harness::{enabled_providers, named, provider_enabled};
use crate::control::pool::{
  Request, Reservation, ask, coworker_providers, in_the_way, promote_bucket, request_of, reservation_of,
  used,
};
use crate::control::roads::{profile, walked};
use crate::control::tiers::{self, Mapping};
use crate::places::Ground;
use crate::queue::routes::{self, JOB_WORK_KEY};
use crate::worker_session::{PROVIDER as WORKER_PROVIDER, records, warmth};

/// What a branch of a work order asks the pool as; a builder turn asks as the worker itself.
pub const COWORK: &str = "cowork";
pub const WORKER: &str = "worker";
/// What a native `Agent` call of a job's own CLI asks the pool as.
pub const NATIVE: &str = "native";

static ROLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9_-]{0,63}$").expect("a pattern"));
/// The shape a request's, an owner's and a reservation's key is held to.
pub static KEY: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9:._/-]{0,499}$").expect("a pattern"));
static SOURCE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9_-]{0,63}$").expect("a pattern"));
static CLAUDE_MODEL: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^claude-[a-zA-Z0-9._-]+$").expect("a pattern"));

/// Why a pool step did not go through, in the words the TypeScript dispatch's error carries.
pub type Fault = String;

fn refused(why: Refused) -> Fault {
  why.error
}

// A store's or a refusal's words, as a pool step's fault carries them.
fn faulted(why: impl std::fmt::Display) -> Fault {
  why.to_string()
}

/// A text the pool keeps: trimmed, not empty, within its length and with no null in it.
pub fn text(value: &str, what: &str, most: usize) -> Result<String, Fault> {
  let clean = value.trim();

  if clean.is_empty() || clean.encode_utf16().count() > most || clean.contains('\0') {
    return Err(format!("Invalid {what}"));
  }

  Ok(clean.to_owned())
}

/// A co-worker role's name, held to its shape.
pub fn role_name(value: &str) -> Result<&str, Fault> {
  if ROLE.is_match(value) { Ok(value) } else { Err("Invalid co-worker role".to_owned()) }
}

fn provider_name(value: &str) -> Result<(), Fault> {
  if ARCHIVE_PROVIDERS.contains(&value) {
    Ok(())
  } else {
    Err("Unknown pool provider".to_owned())
  }
}

fn tier_name(value: &str) -> Result<(), Fault> {
  if TIERS.holds(value) { Ok(()) } else { Err("Unknown pool tier".to_owned()) }
}

fn coworker_tier_name(value: &str) -> Result<(), Fault> {
  tier_name(value)?;

  if TIERS.lends(value) {
    Ok(())
  } else {
    Err(format!(
      "{value} is not a co-worker tier; a co-worker is borrowed at {}",
      TIERS.coworker_tiers().join(", ")
    ))
  }
}

/// The most places a pool limit or a reservation holds.
pub const MOST_SEATS: i64 = 128;

fn capacity(value: i64) -> Result<i64, Fault> {
  if (0..=MOST_SEATS).contains(&value) {
    Ok(value)
  } else {
    Err(format!("Pool capacity must be a whole number from 0 to {MOST_SEATS}"))
  }
}

fn is_model_tier(value: &str) -> bool {
  TIERS.ladder.iter().any(|tier| tier == value)
}

/// The tier a role runs at by default: its assignment, which must stand on the ladder.
pub async fn role_tier(db: &DatabaseConnection, role: &str) -> Result<String, Fault> {
  let key = format!("role:{role}");
  let assigned = tiers::assignments(db)
    .await
    .map_err(refused)?
    .into_iter()
    .find(|assigned| assigned.key == key)
    .ok_or_else(|| format!("Missing tier assignment: {key}"))?;

  if is_model_tier(&assigned.tier) {
    Ok(assigned.tier)
  } else {
    Err(format!("Unknown model tier: {}", assigned.tier))
  }
}

/// What a place in the pool is resolved from: the role, the selection the task names, the tier it asks
/// for, the provider its caller names where only some carry its line, and the work key its caller names
/// where its line is not the role's own.
pub struct Wanted<'a> {
  pub role: &'a str,
  pub selection: &'a str,
  pub requested_tier: Option<&'a str>,
  pub provider: Option<&'a str>,
  pub work_key: Option<&'a str>,
}

impl Wanted<'_> {
  /// The work key the place is resolved and kept under: the one its caller named, or the role's.
  pub fn work_key(&self) -> String {
    self.work_key.map_or_else(|| format!("role:{}", self.role), str::to_owned)
  }
}

/// Where a branch runs, resolved apart from its parent: the selection chooses a provider, the role's tier
/// the model and effort on it, and only a provider that lends co-workers takes one.
pub async fn coworker_mapping(db: &DatabaseConnection, wanted: &Wanted<'_>) -> Result<Mapping, Fault> {
  let open = coworker_providers(db).await.map_err(refused)?;

  on_providers(db, wanted, &open).await
}

/// Where a builder turn runs: the worker itself, on whichever enabled provider its selection names.
pub async fn turn_mapping(db: &DatabaseConnection, wanted: &Wanted<'_>) -> Result<Mapping, Fault> {
  let open = enabled_providers(db).await.map_err(refused)?;

  on_providers(db, wanted, &open).await
}

async fn on_providers(
  db: &DatabaseConnection,
  wanted: &Wanted<'_>,
  open: &[String],
) -> Result<Mapping, Fault> {
  let selection = wanted.selection;

  if !is_model_tier(selection) && !routes::is_selection(selection) {
    return Err(fill_marks(&QUEUE_WORDS.routes.model_wrong, &[("selection", selection)]));
  }

  if let Some(provider) = wanted.provider
    && !open.iter().any(|one| one == provider)
  {
    return Err(if provider_enabled(db, provider).await.map_err(refused)? {
      format!("{provider}: {}", QUEUE_WORDS.pool.single_model)
    } else {
      format!("{provider}: {}", QUEUE_WORDS.pool.off)
    });
  }

  let explicit = wanted.provider.map(str::to_owned).or_else(|| {
    routes::choice(selection)
      .map(|choice| choice.provider.to_owned())
      .or_else(|| CLAUDE_MODEL.is_match(selection).then(|| "claude-code".to_owned()))
  });
  let key = wanted.work_key();
  let line = profile(JOB_WORK_KEY).map_or("worker", |profile| profile.line.as_str());
  let provider = match explicit.filter(|explicit| open.contains(explicit)) {
    Some(provider) => Some(provider),
    None => {
      let mut found = None;

      for name in open {
        if walked(db, named(name).map_err(refused)?, &key, line).await.map_err(refused)? {
          found = Some(name.clone());
          break;
        }
      }

      found
    }
  };
  let provider =
    provider.ok_or_else(|| "No enabled provider is available for a co-worker role".to_owned())?;
  let tier = match wanted.requested_tier {
    Some(tier) => tier.to_owned(),
    None if is_model_tier(selection) => selection.to_owned(),
    None => role_tier(db, wanted.role).await?,
  };

  tiers::resolve_execution(db, &key, &provider, Some(&tier)).await
}

/// One place asked of the pool: under a key the asker names again, by whom, for what, at which
/// generation, and where a builder turn stands. A branch is a child of its order; a turn is the worker.
pub struct Asked<'a> {
  pub request_key: String,
  pub owner: String,
  pub wanted: Wanted<'a>,
  pub generation: i64,
  pub yielding: bool,
  pub token: Option<&'a str>,
  pub place_key: Option<&'a str>,
}

/// The request a token names.
pub async fn request<C: ConnectionTrait>(db: &C, token: &str) -> Result<Option<Request>, Fault> {
  one(db, "SELECT * FROM coworker_pool_requests WHERE token = $1", vec![token.into()])
    .await
    .map_err(faulted)?
    .as_ref()
    .map(request_of)
    .transpose()
    .map_err(refused)
}

/// The requests standing in the given states, in the order they came.
pub async fn requests<C: ConnectionTrait>(db: &C, states: &[&str]) -> Result<Vec<Request>, Fault> {
  let marks: Vec<String> = (1..=states.len()).map(|at| format!("${at}")).collect();
  let sql = format!(
    "SELECT * FROM coworker_pool_requests WHERE state IN ({}) ORDER BY sequence",
    marks.join(",")
  );

  all(db, &sql, states.iter().map(|state| (*state).into()).collect())
    .await
    .map_err(faulted)?
    .iter()
    .map(request_of)
    .collect::<Result<_, _>>()
    .map_err(refused)
}

/// A co-work branch's seat asked of the pool, or the one its token already holds.
pub async fn enqueue_coworker(
  db: &DatabaseConnection,
  asked: &Asked<'_>,
  retain: impl AsyncFnOnce(&sea_orm::DatabaseTransaction, &Request) -> Result<(), Fault>,
) -> Result<Request, Fault> {
  enqueue_seat(db, asked, COWORK, retain).await
}

/// A branch's seat asked of the pool under the source its caller labels it with — a co-work branch's, or
/// the one a seat request names — or the one its token already holds.
pub async fn enqueue_seat(
  db: &DatabaseConnection,
  asked: &Asked<'_>,
  source: &str,
  retain: impl AsyncFnOnce(&sea_orm::DatabaseTransaction, &Request) -> Result<(), Fault>,
) -> Result<Request, Fault> {
  enqueue_request(db, asked, source, true, None, retain).await
}

/// A native `Agent` call's place asked of the pool, on the mapping its role resolves to on Claude Code, the
/// call's own row kept beside it in the same transaction (docs/coworker-pool.md, 「Native co-worker
/// places」).
pub async fn enqueue_native(
  db: &DatabaseConnection,
  asked: &Asked<'_>,
  mapping: &Mapping,
  retain: impl AsyncFnOnce(&sea_orm::DatabaseTransaction, &Request) -> Result<(), Fault>,
) -> Result<Request, Fault> {
  let work_key = asked.wanted.work_key();

  enqueue(
    db,
    asked,
    NATIVE,
    &work_key,
    mapping,
    Standing {
      child: true,
      session: None,
    },
    retain,
  )
  .await
}

/// A builder turn's place asked of the pool, as the worker itself, on its checkout.
pub async fn enqueue_turn(
  ground: &Ground,
  asked: &Asked<'_>,
  retain: impl AsyncFnOnce(&sea_orm::DatabaseTransaction, &Request) -> Result<(), Fault>,
) -> Result<Request, Fault> {
  enqueue_request(&ground.db, asked, WORKER, false, Some(ground), retain).await
}

async fn enqueue_request(
  db: &DatabaseConnection,
  asked: &Asked<'_>,
  source: &str,
  child: bool,
  ground: Option<&Ground>,
  retain: impl AsyncFnOnce(&sea_orm::DatabaseTransaction, &Request) -> Result<(), Fault>,
) -> Result<Request, Fault> {
  let place_key = if child { None } else { asked.place_key };
  let held = match asked.token {
    Some(token) => Some(request(db, token).await?.ok_or("Stored co-worker pool request is missing")?),
    None => None,
  };
  let work_key = asked.wanted.work_key();

  if let Some(held) = &held
    && (held.request_key != asked.request_key
      || held.owner != asked.owner
      || held.source != source
      || held.role != asked.wanted.role
      || held.work_key != work_key
      || held.generation != asked.generation
      || held.place_key.as_deref() != place_key
      || asked.wanted.requested_tier.is_some_and(|tier| held.tier != tier))
  {
    return Err("Stored co-worker pool request does not match its execution root".to_owned());
  }

  let held = match held {
    Some(held) => held,
    None => {
      let mapping = if child {
        coworker_mapping(db, &asked.wanted).await?
      } else {
        turn_mapping(db, &asked.wanted).await?
      };
      // A project's own turn continues the session its worker still holds warm and within the model's
      // context; a child's seat opens its own, and native calls keep their parent's (「A project's own
      // session」).
      let session = match ground.filter(|_| !child) {
        Some(ground) => project_session(ground, asked.place_key, &mapping).await?,
        None => None,
      };

      enqueue(db, asked, source, &work_key, &mapping, Standing { child, session }, retain).await?
    }
  };

  if held.state == "released" || held.state == "cancelled" {
    return Err("Stored co-worker pool request is already finished".to_owned());
  }

  Ok(held)
}

/// The session the project a turn stands for was working in (docs/coworker-pool.md, 「A project's own
/// session」): the one its worker holds on the model the turn goes out on and the login the job line is
/// walked on first, while the provider's declared cache lifetime for that model has not passed since the
/// session last answered and its last answered turn stayed under the model's context limit.
///
/// The worker is the one standing in the place the turn is keyed by, which is the project's checkout. A
/// session past either condition is dropped with its record by the rules the worker's sessions already
/// stand on, and the turn opens a new one as any replacement does.
async fn project_session(
  ground: &Ground,
  place_key: Option<&str>,
  mapping: &Mapping,
) -> Result<Option<String>, Fault> {
  let Some(place) = place_key else { return Ok(None) };

  // A worker's held session is Claude Code's; a turn on another road replaces the held session and keeps
  // none of it (docs/sessions.md, 「A worker's session per model」).
  if mapping.provider != WORKER_PROVIDER {
    return Ok(None);
  }

  let worker = one(
    &ground.db,
    r#"SELECT "name" FROM "agents"
       WHERE "kind" = 'worker' AND "cwd" = $1 AND "resting_since" IS NULL
       ORDER BY "name" LIMIT 1"#,
    vec![place.into()],
  )
  .await
  .map_err(faulted)?;
  let Some(worker) = worker else { return Ok(None) };
  let worker: String = worker.try_get("", "name").map_err(|why| why.to_string())?;

  let account = claude_accounts::walk_for(Judging::of(ground), JOB_WORK_KEY)
    .await
    .map_err(refused)?
    .into_iter()
    .next();
  let Some(account) = account else { return Ok(None) };

  let held = one(
    &ground.db,
    r#"SELECT "claude_session_id", "last_reply_at", "context_ratio" FROM "worker_sessions"
       WHERE "worker" = $1 AND "model" = $2 AND "account" = $3"#,
    vec![worker.clone().into(), mapping.model.clone().into(), account.id.clone().into()],
  )
  .await
  .map_err(faulted)?;
  let Some(held) = held else { return Ok(None) };

  let id: String = held.try_get("", "claude_session_id").map_err(|why| why.to_string())?;
  let last: i64 = held.try_get("", "last_reply_at").map_err(|why| why.to_string())?;
  let filled: Option<f64> = held.try_get("", "context_ratio").map_err(|why| why.to_string())?;
  let warm = warmth::lifetime_ms(WORKER_PROVIDER, &mapping.model)
    .is_none_or(|lifetime| warmth::warm(last.saturating_mul(1000), crate::now_ms(), lifetime));
  let within = filled.is_none_or(|ratio| ratio < 1.0);

  if !warm || !within {
    records::forget(&ground.db, &worker, &mapping.model, Some(&account.id))
      .await
      .map_err(|why| refused(why.into()))?;

    return Ok(None);
  }

  Ok(Some(id))
}

/// What a request stands for beside its mapping: a borrowed child's seat or a worker's own turn on its
/// place, and the session a project's own turn continues where the pool judged one still usable.
struct Standing {
  child: bool,
  session: Option<String>,
}

// The pool's own enqueue: the request written once under its key, its identity held to what was asked,
// and the token kept by the asker in the same transaction, so no process dying between the two leaves a
// head of the line nobody owns.
async fn enqueue(
  db: &DatabaseConnection,
  asked: &Asked<'_>,
  source: &str,
  work_key: &str,
  mapping: &Mapping,
  standing: Standing,
  retain: impl AsyncFnOnce(&sea_orm::DatabaseTransaction, &Request) -> Result<(), Fault>,
) -> Result<Request, Fault> {
  let Standing { child, session } = standing;
  if !KEY.is_match(&asked.request_key) || !KEY.is_match(&asked.owner) || !KEY.is_match(work_key) {
    return Err("Invalid pool request identity".to_owned());
  }

  if !SOURCE.is_match(source) {
    return Err("Invalid pool request source".to_owned());
  }

  if asked.generation < 1 {
    return Err("Invalid pool request generation".to_owned());
  }

  let name = role_name(asked.wanted.role)?;

  provider_name(&mapping.provider)?;
  tier_name(&mapping.tier)?;

  let place_key = asked
    .place_key
    .filter(|_| !child)
    .map(|key| text(key, "pool place key", 500))
    .transpose()?;

  if child {
    coworker_tier_name(&mapping.tier)?;
  } else if place_key.is_none() {
    return Err("A worker's own turn needs its place key".to_owned());
  }

  let now = crate::now_ms();
  let written = db.begin().await.map_err(|why| why.to_string())?;
  let role = one(
    &written,
    "SELECT enabled, pooled FROM coworker_pool_roles WHERE name = $1",
    vec![name.into()],
  )
  .await
  .map_err(faulted)?;
  let (enabled, pooled) = match role {
    Some(row) => (
      row.try_get::<i64>("", "enabled").map_err(|why| why.to_string())? == 1,
      row.try_get::<i64>("", "pooled").map_err(|why| why.to_string())? == 1,
    ),
    // An agent role lends its seats as a pooled role does: its tier is its key's assignment, held to a tier
    // a co-worker is borrowed at like any other (docs/coworker-pool.md, 「Who takes a place」).
    None => {
      let agent_role = one(&written, r#"SELECT 1 FROM "agent_roles" WHERE "name" = $1"#, vec![name.into()])
        .await
        .map_err(faulted)?;

      (agent_role.is_some(), agent_role.is_some())
    }
  };

  if !enabled || (child && !pooled) {
    return Err(format!("Co-worker role is not available in the pool: {name}"));
  }

  let mut capacity = 0;

  if child {
    let limit = one(
      &written,
      "SELECT capacity FROM coworker_pool_tier_limits WHERE provider = $1 AND tier = $2",
      vec![mapping.provider.clone().into(), mapping.tier.clone().into()],
    )
    .await
    .map_err(faulted)?
    .map(|row| row.try_get::<i64>("", "capacity"))
    .transpose()
    .map_err(|why| why.to_string())?;

    capacity = match limit {
      Some(limit) if limit > 0 => limit,
      _ => return Err(format!("No co-worker pool capacity for {}/{}", mapping.provider, mapping.tier)),
    };
  }

  run(
    &written,
    r#"INSERT INTO coworker_pool_requests
       (token,request_key,owner,source,role,work_key,provider,model,effort,tier,capacity,generation,state,enqueued_at,class,place_key,turn,session)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,'waiting',$13,$14,$15,$16,$17)
       ON CONFLICT (request_key) DO NOTHING"#,
    vec![
      uuid::Uuid::new_v4().to_string().into(),
      asked.request_key.clone().into(),
      asked.owner.clone().into(),
      source.into(),
      name.into(),
      work_key.into(),
      mapping.provider.clone().into(),
      mapping.model.clone().into(),
      mapping.effort.clone().unwrap_or_default().into(),
      mapping.tier.clone().into(),
      capacity.into(),
      asked.generation.into(),
      now.into(),
      (if asked.yielding { "yielding" } else { "foreground" }).into(),
      place_key.clone().into(),
      i64::from(!child).into(),
      session.into(),
    ],
  )
  .await
  .map_err(faulted)?;

  let held = one(
    &written,
    "SELECT * FROM coworker_pool_requests WHERE request_key = $1",
    vec![asked.request_key.clone().into()],
  )
  .await
  .map_err(faulted)?
  .as_ref()
  .map(request_of)
  .transpose()
  .map_err(refused)?
  .ok_or("the pool request was not written")?;

  // The class may have moved to the foreground since; who asked, for what and where it counts may not.
  let same = held.owner == asked.owner
    && held.source == source
    && held.role == name
    && held.work_key == work_key
    && held.provider == mapping.provider
    && held.model == mapping.model
    && held.effort == mapping.effort
    && held.tier == mapping.tier
    && held.generation == asked.generation
    && held.place_key == place_key
    && held.turn == !child;

  if !same {
    return Err(format!("Pool request identity changed: {}", asked.request_key));
  }

  retain(&written, &held).await?;
  written.commit().await.map_err(|why| why.to_string())?;

  Ok(held)
}

/// The request let in where its turn has come, or none where it waits on: the asker's owner asking for it,
/// let in once no request whose owner is asking stands ahead of it.
pub async fn acquire(db: &DatabaseConnection, held: &Request) -> Result<Option<Request>, Fault> {
  if held.state == "released" || held.state == "cancelled" {
    return Ok(None);
  }

  let written = db.begin().await.map_err(|why| why.to_string())?;
  let admitted = admission(&written, &held.token, crate::now_ms()).await?;

  written.commit().await.map_err(|why| why.to_string())?;

  Ok(admitted)
}

async fn admission<C: ConnectionTrait>(db: &C, token: &str, now: i64) -> Result<Option<Request>, Fault> {
  let Some(held) = request(db, token).await? else { return Ok(None) };

  if held.state == "active" {
    return Ok(Some(held));
  }

  // A seat is let in by its reservation, never taken as a lease by whoever holds its token.
  if held.state != "waiting" || held.reservation.is_some() {
    return Ok(None);
  }

  // This ask is numbered, and the owner's ask before it is what the requests ahead are judged against.
  let since = ask(db, token).await.map_err(refused)?;

  if place_taken(db, held.place_key.as_deref()).await? {
    return Ok(None);
  }

  if held.turn {
    return admit_turn(db, &held, now).await;
  }

  if held.yielding {
    return admit_yielding(db, &held, since, now).await;
  }

  // A seat its own owner holds in this bucket is the request's place: the seat is given up and the
  // request let in in its stead, which moves nothing in the count.
  let seat = one(
    db,
    r#"SELECT token FROM coworker_pool_requests
       WHERE provider = $1 AND tier = $2 AND owner = $3 AND state = 'reserved' AND reservation IS NOT NULL
       ORDER BY sequence LIMIT 1"#,
    vec![held.provider.clone().into(), held.tier.clone().into(), held.owner.clone().into()],
  )
  .await
  .map_err(faulted)?;

  if let Some(seat) = seat {
    let seat: String = seat.try_get("", "token").map_err(|why| why.to_string())?;

    end_seat(db, &seat, "released", &format!("taken by {token}"), now).await?;
    return admit(db, token, now).await;
  }

  // The owner's own seat standing first in the request's way hands its turn to the request; any other
  // keeps it waiting.
  let inherits = match in_the_way(db, &held.provider, &held.tier, held.sequence, since)
    .await
    .map_err(refused)?
  {
    None => None,
    Some((first, Some(_), owner)) if owner == held.owner => Some(first),
    Some(_) => return Ok(None),
  };

  if used(db, &held.provider, &held.tier).await.map_err(refused)? >= held.capacity {
    return Ok(None);
  }

  if let Some(first) = inherits {
    end_seat(db, &first, "cancelled", &format!("turn given to {token}"), now).await?;
  }

  admit(db, token, now).await
}

// Whether an active request already stands in this place.
async fn place_taken<C: ConnectionTrait>(db: &C, place_key: Option<&str>) -> Result<bool, Fault> {
  let Some(place_key) = place_key else { return Ok(false) };

  Ok(
    one(
      db,
      "SELECT 1 AS taken FROM coworker_pool_requests WHERE place_key = $1 AND state = 'active' LIMIT 1",
      vec![place_key.into()],
    )
    .await
    .map_err(faulted)?
    .is_some(),
  )
}

// A worker's own turn goes in once its place is free: the earliest foreground turn waiting for that place
// first, and a yielding one only when no foreground turn waits there.
async fn admit_turn<C: ConnectionTrait>(
  db: &C,
  held: &Request,
  now: i64,
) -> Result<Option<Request>, Fault> {
  let first = one(
    db,
    r#"SELECT token FROM coworker_pool_requests WHERE place_key = $1 AND state = 'waiting' AND turn = 1
       ORDER BY class = 'yielding', sequence LIMIT 1"#,
    vec![held.place_key.clone().into()],
  )
  .await
  .map_err(faulted)?
  .map(|row| row.try_get::<String>("", "token"))
  .transpose()
  .map_err(|why| why.to_string())?;

  if first.as_deref() != Some(held.token.as_str()) {
    return Ok(None);
  }

  admit(db, &held.token, now).await
}

// A yielding request goes in only where no foreground request stands in its way for its tier or waits for its
// place, and only as the first of the yielding requests that could go in and stand in its way; a request
// stands in its way as the line's rule says, judged against the owner's ask before this one, `since`.
async fn admit_yielding<C: ConnectionTrait>(
  db: &C,
  held: &Request,
  since: Option<i64>,
  now: i64,
) -> Result<Option<Request>, Fault> {
  let waits = one(
    db,
    r#"SELECT 1 AS waits FROM coworker_pool_requests
       WHERE provider = $1 AND tier = $2 AND state = 'waiting' AND class = 'foreground' AND turn = 0
         AND (reservation IS NOT NULL OR $3::bigint IS NULL OR asked > $3::bigint) LIMIT 1"#,
    vec![held.provider.clone().into(), held.tier.clone().into(), since.into()],
  )
  .await
  .map_err(faulted)?;

  if waits.is_some() {
    return Ok(None);
  }

  if let Some(place_key) = &held.place_key {
    let place_waits = one(
      db,
      r#"SELECT 1 AS waits FROM coworker_pool_requests
         WHERE place_key = $1 AND state = 'waiting' AND class = 'foreground' LIMIT 1"#,
      vec![place_key.clone().into()],
    )
    .await
    .map_err(faulted)?;

    if place_waits.is_some() {
      return Ok(None);
    }
  }

  let first = one(
    db,
    r#"SELECT token FROM coworker_pool_requests AS waiting
       WHERE provider = $1 AND tier = $2 AND state = 'waiting' AND class = 'yielding' AND turn = 0
         AND (place_key IS NULL OR NOT EXISTS (SELECT 1 FROM coworker_pool_requests AS other
           WHERE other.place_key = waiting.place_key
             AND (other.state = 'active' OR (other.state = 'waiting' AND other.class = 'foreground'))))
         AND (token = $3 OR $4::bigint IS NULL OR asked > $4::bigint)
       ORDER BY sequence LIMIT 1"#,
    vec![
      held.provider.clone().into(),
      held.tier.clone().into(),
      held.token.clone().into(),
      since.into(),
    ],
  )
  .await
  .map_err(faulted)?
  .map(|row| row.try_get::<String>("", "token"))
  .transpose()
  .map_err(|why| why.to_string())?;

  if first.as_deref() != Some(held.token.as_str()) {
    return Ok(None);
  }

  if used(db, &held.provider, &held.tier).await.map_err(refused)? >= held.capacity {
    return Ok(None);
  }

  admit(db, &held.token, now).await
}

async fn admit<C: ConnectionTrait>(db: &C, token: &str, now: i64) -> Result<Option<Request>, Fault> {
  let changed = run(
    db,
    "UPDATE coworker_pool_requests SET state = 'active', leased_at = $1 WHERE token = $2 AND state = 'waiting'",
    vec![now.into(), token.into()],
  )
  .await
  .map_err(faulted)?;

  if changed != 1 {
    return Ok(None);
  }

  request(db, token).await
}

async fn end_seat<C: ConnectionTrait>(
  db: &C,
  token: &str,
  state: &str,
  outcome: &str,
  now: i64,
) -> Result<(), Fault> {
  run(
    db,
    "UPDATE coworker_pool_requests SET state = $1, ended_at = $2, outcome = $3 WHERE token = $4 AND state IN ('waiting','reserved')",
    vec![state.into(), now.into(), text(outcome, "pool outcome", 500)?.into(), token.into()],
  )
  .await
  .map_err(faulted)
  .map(drop)
}

/// The unit a launch carries a place's lease into, written once: a lease no longer active, or bound to
/// another unit, refuses it.
pub async fn bind(db: &DatabaseConnection, held: &Request, child_id: &str) -> Result<Request, Fault> {
  if held.state != "active" {
    return Err("Co-worker launch requires an active pool lease".to_owned());
  }

  if request(db, &held.token).await?.map(|now| now.state).as_deref() != Some("active") {
    return Err("Co-worker pool lease is no longer active".to_owned());
  }

  let clean = text(child_id, "pool child identity", 500)?;
  let written = db.begin().await.map_err(|why| why.to_string())?;
  let now = request(&written, &held.token).await?.ok_or("Unknown co-worker pool lease")?;

  if now.state != "active" {
    return Err("Only an active co-worker pool lease may be bound".to_owned());
  }

  if now.child_id.as_ref().is_some_and(|bound| *bound != clean) {
    return Err("Co-worker pool child identity cannot change".to_owned());
  }

  run(
    &written,
    "UPDATE coworker_pool_requests SET child_id = COALESCE(child_id, $1) WHERE token = $2 AND state = 'active'",
    vec![clean.into(), held.token.clone().into()],
  )
  .await
  .map_err(faulted)?;

  let bound = request(&written, &held.token).await?.ok_or("Unknown co-worker pool lease")?;

  written.commit().await.map_err(|why| why.to_string())?;

  Ok(bound)
}

/// A launch held to the lease it was admitted on: the same owner, role, provider and model, bound to the
/// unit it starts.
pub async fn require(
  db: &DatabaseConnection,
  token: Option<&str>,
  owner: &str,
  role: &str,
  child_id: &str,
  provider: &str,
  model: &str,
) -> Result<Request, Fault> {
  let token = token.ok_or("Co-worker launch has no pool lease")?;
  let held = request(db, token).await?.filter(|held| held.state == "active");
  let held = held.ok_or("Co-worker launch has no active pool lease")?;

  if held.owner != owner
    || held.source != COWORK
    || held.role != role
    || held.provider != provider
    || held.model != model
  {
    return Err("Co-worker launch does not match its pool lease".to_owned());
  }

  bind(db, &held, child_id).await
}

/// One lease ended with what it came to — released, or cancelled where it never ran — and the seat at the
/// head of its line let into the place; whether it was still held.
pub async fn finish(
  db: &DatabaseConnection,
  token: &str,
  outcome: &str,
  cancel: bool,
) -> Result<bool, Fault> {
  let clean = text(outcome, "pool outcome", 500)?;

  crate::control::pool::give_back(db, token, &clean, cancel, crate::now_ms())
    .await
    .map_err(refused)
}

/// What an owner's seats are held as: under a key the owner names again, by what brought them — a work
/// order's stage, a seat request's owner — for its role on the provider and tier they will be let in under,
/// so many of them.
pub struct Seats<'a> {
  pub reservation_key: String,
  pub owner: String,
  pub source: &'a str,
  pub wanted: Wanted<'a>,
  pub count: i64,
}

/// Seats of one tier on one provider held for an owner: the role resolved as a request of it would be, so
/// the seats count where the children will. Asked again, the reservation keeps its snapshot and only its
/// live seats move to the new count — more join the line behind whoever stands in it, fewer give up the
/// youngest first.
pub async fn reserve(db: &DatabaseConnection, seats: &Seats<'_>) -> Result<Reservation, Fault> {
  let mapping = coworker_mapping(db, &seats.wanted).await?;

  reserve_on(db, seats, &mapping).await
}

/// Seats held as `reserve` holds them, on a mapping the asker resolved itself: a CLI's own native calls
/// hold theirs on the tier their role resolves to on Claude Code.
pub async fn reserve_on(
  db: &DatabaseConnection,
  seats: &Seats<'_>,
  mapping: &Mapping,
) -> Result<Reservation, Fault> {
  let work_key = seats.wanted.work_key();

  if !KEY.is_match(&seats.reservation_key) || !KEY.is_match(&seats.owner) || !KEY.is_match(&work_key) {
    return Err("Invalid pool reservation identity".to_owned());
  }

  let count = capacity(seats.count)?;
  let name = role_name(seats.wanted.role)?;

  provider_name(&mapping.provider)?;
  coworker_tier_name(&mapping.tier)?;

  let now = crate::now_ms();
  let written = db.begin().await.map_err(|why| why.to_string())?;
  let role = one(
    &written,
    "SELECT enabled, pooled FROM coworker_pool_roles WHERE name = $1",
    vec![name.into()],
  )
  .await
  .map_err(faulted)?;
  let lent = match role {
    Some(row) => {
      row.try_get::<i64>("", "enabled").map_err(|why| why.to_string())? == 1
        && row.try_get::<i64>("", "pooled").map_err(|why| why.to_string())? == 1
    }
    None => false,
  };

  if !lent {
    return Err(format!("Co-worker role is not available in the pool: {name}"));
  }

  let limit = one(
    &written,
    "SELECT capacity FROM coworker_pool_tier_limits WHERE provider = $1 AND tier = $2",
    vec![mapping.provider.clone().into(), mapping.tier.clone().into()],
  )
  .await
  .map_err(faulted)?
  .map(|row| row.try_get::<i64>("", "capacity"))
  .transpose()
  .map_err(|why| why.to_string())?;
  let limit = match limit {
    Some(limit) if limit > 0 => limit,
    _ => return Err(format!("No co-worker pool capacity for {}/{}", mapping.provider, mapping.tier)),
  };

  run(
    &written,
    r#"INSERT INTO coworker_pool_reservations
       (token,reservation_key,owner,source,role,work_key,provider,model,effort,tier,wanted,state,created_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'held',$12)
       ON CONFLICT (reservation_key) DO NOTHING"#,
    vec![
      uuid::Uuid::new_v4().to_string().into(),
      seats.reservation_key.clone().into(),
      seats.owner.clone().into(),
      seats.source.into(),
      name.into(),
      work_key.clone().into(),
      mapping.provider.clone().into(),
      mapping.model.clone().into(),
      mapping.effort.clone().unwrap_or_default().into(),
      mapping.tier.clone().into(),
      count.into(),
      now.into(),
    ],
  )
  .await
  .map_err(faulted)?;

  let held = reservation_by_key(&written, &seats.reservation_key).await?;
  let same = held.owner == seats.owner
    && held.source == seats.source
    && held.role == name
    && held.provider == mapping.provider
    && held.tier == mapping.tier
    && held.work_key == work_key;

  if !same {
    return Err(format!("Pool reservation identity changed: {}", seats.reservation_key));
  }

  // A key given back and asked for again is held again, on the mapping of this moment: its old seats have
  // all ended, so the new ones join the line afresh.
  let (model, effort) = if held.state == "held" {
    (held.model.clone(), held.effort.clone())
  } else {
    run(
      &written,
      r#"UPDATE coworker_pool_reservations SET state = 'held', model = $1, effort = $2, created_at = $3, ended_at = NULL, outcome = NULL
         WHERE token = $4"#,
      vec![
        mapping.model.clone().into(),
        mapping.effort.clone().unwrap_or_default().into(),
        now.into(),
        held.token.clone().into(),
      ],
    )
    .await
    .map_err(faulted)?;

    (mapping.model.clone(), mapping.effort.clone())
  };

  run(
    &written,
    "UPDATE coworker_pool_reservations SET wanted = $1 WHERE token = $2",
    vec![count.into(), held.token.clone().into()],
  )
  .await
  .map_err(faulted)?;

  let live = live_seats(&written, &held.token).await?;
  let made: i64 = one(
    &written,
    "SELECT COUNT(*) AS count FROM coworker_pool_requests WHERE reservation = $1",
    vec![held.token.clone().into()],
  )
  .await
  .map_err(faulted)?
  .map(|row| row.try_get::<i64>("", "count"))
  .transpose()
  .map_err(|why| why.to_string())?
  .unwrap_or(0);
  let live_count = i64::try_from(live.len()).unwrap_or(i64::MAX);

  for seat in (made + 1)..=(made + count - live_count) {
    run(
      &written,
      r#"INSERT INTO coworker_pool_requests
         (token,request_key,owner,source,role,work_key,provider,model,effort,tier,capacity,generation,state,enqueued_at,reservation)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,1,'waiting',$12,$13)"#,
      vec![
        uuid::Uuid::new_v4().to_string().into(),
        format!("{}:seat-{seat}", held.reservation_key).into(),
        held.owner.clone().into(),
        held.source.clone().into(),
        held.role.clone().into(),
        work_key.clone().into(),
        held.provider.clone().into(),
        model.clone().into(),
        effort.clone().unwrap_or_default().into(),
        held.tier.clone().into(),
        limit.into(),
        now.into(),
        held.token.clone().into(),
      ],
    )
    .await
    .map_err(faulted)?;
  }

  for seat in live.iter().skip(usize::try_from(count).unwrap_or(0)) {
    let state = if seat.state == "reserved" { "released" } else { "cancelled" };

    end_seat(&written, &seat.token, state, "reservation lowered", now).await?;
  }

  promote_bucket(&written, &held.provider, &held.tier, now).await.map_err(refused)?;

  let reserved = reservation_by_token(&written, &held.token).await?;

  written.commit().await.map_err(|why| why.to_string())?;

  Ok(reserved)
}

async fn reservation_by_key<C: ConnectionTrait>(db: &C, key: &str) -> Result<Reservation, Fault> {
  one(
    db,
    "SELECT * FROM coworker_pool_reservations WHERE reservation_key = $1",
    vec![key.into()],
  )
  .await
  .map_err(faulted)?
  .as_ref()
  .map(reservation_of)
  .transpose()
  .map_err(refused)?
  .ok_or_else(|| "the pool reservation was not written".to_owned())
}

async fn reservation_by_token<C: ConnectionTrait>(db: &C, token: &str) -> Result<Reservation, Fault> {
  one(db, "SELECT * FROM coworker_pool_reservations WHERE token = $1", vec![token.into()])
    .await
    .map_err(faulted)?
    .as_ref()
    .map(reservation_of)
    .transpose()
    .map_err(refused)?
    .ok_or_else(|| "the pool reservation is gone".to_owned())
}

async fn live_seats<C: ConnectionTrait>(db: &C, token: &str) -> Result<Vec<Request>, Fault> {
  all(
    db,
    "SELECT * FROM coworker_pool_requests WHERE reservation = $1 AND state IN ('waiting','reserved') ORDER BY sequence",
    vec![token.into()],
  )
  .await
  .map_err(faulted)?
  .iter()
  .map(request_of)
  .collect::<Result<_, _>>()
  .map_err(refused)
}

/// A reservation given back: its waiting seats leave the line and its reserved ones free their places;
/// whether it was still held.
pub async fn release_reservation(
  db: &DatabaseConnection,
  token: &str,
  outcome: &str,
) -> Result<bool, Fault> {
  let clean = text(outcome, "pool outcome", 500)?;
  let now = crate::now_ms();
  let written = db.begin().await.map_err(|why| why.to_string())?;
  let changed = run(
    &written,
    "UPDATE coworker_pool_reservations SET state = 'released', ended_at = $1, outcome = $2 WHERE token = $3 AND state = 'held'",
    vec![now.into(), clean.clone().into(), token.into()],
  )
  .await
  .map_err(faulted)?;

  if changed != 1 {
    written.rollback().await.map_err(|why| why.to_string())?;
    return Ok(false);
  }

  let held = reservation_by_token(&written, token).await?;

  for seat in live_seats(&written, token).await? {
    let state = if seat.state == "reserved" { "released" } else { "cancelled" };

    end_seat(&written, &seat.token, state, &clean, now).await?;
  }

  promote_bucket(&written, &held.provider, &held.tier, now).await.map_err(refused)?;
  written.commit().await.map_err(|why| why.to_string())?;

  Ok(true)
}

/// The reservations an owner holds, or every one held where none is named, oldest first.
pub async fn held_reservations(
  db: &DatabaseConnection,
  owner: Option<&str>,
) -> Result<Vec<Reservation>, Fault> {
  let rows = match owner {
    Some(owner) => all(
      db,
      "SELECT * FROM coworker_pool_reservations WHERE owner = $1 AND state = 'held' ORDER BY created_at, token",
      vec![owner.into()],
    )
    .await,
    None => all(
      db,
      "SELECT * FROM coworker_pool_reservations WHERE state = 'held' ORDER BY created_at, token",
      vec![],
    )
    .await,
  };

  rows
    .map_err(faulted)?
    .iter()
    .map(reservation_of)
    .collect::<Result<_, _>>()
    .map_err(refused)
}

// A process the pool marked by its id and its start, alive while a process of that id started then stands.
fn alive(pid: Option<i64>, start: Option<&str>) -> bool {
  let (Some(pid), Some(start)) = (pid, start) else { return false };
  let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
    return false;
  };
  let Some(after) = stat.rfind(')').and_then(|at| stat.get(at + 2..)) else {
    return false;
  };

  after.split(' ').nth(19) == Some(start)
}

/// How long a lease bound to its launched child is left alone before the child's standing is asked: the
/// launch's own start, so a child that has not come up yet is never read as one that ended.
pub const BOUND_GRACE_MS: i64 = 10 * 60 * 1000;

/// Every active lease bound to a launched child — a unit, or a run a pane holds — leased before `before`,
/// as its token and its child; the native calls, whose callers are read by their processes, left out.
pub async fn bound_leases(db: &DatabaseConnection, before: i64) -> Result<Vec<(String, String)>, Fault> {
  let rows = all(
    db,
    r#"SELECT r.token, r.child_id FROM coworker_pool_requests r
       WHERE r.state = 'active' AND r.child_id IS NOT NULL AND r.leased_at < $1
         AND NOT EXISTS (SELECT 1 FROM coworker_pool_native n WHERE n.token = r.token)
       ORDER BY r.sequence"#,
    vec![before.into()],
  )
  .await
  .map_err(faulted)?;

  rows
    .iter()
    .map(|row| {
      Ok((
        row.try_get("", "token").map_err(faulted)?,
        row.try_get("", "child_id").map_err(faulted)?,
      ))
    })
    .collect()
}

/// The bound leases whose child no longer stands given back: the place a job's turn or a branch held is
/// released once its unit and its pane have ended and no open execution names it
/// (docs/coworker-pool.md, 「Prior requests and yielding requests」); how many.
pub async fn release_ended(
  db: &DatabaseConnection,
  leases: &[(String, String)],
  standing: &std::collections::HashSet<String>,
) -> Result<i64, Fault> {
  let mut released = 0;

  for (token, child) in leases {
    if standing.contains(child) {
      continue;
    }

    if finish(db, token, "its launched child no longer stands", false).await? {
      released += 1;
    }
  }

  Ok(released)
}

/// Every bound lease whose child has ended released, asked of what runs on this machine; nothing where
/// that could not be asked, since "nothing stands" and "nobody could say" are kept apart.
pub async fn reclaim_bound(
  db: &DatabaseConnection,
  running: &crate::queue::running::Running,
) -> Result<i64, Fault> {
  let leases = bound_leases(db, crate::now_ms() - BOUND_GRACE_MS).await?;

  if leases.is_empty() {
    return Ok(0);
  }

  let children: Vec<String> = leases.iter().map(|(_, child)| child.clone()).collect();
  let Some(standing) = running.answered(&children).await.map_err(|why| why.detail)? else {
    return Ok(0);
  };

  release_ended(db, &leases, &standing).await
}

// How long a native call's stop mark is kept.
const STOP_MARK_MS: i64 = 24 * 60 * 60 * 1000;

/// Every native request whose waiter, while it waits, or whose caller, once let in, is gone, ended; and
/// every native reservation whose CLI is gone given back.
pub async fn reclaim_native(db: &DatabaseConnection) -> Result<i64, Fault> {
  let now = crate::now_ms();
  let rows = all(
    db,
    r#"SELECT n.token, n.waiter_pid, n.waiter_start, n.owner_pid, n.owner_start, r.state
       FROM coworker_pool_native n JOIN coworker_pool_requests r ON r.token = n.token
       WHERE r.state IN ('waiting','active')"#,
    vec![],
  )
  .await
  .map_err(faulted)?;
  let mut reclaimed = 0;

  for row in rows {
    let read = |key: &str| row.try_get::<Option<i64>>("", key).ok().flatten();
    let text = |key: &str| row.try_get::<Option<String>>("", key).ok().flatten();
    let token: String = row.try_get("", "token").map_err(|why| why.to_string())?;
    let waiting = text("state").as_deref() == Some("waiting");
    let lives = if waiting {
      alive(read("waiter_pid"), text("waiter_start").as_deref())
    } else {
      alive(read("owner_pid"), text("owner_start").as_deref())
    };

    if lives {
      continue;
    }

    let outcome = if waiting {
      "native call's waiting hook ended"
    } else {
      "native call's caller process ended"
    };

    if finish(db, &token, outcome, waiting).await? {
      reclaimed += 1;
    }
  }

  let reservations = all(
    db,
    r#"SELECT n.token, n.owner_pid, n.owner_start FROM coworker_pool_native_reservations n
       JOIN coworker_pool_reservations r ON r.token = n.token WHERE r.state = 'held'"#,
    vec![],
  )
  .await
  .map_err(faulted)?;

  for row in reservations {
    let token: String = row.try_get("", "token").map_err(|why| why.to_string())?;
    let pid = row.try_get::<Option<i64>>("", "owner_pid").ok().flatten();
    let start = row.try_get::<Option<String>>("", "owner_start").ok().flatten();

    if alive(pid, start.as_deref()) {
      continue;
    }

    if release_reservation(db, &token, "the CLI that reserved these seats ended").await? {
      reclaimed += 1;
    }
  }

  run(
    db,
    "DELETE FROM coworker_pool_native_stops WHERE at < $1",
    vec![(now - STOP_MARK_MS).into()],
  )
  .await
  .map_err(faulted)?;

  Ok(reclaimed)
}
