//! The role slots (docs/role-slots.md): every run of a role asks a slot of it before its program launches,
//! waits in the role's line, is let in under the role's cap on the provider its order — or the road its
//! caller fixed — has quota on, is bound to the run it launched and given back with what it came to; and an
//! owner reserves slots of a role ahead. The labels and keys of a consumer's work arrive with each request;
//! nothing here reads its orders.

use std::sync::LazyLock;

use regex::Regex;
use sea_orm::{ConnectionTrait, DatabaseConnection, QueryResult, TransactionTrait};
use serde::Serialize;
use store::{all, one, run};

use crate::agent_roles::{self, AgentRole};
use crate::order::Entry;
use crate::places::Ground;
use crate::road::{self, Ask, Row};

/// What a branch of a work order asks a slot as.
pub const COWORK: &str = "cowork";
/// What a job's run asks a slot as.
pub const JOB: &str = "job";
/// What a native `Agent` call of a job's own CLI asks a slot as.
pub const NATIVE: &str = "native";
/// What a supervisor's delegated task asks a slot as.
pub const DELEGATE: &str = "delegate";

/// The shape a request's, an owner's and a reservation's key is held to.
pub static KEY: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9:._/@-]{0,499}$").expect("a pattern"));
static SOURCE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9_-]{0,63}$").expect("a pattern"));

/// Why a slot step did not go through, in one English sentence.
pub type Fault = String;

fn faulted(why: impl std::fmt::Display) -> Fault {
  why.to_string()
}

/// The most slots a reservation holds.
pub const MOST_SEATS: i64 = 128;

/// A text the line keeps: trimmed, not empty, within its length and with no null in it.
pub fn text(value: &str, what: &str, most: usize) -> Result<String, Fault> {
  let clean = value.trim();

  if clean.is_empty() || clean.encode_utf16().count() > most || clean.contains('\0') {
    return Err(format!("Invalid {what}"));
  }

  Ok(clean.to_owned())
}

/// One slot request with its lease (docs/api.md, 「slots, reservations and places」).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
  pub sequence: i64,
  pub token: String,
  pub request_key: String,
  pub owner: String,
  pub source: String,
  pub role: String,
  /// The path its run will work at, where its caller names one.
  pub path: Option<String>,
  /// The provider, model and effort it is let in on; empty until it is, unless its caller fixed its road.
  pub provider: String,
  pub model: String,
  pub effort: Option<String>,
  /// The role's cap when it was let in.
  pub capacity: i64,
  pub state: String,
  pub enqueued_at: i64,
  pub leased_at: Option<i64>,
  pub child_id: Option<String>,
  pub ended_at: Option<i64>,
  pub outcome: Option<String>,
  pub reservation: Option<String>,
  pub yielding: bool,
  pub place_key: Option<String>,
  /// The session its run continues, where its caller named one.
  pub session: Option<String>,
  /// One line naming the work it is asked for, where its asker names one.
  pub title: Option<String>,
  /// The supervisor that asked for it, where its asker names one.
  pub asked_by: Option<String>,
  /// The run its child is, where its binding names one.
  pub agent_name: Option<String>,
  /// Why it waits where no provider of its road has quota, or none.
  pub held_for: Option<String>,
  /// Who gave it back — the token's holder, its owner, or the line on evidence its run ended — or none while
  /// it stands.
  pub ended_by: Option<String>,
}

/// Who gave a request back (docs/role-slots.md, 「The line」).
pub const BY_HOLDER: &str = "holder";
pub const BY_OWNER: &str = "owner";
pub const BY_RUN_ENDED: &str = "run-ended";

/// The job a request is the run of, where its source is a job's: a job's slot is asked under the owner
/// `job-<id>` (docs/api.md, 「slots, reservations and places」).
pub fn job_of(request: &Request) -> Option<i64> {
  (request.source == JOB)
    .then(|| request.owner.strip_prefix(JOB_OWNER)?.parse().ok())
    .flatten()
}

/// The owner a job's slot is asked under, before the job's id.
pub const JOB_OWNER: &str = "job-";

impl Request {
  /// Whether its caller fixed the road it goes out on, which it then waits for and never leaves.
  pub fn fixed(&self) -> bool {
    !self.provider.is_empty() && self.state == "waiting"
  }
}

fn optional_text(stored: String) -> Option<String> {
  (!stored.is_empty()).then_some(stored)
}

/// A request as its row holds it.
pub fn request_of(row: &QueryResult) -> Result<Request, Fault> {
  let get = |why: sea_orm::DbErr| why.to_string();

  Ok(Request {
    sequence: row.try_get("", "sequence").map_err(get)?,
    token: row.try_get("", "token").map_err(get)?,
    request_key: row.try_get("", "request_key").map_err(get)?,
    owner: row.try_get("", "owner").map_err(get)?,
    source: row.try_get("", "source").map_err(get)?,
    role: row.try_get("", "role").map_err(get)?,
    path: row.try_get("", "path").map_err(get)?,
    provider: row.try_get("", "provider").map_err(get)?,
    model: row.try_get("", "model").map_err(get)?,
    effort: optional_text(row.try_get("", "effort").map_err(get)?),
    capacity: row.try_get("", "capacity").map_err(get)?,
    state: row.try_get("", "state").map_err(get)?,
    enqueued_at: row.try_get("", "enqueued_at").map_err(get)?,
    leased_at: row.try_get("", "leased_at").map_err(get)?,
    child_id: row.try_get("", "child_id").map_err(get)?,
    ended_at: row.try_get("", "ended_at").map_err(get)?,
    outcome: row.try_get("", "outcome").map_err(get)?,
    reservation: row.try_get("", "reservation").map_err(get)?,
    yielding: row.try_get::<String>("", "class").map_err(get)? == "yielding",
    place_key: row.try_get("", "place_key").map_err(get)?,
    session: row.try_get("", "session").map_err(get)?,
    title: row.try_get("", "title").map_err(get)?,
    asked_by: row.try_get("", "asked_by").map_err(get)?,
    agent_name: row.try_get("", "agent").map_err(get)?,
    held_for: row.try_get("", "held_for").map_err(get)?,
    ended_by: row.try_get("", "ended_by").map_err(get)?,
  })
}

/// One owner's reservation of a role's slots.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reservation {
  pub token: String,
  pub reservation_key: String,
  pub state: String,
  pub owner: String,
  pub source: String,
  pub role: String,
  pub wanted: i64,
  pub created_at: i64,
}

/// A reservation as its row holds it.
pub fn reservation_of(row: &QueryResult) -> Result<Reservation, Fault> {
  let get = |why: sea_orm::DbErr| why.to_string();

  Ok(Reservation {
    token: row.try_get("", "token").map_err(get)?,
    reservation_key: row.try_get("", "reservation_key").map_err(get)?,
    state: row.try_get("", "state").map_err(get)?,
    owner: row.try_get("", "owner").map_err(get)?,
    source: row.try_get("", "source").map_err(get)?,
    role: row.try_get("", "role").map_err(get)?,
    wanted: row.try_get("", "wanted").map_err(get)?,
    created_at: row.try_get("", "created_at").map_err(get)?,
  })
}

/// What a slot is asked for: the role, the path its run works at, and the road its caller fixes where it
/// fixes one — a repository's pin, a delegation's supervisor's own, a native call's caller's provider, a
/// session's named road.
#[derive(Clone, Debug, Default)]
pub struct Wanted<'a> {
  pub role: &'a str,
  pub path: Option<&'a str>,
  pub fixed: Option<Entry>,
}

/// One slot asked of a role's line: under a key the asker names again, by whom, for what, and where only one
/// run at a time may stand.
#[derive(Clone, Debug, Default)]
pub struct Asked<'a> {
  pub request_key: String,
  pub owner: String,
  pub wanted: Wanted<'a>,
  pub yielding: bool,
  pub token: Option<&'a str>,
  pub place_key: Option<&'a str>,
  pub session: Option<&'a str>,
  pub title: Option<&'a str>,
  pub asked_by: Option<&'a str>,
}

/// The road a caller names for its slot: the provider and model, at the effort and thinking the role's order
/// names for that provider.
pub async fn named_road<C: ConnectionTrait>(
  db: &C,
  role: &str,
  provider: &str,
  model: &str,
) -> Result<Entry, Fault> {
  let held = role_of(db, role).await?;
  let own = held.order.entry(provider);

  Ok(Entry {
    provider: provider.to_owned(),
    model: model.to_owned(),
    effort: own.and_then(|entry| entry.effort.clone()),
    thinking: own.and_then(|entry| entry.thinking.clone()),
    answer_tokens: own.and_then(|entry| entry.answer_tokens),
  })
}

/// The provider a slot naming a model alone goes out on (docs/role-slots.md, 「Who takes a slot」): the first
/// entry of the role's order naming that model, else the role's active provider as the ask is made, else the
/// order's first.
pub async fn provider_for_model(
  ground: &crate::places::Ground,
  role: &str,
  model: &str,
) -> Result<String, Fault> {
  let held = role_of(&ground.db, role).await?;

  if let Some(entry) = held.order.entries.iter().find(|entry| entry.model == model) {
    return Ok(entry.provider.clone());
  }

  let ask = crate::road::Ask::of(crate::road::Row::Role(&held.name), &held.line, &held.order);
  let active = crate::road::resolve(ground, &ask)
    .await
    .ok()
    .and_then(|resolution| resolution.road().map(|road| road.provider.clone()));

  active
    .or_else(|| held.order.first().map(|entry| entry.provider.clone()))
    .ok_or_else(|| format!("The {role} role's order names no provider"))
}

/// The road a slot naming none is fixed to: for a role whose order is empty — the supervisor's double, the
/// delegator role, as seeded — the road of the supervisor the ask names as its asker, as it stands now: the
/// provider, model and effort its last turn went out on, else its order's first (docs/role-slots.md, 「A
/// delegated task」). A slot of such a role no supervisor asks is refused, since it has no road. A role whose
/// order holds an entry is fixed to nothing and walks that order, whoever asks it.
pub async fn fixed_road<C: ConnectionTrait>(
  db: &C,
  role: &str,
  asked_by: Option<&str>,
) -> Result<Option<Entry>, Fault> {
  let held = role_of(db, role).await?;

  if !held.order.follows_supervisor() {
    return Ok(None);
  }

  let supervisor = match asked_by {
    Some(name) => crate::supervisors::named(db, name).await.map_err(faulted)?,
    None => None,
  };
  let Some(supervisor) = supervisor else {
    return Err(format!(
      "The {role} role's order is empty, so it goes out on the road of the supervisor that asks it, and no \
       supervisor asked this slot"
    ));
  };
  let stands = supervisor
    .last_mode
    .clone()
    .filter(|mode| !mode.is_empty())
    .zip(supervisor.last_model.clone())
    .map(|(provider, model)| Entry {
      effort: supervisor.last_effort.clone().filter(|effort| !effort.is_empty()),
      thinking: supervisor.order.entry(&provider).and_then(|entry| entry.thinking.clone()),
      answer_tokens: supervisor.order.entry(&provider).and_then(|entry| entry.answer_tokens),
      provider,
      model,
    });

  stands
    .or_else(|| supervisor.order.first().cloned())
    .map(Some)
    .ok_or_else(|| format!("The supervisor {} names no provider to go out on", supervisor.name))
}

/// The request a token names.
pub async fn request<C: ConnectionTrait>(db: &C, token: &str) -> Result<Option<Request>, Fault> {
  one(db, r#"SELECT * FROM "role_slot_requests" WHERE "token" = $1"#, vec![token.into()])
    .await
    .map_err(faulted)?
    .as_ref()
    .map(request_of)
    .transpose()
}

/// The requests standing in the given states, in the order they came.
pub async fn requests<C: ConnectionTrait>(db: &C, states: &[&str]) -> Result<Vec<Request>, Fault> {
  let marks: Vec<String> = (1..=states.len()).map(|at| format!("${at}")).collect();
  let sql = format!(
    r#"SELECT * FROM "role_slot_requests" WHERE "state" IN ({}) ORDER BY "sequence""#,
    marks.join(",")
  );

  all(db, &sql, states.iter().map(|state| (*state).into()).collect())
    .await
    .map_err(faulted)?
    .iter()
    .map(request_of)
    .collect()
}

/// The role a slot is asked under, which the store must hold: an unknown role is an error, never a run on some
/// default (docs/role-slots.md, 「Who takes a slot」).
pub async fn role_of<C: ConnectionTrait>(db: &C, name: &str) -> Result<AgentRole, Fault> {
  agent_roles::named(db, name)
    .await
    .map_err(faulted)?
    .ok_or_else(|| agent_roles::unknown(name))
}

/// A slot asked of its role's line under the source its caller labels it with, or the one its token already
/// holds; `retain` keeps the token beside it in the same transaction, so no process dying between the two
/// leaves a request nobody owns.
pub async fn enqueue(
  db: &DatabaseConnection,
  asked: &Asked<'_>,
  source: &str,
  retain: impl AsyncFnOnce(&sea_orm::DatabaseTransaction, &Request) -> Result<(), Fault>,
) -> Result<Request, Fault> {
  if !KEY.is_match(&asked.request_key) || !KEY.is_match(&asked.owner) {
    return Err("Invalid slot request identity".to_owned());
  }

  if !SOURCE.is_match(source) {
    return Err("Invalid slot request source".to_owned());
  }

  let role = role_of(db, asked.wanted.role).await?;
  let place_key = asked.place_key.map(|key| text(key, "slot place key", 500)).transpose()?;
  let path = asked.wanted.path.map(|path| text(path, "slot path", 4096)).transpose()?;

  if let Some(token) = asked.token {
    let held = request(db, token).await?.ok_or("The slot request is gone")?;

    if held.request_key != asked.request_key || held.owner != asked.owner || held.role != role.name {
      return Err("The slot request does not match its token".to_owned());
    }

    if held.state == "released" || held.state == "cancelled" {
      return Err("The slot request is already finished".to_owned());
    }

    return Ok(held);
  }

  let fixed = asked.wanted.fixed.as_ref();
  let written = db.begin().await.map_err(faulted)?;

  let opened = run(
    &written,
    r#"INSERT INTO "role_slot_requests" ("token", "request_key", "owner", "source", "role", "path", "provider",
         "model", "effort", "capacity", "state", "enqueued_at", "class", "place_key", "session", "title",
         "asked_by")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'waiting', $11, $12, $13, $14, $15, $16)
       ON CONFLICT ("request_key") DO NOTHING"#,
    vec![
      uuid::Uuid::new_v4().to_string().into(),
      asked.request_key.clone().into(),
      asked.owner.clone().into(),
      source.into(),
      role.name.clone().into(),
      path.into(),
      fixed.map(|entry| entry.provider.clone()).unwrap_or_default().into(),
      fixed.map(|entry| entry.model.clone()).unwrap_or_default().into(),
      fixed.and_then(|entry| entry.effort.clone()).unwrap_or_default().into(),
      role.max_concurrent.into(),
      crate::now_ms().into(),
      (if asked.yielding { "yielding" } else { "foreground" }).into(),
      place_key.clone().into(),
      asked.session.map(str::to_owned).into(),
      asked.title.map(str::to_owned).into(),
      asked.asked_by.map(str::to_owned).into(),
    ],
  )
  .await
  .map_err(faulted)?;

  let held = one(
    &written,
    r#"SELECT * FROM "role_slot_requests" WHERE "request_key" = $1"#,
    vec![asked.request_key.clone().into()],
  )
  .await
  .map_err(faulted)?
  .as_ref()
  .map(request_of)
  .transpose()?
  .ok_or("The slot request was not written")?;

  if held.owner != asked.owner
    || held.source != source
    || held.role != role.name
    || held.place_key != place_key
  {
    return Err(format!("The slot request's identity changed: {}", asked.request_key));
  }

  // A key names one request for good: asked again while it stands it is answered with it, and once it was
  // given back the key is spent (docs/role-slots.md, 「The line」).
  if opened == 0 && (held.state == "released" || held.state == "cancelled") {
    return Err(format!(
      "The slot request {} was given back; a new slot is asked under a new request key",
      asked.request_key
    ));
  }

  retain(&written, &held).await?;
  written.commit().await.map_err(faulted)?;

  Ok(held)
}

/// The road a request goes out on if it is let in now: the road its caller fixed, or the first provider of its
/// role's order with quota above the role's share; why it waits otherwise.
pub async fn road_of(
  ground: &Ground,
  held: &Request,
  role: &AgentRole,
) -> Result<Result<Entry, String>, Fault> {
  let fixed = (!held.provider.is_empty()).then(|| Entry {
    provider: held.provider.clone(),
    model: held.model.clone(),
    effort: held.effort.clone(),
    thinking: role.order.entry(&held.provider).and_then(|entry| entry.thinking.clone()),
    answer_tokens: role.order.entry(&held.provider).and_then(|entry| entry.answer_tokens),
  });
  let ask = Ask {
    fixed: fixed.as_ref(),
    ..Ask::of(Row::Role(&role.name), &role.line, &role.order)
  };
  let resolution = road::resolve(ground, &ask).await?;

  Ok(match resolution.taken() {
    Some(step) => Ok(step.entry.clone()),
    None => Err(resolution.sentence()),
  })
}

/// The request let in where its turn has come, or none where it waits on: its owner asking for it, let in once
/// no request whose owner is asking stands ahead of it in its role's line, the role's cap has room and its road
/// has quota. A request whose road has none keeps its place and says why; one let in carries the provider,
/// model and effort it goes out on (docs/role-slots.md, 「The line」).
pub async fn acquire(ground: &Ground, held: &Request) -> Result<Option<Request>, Fault> {
  if held.state == "released" || held.state == "cancelled" {
    return Ok(None);
  }

  if held.state == "active" {
    return Ok(Some(held.clone()));
  }

  let role = role_of(&ground.db, &held.role).await?;
  let road = road_of(ground, held, &role).await?;
  let written = ground.db.begin().await.map_err(faulted)?;
  let admitted = admission(&written, &held.token, &role, road, crate::now_ms()).await?;

  written.commit().await.map_err(faulted)?;

  Ok(admitted)
}

/// A native call's request let in as `acquire` lets one in, on the road its caller fixed with no quota asked of
/// it: a native co-worker stands inside its caller's program, which already goes out on that road.
pub async fn acquire_inside(db: &DatabaseConnection, held: &Request) -> Result<Option<Request>, Fault> {
  if held.state == "released" || held.state == "cancelled" {
    return Ok(None);
  }

  if held.state == "active" {
    return Ok(Some(held.clone()));
  }

  if held.provider.is_empty() {
    return Err("A native call's slot names the road its caller stands on".to_owned());
  }

  let role = role_of(db, &held.role).await?;
  let entry = Entry {
    provider: held.provider.clone(),
    model: held.model.clone(),
    effort: held.effort.clone(),
    thinking: None,
    answer_tokens: None,
  };
  let written = db.begin().await.map_err(faulted)?;
  let admitted = admission(&written, &held.token, &role, Ok(entry), crate::now_ms()).await?;

  written.commit().await.map_err(faulted)?;

  Ok(admitted)
}

// Numbers a waiting request's ask in the line's one increasing order of asks, and answers the ask its owner
// made before this one, none where this is its first.
async fn ask<C: ConnectionTrait>(db: &C, token: &str) -> Result<Option<i64>, Fault> {
  let before =
    one(db, r#"SELECT "asked" FROM "role_slot_requests" WHERE "token" = $1"#, vec![token.into()])
      .await
      .map_err(faulted)?
      .map(|row| row.try_get::<Option<i64>>("", "asked"))
      .transpose()
      .map_err(faulted)?
      .flatten();

  run(
    db,
    r#"UPDATE "role_slot_requests" SET "asked" = nextval('role_slot_asks') WHERE "token" = $1"#,
    vec![token.into()],
  )
  .await
  .map_err(faulted)?;

  Ok(before)
}

async fn admission<C: ConnectionTrait>(
  db: &C,
  token: &str,
  role: &AgentRole,
  road: Result<Entry, String>,
  now: i64,
) -> Result<Option<Request>, Fault> {
  let Some(held) = request(db, token).await? else { return Ok(None) };

  if held.state == "active" {
    return Ok(Some(held));
  }

  // A reserved slot is let in by its reservation, never taken as a lease by whoever holds its token.
  if held.state != "waiting" || held.reservation.is_some() {
    return Ok(None);
  }

  let since = ask(db, token).await?;

  // A request whose road has no quota keeps its place, says why and stands in no one's way.
  let entry = match road {
    Ok(entry) => {
      run(
        db,
        r#"UPDATE "role_slot_requests" SET "held_for" = NULL WHERE "token" = $1"#,
        vec![token.into()],
      )
      .await
      .map_err(faulted)?;

      entry
    }
    Err(why) => {
      run(
        db,
        r#"UPDATE "role_slot_requests" SET "held_for" = $2 WHERE "token" = $1"#,
        vec![token.into(), why.into()],
      )
      .await
      .map_err(faulted)?;

      return Ok(None);
    }
  };

  if place_taken(db, held.place_key.as_deref()).await? {
    return Ok(None);
  }

  // A slot its own owner holds reserved in this role is the request's place: the reservation's slot is given
  // up and the request let in in its stead, which moves nothing in the count.
  let seat = one(
    db,
    r#"SELECT "token" FROM "role_slot_requests"
       WHERE "role" = $1 AND "owner" = $2 AND "state" = 'reserved' AND "reservation" IS NOT NULL
       ORDER BY "sequence" LIMIT 1"#,
    vec![held.role.clone().into(), held.owner.clone().into()],
  )
  .await
  .map_err(faulted)?;

  if let Some(seat) = seat {
    let seat: String = seat.try_get("", "token").map_err(faulted)?;

    end_seat(db, &seat, "released", &format!("taken by {token}"), now).await?;

    return admit(db, token, &entry, role.max_concurrent, now).await;
  }

  if held.yielding && foreground_waits(db, &held.role, since).await? {
    return Ok(None);
  }

  let inherits = match in_the_way(db, &held.role, held.sequence, since).await? {
    None => None,
    Some((first, Some(_), owner)) if owner == held.owner => Some(first),
    Some(_) => return Ok(None),
  };

  if used(db, &held.role).await? >= role.max_concurrent {
    return Ok(None);
  }

  if let Some(first) = inherits {
    end_seat(db, &first, "cancelled", &format!("turn given to {token}"), now).await?;
  }

  admit(db, token, &entry, role.max_concurrent, now).await
}

// Whether an active request already stands in this place.
async fn place_taken<C: ConnectionTrait>(db: &C, place_key: Option<&str>) -> Result<bool, Fault> {
  let Some(place_key) = place_key else { return Ok(false) };

  Ok(
    one(
      db,
      r#"SELECT 1 AS "taken" FROM "role_slot_requests" WHERE "place_key" = $1 AND "state" = 'active' LIMIT 1"#,
      vec![place_key.into()],
    )
    .await
    .map_err(faulted)?
    .is_some(),
  )
}

// Whether a prior request of the role waits whose owner is asking, which a yielding request gives way to.
async fn foreground_waits<C: ConnectionTrait>(
  db: &C,
  role: &str,
  since: Option<i64>,
) -> Result<bool, Fault> {
  Ok(
    one(
      db,
      r#"SELECT 1 AS "waits" FROM "role_slot_requests"
         WHERE "role" = $1 AND "state" = 'waiting' AND "class" = 'foreground' AND "held_for" IS NULL
           AND ("reservation" IS NOT NULL OR $2::bigint IS NULL OR "asked" > $2::bigint) LIMIT 1"#,
      vec![role.into(), since.into()],
    )
    .await
    .map_err(faulted)?
    .is_some(),
  )
}

// The first request of a role's line standing in the way of one asked for at `sequence`: one ahead of it that
// is a reservation's slot, or whose owner has asked since that request's own last ask `since` — every one
// ahead where it has not asked before. A request whose owner has not asked since, whose place key is taken
// or whose road has no quota is passed over and keeps its place.
pub(crate) async fn in_the_way<C: ConnectionTrait>(
  db: &C,
  role: &str,
  sequence: i64,
  since: Option<i64>,
) -> Result<Option<(String, Option<String>, String)>, Fault> {
  let row = one(
    db,
    r#"SELECT "token", "reservation", "owner" FROM "role_slot_requests" AS "waiting"
       WHERE "role" = $1 AND "state" = 'waiting' AND "class" = 'foreground' AND "held_for" IS NULL
         AND "sequence" < $2
         AND ("place_key" IS NULL OR NOT EXISTS (SELECT 1 FROM "role_slot_requests" AS "held"
           WHERE "held"."place_key" = "waiting"."place_key" AND "held"."state" = 'active'))
         AND ("reservation" IS NOT NULL OR $3::bigint IS NULL OR "asked" > $3::bigint)
       ORDER BY "sequence" LIMIT 1"#,
    vec![role.into(), sequence.into(), since.into()],
  )
  .await
  .map_err(faulted)?;

  row
    .map(|row| {
      Ok((
        row.try_get("", "token").map_err(faulted)?,
        row.try_get("", "reservation").map_err(faulted)?,
        row.try_get("", "owner").map_err(faulted)?,
      ))
    })
    .transpose()
}

/// How many slots of a role are taken: its runs let in and its reserved slots together.
pub async fn used<C: ConnectionTrait>(db: &C, role: &str) -> Result<i64, Fault> {
  let row = one(
    db,
    r#"SELECT COUNT(*) AS "count" FROM "role_slot_requests" WHERE "role" = $1 AND "state" IN ('active', 'reserved')"#,
    vec![role.into()],
  )
  .await
  .map_err(faulted)?;

  Ok(
    row
      .map(|row| row.try_get::<i64>("", "count"))
      .transpose()
      .map_err(faulted)?
      .unwrap_or(0),
  )
}

async fn admit<C: ConnectionTrait>(
  db: &C,
  token: &str,
  entry: &Entry,
  capacity: i64,
  now: i64,
) -> Result<Option<Request>, Fault> {
  let changed = run(
    db,
    r#"UPDATE "role_slot_requests" SET "state" = 'active', "leased_at" = $1, "provider" = $3, "model" = $4,
         "effort" = $5, "capacity" = $6, "held_for" = NULL
       WHERE "token" = $2 AND "state" = 'waiting'"#,
    vec![
      now.into(),
      token.into(),
      entry.provider.clone().into(),
      entry.model.clone().into(),
      entry.effort.clone().unwrap_or_default().into(),
      capacity.into(),
    ],
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
    r#"UPDATE "role_slot_requests" SET "state" = $1, "ended_at" = $2, "outcome" = $3, "ended_by" = $5
       WHERE "token" = $4 AND "state" IN ('waiting', 'reserved')"#,
    vec![
      state.into(),
      now.into(),
      text(outcome, "slot outcome", 500)?.into(),
      token.into(),
      // A seat the line ends is ended by its holder's own move: its reservation lowered, released or taken.
      BY_HOLDER.into(),
    ],
  )
  .await
  .map_err(faulted)
  .map(drop)
}

/// The run a launch carries a slot's lease into, written once: a lease no longer active, or bound to another
/// run, refuses it.
pub async fn bind(db: &DatabaseConnection, held: &Request, child_id: &str) -> Result<Request, Fault> {
  let clean = text(child_id, "slot child identity", 500)?;
  let written = db.begin().await.map_err(faulted)?;
  let now = request(&written, &held.token).await?.ok_or("Unknown slot lease")?;

  if now.state != "active" {
    return Err("Only an active slot lease may be bound".to_owned());
  }

  if now.child_id.as_ref().is_some_and(|bound| *bound != clean) {
    // A lease bound to the role session a start laid on it stays bound to it; the child a later binding names
    // is kept as the run it is.
    if now.session.is_some() && now.session == now.child_id {
      run(
        &written,
        r#"UPDATE "role_slot_requests" SET "agent" = COALESCE("agent", $1) WHERE "token" = $2"#,
        vec![clean.into(), held.token.clone().into()],
      )
      .await
      .map_err(faulted)?;

      let bound = request(&written, &held.token).await?.ok_or("Unknown slot lease")?;

      written.commit().await.map_err(faulted)?;

      return Ok(bound);
    }

    return Err("A slot lease's child cannot change".to_owned());
  }

  run(
    &written,
    r#"UPDATE "role_slot_requests" SET "child_id" = COALESCE("child_id", $1) WHERE "token" = $2 AND "state" = 'active'"#,
    vec![clean.into(), held.token.clone().into()],
  )
  .await
  .map_err(faulted)?;

  let bound = request(&written, &held.token).await?.ok_or("Unknown slot lease")?;

  written.commit().await.map_err(faulted)?;

  Ok(bound)
}

/// A launch held to the lease it was let in on: the same owner and role, bound to the run it starts.
pub async fn require(
  db: &DatabaseConnection,
  token: Option<&str>,
  owner: &str,
  role: &str,
  child_id: &str,
) -> Result<Request, Fault> {
  let token = token.ok_or("The launch holds no slot")?;
  let held = request(db, token).await?.filter(|held| held.state == "active");
  let held = held.ok_or("The launch holds no active slot")?;

  if held.owner != owner || held.role != role {
    return Err("The launch does not match its slot".to_owned());
  }

  bind(db, &held, child_id).await
}

/// One lease ended with what it came to — released, or cancelled where it never ran — and the reserved slots
/// of its role let in where room has come; whether it was still held.
pub async fn finish(
  db: &DatabaseConnection,
  token: &str,
  outcome: &str,
  cancel: bool,
) -> Result<bool, Fault> {
  finish_by(db, token, outcome, cancel, BY_HOLDER).await
}

/// One lease ended as `finish` ends it, kept with who gave it back.
pub async fn finish_by(
  db: &DatabaseConnection,
  token: &str,
  outcome: &str,
  cancel: bool,
  by: &str,
) -> Result<bool, Fault> {
  let clean = text(outcome, "slot outcome", 500)?;
  let now = crate::now_ms();
  let written = db.begin().await.map_err(faulted)?;
  let changed = run(
    &written,
    r#"UPDATE "role_slot_requests" SET "state" = $1, "ended_at" = $2, "outcome" = $3, "ended_by" = $5
       WHERE "token" = $4 AND "state" IN ('waiting', 'active', 'reserved')"#,
    vec![
      (if cancel { "cancelled" } else { "released" }).into(),
      now.into(),
      clean.into(),
      token.into(),
      by.into(),
    ],
  )
  .await
  .map_err(faulted)?;

  if changed != 1 {
    written.rollback().await.map_err(faulted)?;

    return Ok(false);
  }

  if let Some(held) = request(&written, token).await? {
    promote_role(&written, &held.role, now).await?;
  }

  written.commit().await.map_err(faulted)?;

  Ok(true)
}

/// Waiting slots a stopped order waited on given back, each cancelled with why.
pub async fn cancel_waiting(db: &DatabaseConnection, requests: &[String], why: &str) -> Result<(), Fault> {
  for request in requests {
    finish(db, request, why, true).await?;
  }

  Ok(())
}

/// One role's reserved slots let in, in the line's order, while none stands in the way of the next and the
/// role's cap has room. The pass is an ask of every such slot waiting in the role, its standing reservation
/// being its owner asking.
pub async fn promote_role<C: ConnectionTrait>(db: &C, role: &str, now: i64) -> Result<i64, Fault> {
  let Some(cap) = agent_roles::named(db, role).await.map_err(faulted)?.map(|role| role.max_concurrent)
  else {
    return Ok(0);
  };
  let seats = all(
    db,
    r#"SELECT "token", "sequence", "asked" FROM "role_slot_requests" AS "waiting"
       WHERE "role" = $1 AND "state" = 'waiting' AND "class" = 'foreground' AND "reservation" IS NOT NULL
         AND ("place_key" IS NULL OR NOT EXISTS (SELECT 1 FROM "role_slot_requests" AS "held"
           WHERE "held"."place_key" = "waiting"."place_key" AND "held"."state" = 'active'))
       ORDER BY "sequence""#,
    vec![role.into()],
  )
  .await
  .map_err(faulted)?;

  run(
    db,
    r#"UPDATE "role_slot_requests" SET "asked" = nextval('role_slot_asks')
       WHERE "role" = $1 AND "state" = 'waiting' AND "class" = 'foreground' AND "reservation" IS NOT NULL"#,
    vec![role.into()],
  )
  .await
  .map_err(faulted)?;

  let mut promoted = 0;

  for seat in seats {
    let token: String = seat.try_get("", "token").map_err(faulted)?;
    let since: Option<i64> = seat.try_get("", "asked").map_err(faulted)?;
    let sequence: i64 = seat.try_get("", "sequence").map_err(faulted)?;

    if in_the_way(db, role, sequence, since).await?.is_some() || used(db, role).await? >= cap {
      break;
    }

    run(
      db,
      r#"UPDATE "role_slot_requests" SET "state" = 'reserved', "leased_at" = $1, "capacity" = $3
         WHERE "token" = $2 AND "state" = 'waiting'"#,
      vec![now.into(), token.into(), cap.into()],
    )
    .await
    .map_err(faulted)?;
    promoted += 1;
  }

  Ok(promoted)
}

/// The line's own pass (docs/role-slots.md, 「Reserving slots」): every role's reserved slots let in where its
/// cap has room.
pub async fn promote_reservations<C: ConnectionTrait>(db: &C, now: i64) -> Result<i64, Fault> {
  let roles = all(
    db,
    r#"SELECT DISTINCT "role" FROM "role_slot_requests" WHERE "state" = 'waiting' AND "reservation" IS NOT NULL"#,
    vec![],
  )
  .await
  .map_err(faulted)?;
  let mut promoted = 0;

  for row in roles {
    let role: String = row.try_get("", "role").map_err(faulted)?;

    promoted += promote_role(db, &role, now).await?;
  }

  Ok(promoted)
}

/// What an owner's slots are held as: under a key the owner names again, by what brought them, for a role, so
/// many of them.
pub struct Seats<'a> {
  pub reservation_key: String,
  pub owner: String,
  pub source: &'a str,
  pub role: &'a str,
  pub count: i64,
}

/// Slots of a role held for an owner. Asked again with the same key, only its live slots move to the new
/// count — more join the line behind whoever stands in it, fewer give up the youngest first.
pub async fn reserve(db: &DatabaseConnection, seats: &Seats<'_>) -> Result<Reservation, Fault> {
  if !KEY.is_match(&seats.reservation_key) || !KEY.is_match(&seats.owner) {
    return Err("Invalid slot reservation identity".to_owned());
  }

  if !(0..=MOST_SEATS).contains(&seats.count) {
    return Err(format!("A reservation holds a whole number of slots from 0 to {MOST_SEATS}"));
  }

  let role = role_of(db, seats.role).await?;
  let count = seats.count;
  let now = crate::now_ms();
  let written = db.begin().await.map_err(faulted)?;

  run(
    &written,
    r#"INSERT INTO "role_slot_reservations" ("token", "reservation_key", "owner", "source", "role", "provider",
         "model", "effort", "wanted", "state", "created_at")
       VALUES ($1, $2, $3, $4, $5, '', '', '', $6, 'held', $7) ON CONFLICT ("reservation_key") DO NOTHING"#,
    vec![
      uuid::Uuid::new_v4().to_string().into(),
      seats.reservation_key.clone().into(),
      seats.owner.clone().into(),
      seats.source.into(),
      role.name.clone().into(),
      count.into(),
      now.into(),
    ],
  )
  .await
  .map_err(faulted)?;

  let held = reservation_by_key(&written, &seats.reservation_key).await?;

  if held.owner != seats.owner || held.source != seats.source || held.role != role.name {
    return Err(format!("The slot reservation's identity changed: {}", seats.reservation_key));
  }

  run(
    &written,
    r#"UPDATE "role_slot_reservations" SET "state" = 'held', "wanted" = $1, "ended_at" = NULL, "outcome" = NULL,
         "created_at" = CASE WHEN "state" = 'held' THEN "created_at" ELSE $3 END
       WHERE "token" = $2"#,
    vec![count.into(), held.token.clone().into(), now.into()],
  )
  .await
  .map_err(faulted)?;

  let live = live_seats(&written, &held.token).await?;
  let made: i64 = one(
    &written,
    r#"SELECT COUNT(*) AS "count" FROM "role_slot_requests" WHERE "reservation" = $1"#,
    vec![held.token.clone().into()],
  )
  .await
  .map_err(faulted)?
  .map(|row| row.try_get::<i64>("", "count"))
  .transpose()
  .map_err(faulted)?
  .unwrap_or(0);
  let live_count = i64::try_from(live.len()).unwrap_or(i64::MAX);

  for seat in (made + 1)..=(made + count - live_count) {
    run(
      &written,
      r#"INSERT INTO "role_slot_requests" ("token", "request_key", "owner", "source", "role", "provider", "model",
           "effort", "capacity", "state", "enqueued_at", "reservation")
         VALUES ($1, $2, $3, $4, $5, '', '', '', $6, 'waiting', $7, $8)"#,
      vec![
        uuid::Uuid::new_v4().to_string().into(),
        format!("{}:seat-{seat}", held.reservation_key).into(),
        held.owner.clone().into(),
        held.source.clone().into(),
        held.role.clone().into(),
        role.max_concurrent.into(),
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

  promote_role(&written, &held.role, now).await?;

  let reserved = reservation_by_token(&written, &held.token).await?;

  written.commit().await.map_err(faulted)?;

  Ok(reserved)
}

async fn reservation_by_key<C: ConnectionTrait>(db: &C, key: &str) -> Result<Reservation, Fault> {
  one(
    db,
    r#"SELECT * FROM "role_slot_reservations" WHERE "reservation_key" = $1"#,
    vec![key.into()],
  )
  .await
  .map_err(faulted)?
  .as_ref()
  .map(reservation_of)
  .transpose()?
  .ok_or_else(|| "The slot reservation was not written".to_owned())
}

async fn reservation_by_token<C: ConnectionTrait>(db: &C, token: &str) -> Result<Reservation, Fault> {
  one(db, r#"SELECT * FROM "role_slot_reservations" WHERE "token" = $1"#, vec![token.into()])
    .await
    .map_err(faulted)?
    .as_ref()
    .map(reservation_of)
    .transpose()?
    .ok_or_else(|| "The slot reservation is gone".to_owned())
}

async fn live_seats<C: ConnectionTrait>(db: &C, token: &str) -> Result<Vec<Request>, Fault> {
  all(
    db,
    r#"SELECT * FROM "role_slot_requests" WHERE "reservation" = $1 AND "state" IN ('waiting', 'reserved')
       ORDER BY "sequence""#,
    vec![token.into()],
  )
  .await
  .map_err(faulted)?
  .iter()
  .map(request_of)
  .collect()
}

/// The live slots of one reservation, oldest first.
pub async fn seats(db: &DatabaseConnection, token: &str) -> Result<Vec<Request>, Fault> {
  live_seats(db, token).await
}

/// A reservation given back: its waiting slots leave the line and its reserved ones free; whether it was
/// still held.
pub async fn release_reservation(
  db: &DatabaseConnection,
  token: &str,
  outcome: &str,
) -> Result<bool, Fault> {
  release_reservation_by(db, token, outcome, BY_HOLDER).await
}

/// A reservation given back as `release_reservation` gives it back, its slots kept with who gave them back.
pub async fn release_reservation_by(
  db: &DatabaseConnection,
  token: &str,
  outcome: &str,
  by: &str,
) -> Result<bool, Fault> {
  let clean = text(outcome, "slot outcome", 500)?;
  let now = crate::now_ms();
  let written = db.begin().await.map_err(faulted)?;
  let changed = run(
    &written,
    r#"UPDATE "role_slot_reservations" SET "state" = 'released', "ended_at" = $1, "outcome" = $2
       WHERE "token" = $3 AND "state" = 'held'"#,
    vec![now.into(), clean.clone().into(), token.into()],
  )
  .await
  .map_err(faulted)?;

  if changed != 1 {
    written.rollback().await.map_err(faulted)?;

    return Ok(false);
  }

  let held = reservation_by_token(&written, token).await?;

  for seat in live_seats(&written, token).await? {
    let state = if seat.state == "reserved" { "released" } else { "cancelled" };

    end_seat(&written, &seat.token, state, &clean, now).await?;
    run(
      &written,
      r#"UPDATE "role_slot_requests" SET "ended_by" = $1 WHERE "token" = $2"#,
      vec![by.into(), seat.token.clone().into()],
    )
    .await
    .map_err(faulted)?;
  }

  promote_role(&written, &held.role, now).await?;
  written.commit().await.map_err(faulted)?;

  Ok(true)
}

/// The reservations an owner holds, or every one held where none is named, oldest first.
pub async fn held_reservations(
  db: &DatabaseConnection,
  owner: Option<&str>,
) -> Result<Vec<Reservation>, Fault> {
  let rows = match owner {
    Some(owner) => {
      all(
        db,
        r#"SELECT * FROM "role_slot_reservations" WHERE "owner" = $1 AND "state" = 'held' ORDER BY "created_at", "token""#,
        vec![owner.into()],
      )
      .await
    }
    None => {
      all(
        db,
        r#"SELECT * FROM "role_slot_reservations" WHERE "state" = 'held' ORDER BY "created_at", "token""#,
        vec![],
      )
      .await
    }
  };

  rows.map_err(faulted)?.iter().map(reservation_of).collect()
}

// A process the line marked by its id and its start, alive while a process of that id started then stands.
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

/// Every active lease bound to a launched unit, pane or execution, as its token, its child and whether the line
/// has seen that child standing; the native calls, whose callers are read by their processes, and the leases
/// bound to a role session, which end with their session, left out.
pub async fn bound_leases(db: &DatabaseConnection) -> Result<Vec<(String, String, bool)>, Fault> {
  let rows = all(
    db,
    r#"SELECT "r"."token", "r"."child_id", "r"."child_seen" FROM "role_slot_requests" "r"
       WHERE "r"."state" = 'active' AND "r"."child_id" IS NOT NULL
         AND "r"."child_id" IS DISTINCT FROM "r"."session"
         AND NOT EXISTS (SELECT 1 FROM "role_slot_native" "n" WHERE "n"."token" = "r"."token")
       ORDER BY "r"."sequence""#,
    vec![],
  )
  .await
  .map_err(faulted)?;

  rows
    .iter()
    .map(|row| {
      Ok((
        row.try_get("", "token").map_err(faulted)?,
        row.try_get("", "child_id").map_err(faulted)?,
        row.try_get::<i64>("", "child_seen").map_err(faulted)? != 0,
      ))
    })
    .collect()
}

/// The bound leases read against what stands: a child standing is marked seen, and a child seen standing
/// before and standing no longer has its lease given back; a child never seen standing is left alone,
/// however long it takes to come up. How many were given back.
pub async fn release_ended(
  db: &DatabaseConnection,
  leases: &[(String, String, bool)],
  standing: &std::collections::HashSet<String>,
) -> Result<i64, Fault> {
  let mut released = 0;

  for (token, child, seen) in leases {
    if standing.contains(child) {
      if !seen {
        run(
          db,
          r#"UPDATE "role_slot_requests" SET "child_seen" = 1 WHERE "token" = $1"#,
          vec![token.clone().into()],
        )
        .await
        .map_err(faulted)?;
      }

      continue;
    }

    if *seen && finish_by(db, token, "its launched child no longer stands", false, BY_RUN_ENDED).await? {
      released += 1;
    }
  }

  Ok(released)
}

/// Every bound lease whose child was seen standing and has ended released, asked of what runs on this
/// machine; nothing where that could not be asked, since "nothing stands" and "nobody could say" are kept
/// apart.
pub async fn reclaim_bound(
  db: &DatabaseConnection,
  running: &crate::queue::running::Running,
) -> Result<i64, Fault> {
  let leases = bound_leases(db).await?;

  if leases.is_empty() {
    return Ok(0);
  }

  let children: Vec<String> = leases.iter().map(|(_, child, _)| child.clone()).collect();
  let Some(standing) = running.answered(&children).await.map_err(|why| why.detail)? else {
    return Ok(0);
  };

  release_ended(db, &leases, &standing).await
}

/// An active lease bound to the role session a start laid on it opened, whether or not it was bound before, the
/// session kept beside it, so the lease ends with that session and no longer with the one it was bound to
/// (docs/role-slots.md, 「The line」). The run name a binding named stays only while the lease stays on the same
/// session; a lease moved onto another forgets it, and its child is read afresh.
pub async fn bind_session(db: &DatabaseConnection, token: &str, session: &str) -> Result<(), Fault> {
  run(
    db,
    r#"UPDATE "role_slot_requests"
       SET "agent" = CASE WHEN "child_id" = $1 AND "session" = $1 THEN "agent" END,
           "child_seen" = CASE WHEN "child_id" = $1 AND "session" = $1 THEN "child_seen" ELSE 0 END,
           "child_id" = $1, "session" = $1
       WHERE "token" = $2 AND "state" = 'active'"#,
    vec![session.into(), token.into()],
  )
  .await
  .map_err(faulted)?;

  Ok(())
}

/// A bound lease marked as bound to the role session its child names, so it ends with that session.
pub async fn bound_to_session(db: &DatabaseConnection, token: &str) -> Result<(), Fault> {
  run(
    db,
    r#"UPDATE "role_slot_requests" SET "session" = "child_id" WHERE "token" = $1 AND "child_id" IS NOT NULL"#,
    vec![token.into()],
  )
  .await
  .map_err(faulted)?;

  Ok(())
}

/// Every active lease bound to one of these role sessions given back, as ended with its session; how many.
pub async fn release_sessions(
  db: &DatabaseConnection,
  sessions: &[String],
  why: &str,
) -> Result<i64, Fault> {
  let mut released = 0;

  for session in sessions {
    let rows = all(
      db,
      r#"SELECT "token" FROM "role_slot_requests"
         WHERE "state" = 'active' AND "session" = $1 AND "child_id" = $1"#,
      vec![session.clone().into()],
    )
    .await
    .map_err(faulted)?;

    for row in rows {
      let token: String = row.try_get("", "token").map_err(faulted)?;

      if finish_by(db, &token, why, false, BY_RUN_ENDED).await? {
        released += 1;
      }
    }
  }

  Ok(released)
}

/// Every active lease bound to a role session given back: what a start of the process finds, since a role
/// session is held in memory from its start to its end and no session of the process before stands.
pub async fn release_every_session(db: &DatabaseConnection) -> Result<i64, Fault> {
  let rows = all(
    db,
    r#"SELECT DISTINCT "session" FROM "role_slot_requests"
       WHERE "state" = 'active' AND "session" IS NOT NULL AND "child_id" = "session""#,
    vec![],
  )
  .await
  .map_err(faulted)?;
  let sessions: Vec<String> = rows
    .iter()
    .map(|row| row.try_get("", "session"))
    .collect::<Result<_, _>>()
    .map_err(faulted)?;

  release_sessions(db, &sessions, "its role session ended with the agency process that held it").await
}

/// What an owner's release gave back (docs/role-slots.md, 「An owner's release」).
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerReleased {
  pub cancelled: i64,
  pub released: i64,
  pub reservations: i64,
  /// The role sessions its leases were bound to, which the caller ends.
  pub sessions: Vec<String>,
}

/// Everything an owner holds given back as its owner's: each waiting or reserved request cancelled, each
/// active one released, each reservation released; the role sessions its active leases were bound to named
/// for the caller to end.
pub async fn release_owner(db: &DatabaseConnection, owner: &str) -> Result<OwnerReleased, Fault> {
  let why = "its owner gave back everything it held";
  let mut done = OwnerReleased::default();

  for held in held_reservations(db, Some(owner)).await? {
    if release_reservation_by(db, &held.token, why, BY_OWNER).await? {
      done.reservations += 1;
    }
  }

  let rows = all(
    db,
    r#"SELECT * FROM "role_slot_requests"
       WHERE "owner" = $1 AND "state" IN ('waiting', 'reserved', 'active') ORDER BY "sequence""#,
    vec![owner.into()],
  )
  .await
  .map_err(faulted)?;

  for row in &rows {
    let held = request_of(row)?;
    let active = held.state == "active";

    if active
      && let Some(session) = held.session.clone().filter(|session| held.child_id.as_ref() == Some(session))
    {
      done.sessions.push(session);
    }

    if finish_by(db, &held.token, why, !active, BY_OWNER).await? {
      if active {
        done.released += 1;
      } else {
        done.cancelled += 1;
      }
    }
  }

  Ok(done)
}

// How long a native call's stop mark is kept.
const STOP_MARK_MS: i64 = 24 * 60 * 60 * 1000;

/// Every native request whose waiter, while it waits, or whose caller, once let in, is gone, ended; and every
/// native reservation whose CLI is gone given back.
pub async fn reclaim_native(db: &DatabaseConnection) -> Result<i64, Fault> {
  let now = crate::now_ms();
  let rows = all(
    db,
    r#"SELECT "n"."token", "n"."waiter_pid", "n"."waiter_start", "n"."owner_pid", "n"."owner_start", "r"."state"
       FROM "role_slot_native" "n" JOIN "role_slot_requests" "r" ON "r"."token" = "n"."token"
       WHERE "r"."state" IN ('waiting', 'active')"#,
    vec![],
  )
  .await
  .map_err(faulted)?;
  let mut reclaimed = 0;

  for row in rows {
    let read = |key: &str| row.try_get::<Option<i64>>("", key).ok().flatten();
    let text = |key: &str| row.try_get::<Option<String>>("", key).ok().flatten();
    let token: String = row.try_get("", "token").map_err(faulted)?;
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

    if finish_by(db, &token, outcome, waiting, BY_RUN_ENDED).await? {
      reclaimed += 1;
    }
  }

  let reservations = all(
    db,
    r#"SELECT "n"."token", "n"."owner_pid", "n"."owner_start" FROM "role_slot_native_reservations" "n"
       JOIN "role_slot_reservations" "r" ON "r"."token" = "n"."token" WHERE "r"."state" = 'held'"#,
    vec![],
  )
  .await
  .map_err(faulted)?;

  for row in reservations {
    let token: String = row.try_get("", "token").map_err(faulted)?;
    let pid = row.try_get::<Option<i64>>("", "owner_pid").ok().flatten();
    let start = row.try_get::<Option<String>>("", "owner_start").ok().flatten();

    if alive(pid, start.as_deref()) {
      continue;
    }

    if release_reservation_by(db, &token, "the CLI that reserved these slots ended", BY_RUN_ENDED).await? {
      reclaimed += 1;
    }
  }

  run(
    db,
    r#"DELETE FROM "role_slot_native_stops" WHERE "at" < $1"#,
    vec![(now - STOP_MARK_MS).into()],
  )
  .await
  .map_err(faulted)?;

  Ok(reclaimed)
}
