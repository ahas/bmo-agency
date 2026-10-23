//! The run ledger (docs/api.md, 「runs」, docs/run-records.md, 「The run ledger」): one row of `runs` a run
//! of an agent's unit, opened as the job unit starts the unit and closed as it ends, or as a chat turn starts
//! and ends, its times in epoch seconds. A run keeps the first closing it was given; the message its answer
//! went out as is attached apart from the closing. A run's runner is left in its agent's host under the
//! run's unit, and a runner the host did not start closes its run as an error.

use std::collections::BTreeMap;

use sea_orm::{ConnectionTrait, QueryResult};
use serde::{Deserialize, Serialize};
use store::{all, one, run};

use crate::Refusal;
use crate::host_run::{self, Command, Left};
use crate::places::Ground;

/// The outcome a run is closed with where its runner never started.
const LAUNCH_ERROR: &str = "error";

/// The search a run carried, as a caller names it: by its number or by its text.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum SearchId {
  Number(i64),
  Text(String),
}

impl SearchId {
  pub fn text(&self) -> String {
    match self {
      Self::Number(number) => number.to_string(),
      Self::Text(text) => text.clone(),
    }
  }
}

/// What a run is opened with.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Opening {
  pub agent: String,
  #[serde(default, alias = "chat_id")]
  pub chat_id: Option<i64>,
  pub unit: String,
  pub depth: u32,
  #[serde(default, alias = "cap_ms")]
  pub cap_ms: Option<i64>,
  #[serde(alias = "started_at")]
  pub started_at: i64,
  #[serde(default, alias = "started_by")]
  pub started_by: Option<String>,
  #[serde(default, alias = "search_id")]
  pub search_id: Option<SearchId>,
  /// The round of the room the run belongs to, where the turn named one.
  #[serde(default)]
  pub round: Option<String>,
  /// The message the run is about until its answer's is attached.
  #[serde(default, alias = "message_id")]
  pub message_id: Option<i64>,
}

/// What a run is closed with.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Closing {
  pub finished_at: i64,
  pub outcome: String,
  #[serde(default)]
  pub context_tokens: Option<i64>,
}

/// One run of the ledger, as a caller reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Run {
  pub id: i64,
  pub agent: String,
  pub chat_id: Option<i64>,
  pub unit: String,
  pub depth: i64,
  pub started_at: i64,
  pub finished_at: Option<i64>,
  pub outcome: Option<String>,
  pub context_tokens: Option<i64>,
  pub round: Option<String>,
  pub message_id: Option<i64>,
}

impl Run {
  fn of(row: &QueryResult) -> Result<Self, Refusal> {
    Ok(Self {
      id: row.try_get("", "id")?,
      agent: row.try_get("", "agent")?,
      chat_id: row.try_get("", "chat_id")?,
      unit: row.try_get("", "unit")?,
      depth: row.try_get("", "depth")?,
      started_at: row.try_get("", "started_at")?,
      finished_at: row.try_get("", "finished_at")?,
      outcome: row.try_get("", "outcome")?,
      context_tokens: row.try_get("", "context_tokens")?,
      round: row.try_get("", "round")?,
      message_id: row.try_get("", "message_id")?,
    })
  }
}

/// How a closing came out short of closing the run.
#[derive(Debug)]
pub enum NotClosed {
  /// The ledger holds no run of that id.
  Unknown,
  /// The run was closed before, and keeps that closing.
  Closed,
  Store(Refusal),
}

impl From<Refusal> for NotClosed {
  fn from(refusal: Refusal) -> Self {
    Self::Store(refusal)
  }
}

/// Opens a run, answering its id.
pub async fn open<C: ConnectionTrait>(db: &C, opening: &Opening) -> Result<i64, Refusal> {
  let row = one(
    db,
    r#"INSERT INTO "runs"
         ("agent", "chat_id", "unit", "depth", "cap_ms", "started_at", "started_by", "search_id", "round", "message_id")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING "id""#,
    vec![
      opening.agent.clone().into(),
      opening.chat_id.into(),
      opening.unit.clone().into(),
      i64::from(opening.depth).into(),
      opening.cap_ms.into(),
      opening.started_at.into(),
      opening.started_by.clone().into(),
      opening.search_id.as_ref().map(SearchId::text).into(),
      opening.round.clone().into(),
      opening.message_id.into(),
    ],
  )
  .await?
  .ok_or_else(|| Refusal::failed("the run was not written"))?;

  Ok(row.try_get("", "id")?)
}

/// Closes a run that is still open.
pub async fn close<C: ConnectionTrait>(db: &C, id: i64, closing: &Closing) -> Result<(), NotClosed> {
  let closed = run(
    db,
    r#"UPDATE "runs" SET "finished_at" = $2, "outcome" = $3, "context_tokens" = $4
       WHERE "id" = $1 AND "finished_at" IS NULL"#,
    vec![
      id.into(),
      closing.finished_at.into(),
      closing.outcome.clone().into(),
      closing.context_tokens.into(),
    ],
  )
  .await
  .map_err(Refusal::from)?;

  if closed > 0 {
    return Ok(());
  }

  Err(match read(db, id).await? {
    Some(_) => NotClosed::Closed,
    None => NotClosed::Unknown,
  })
}

// What a run is read with.
const RUN_FIELDS: &str = r#""id", "agent", "chat_id", "unit", "depth", "started_at", "finished_at", "outcome",
  "context_tokens", "round", "message_id""#;

/// A run by its id, or none where the ledger holds no row of it.
pub async fn read<C: ConnectionTrait>(db: &C, id: i64) -> Result<Option<Run>, Refusal> {
  one(db, &format!(r#"SELECT {RUN_FIELDS} FROM "runs" WHERE "id" = $1"#), vec![id.into()])
    .await?
    .as_ref()
    .map(Run::of)
    .transpose()
}

/// The latest run of an agent kept under a unit, the name its log lines are written under, or none.
pub async fn of_unit<C: ConnectionTrait>(db: &C, agent: &str, unit: &str) -> Result<Option<Run>, Refusal> {
  one(
    db,
    &format!(
      r#"SELECT {RUN_FIELDS} FROM "runs" WHERE "unit" = $1 AND ($2 = '' OR "agent" = $2)
         ORDER BY "id" DESC LIMIT 1"#
    ),
    vec![unit.into(), agent.into()],
  )
  .await?
  .as_ref()
  .map(Run::of)
  .transpose()
}

/// What a list of runs is narrowed by: after an id, an agent, a room, a round, and names each matched
/// against a run's unit and its round; none narrows nothing.
#[derive(Clone, Debug, Default)]
pub struct Search {
  pub after: i64,
  pub limit: i64,
  pub agent: Option<String>,
  pub room: Option<i64>,
  pub round: Option<String>,
  pub names: Vec<String>,
}

/// The runs a search names, in id order.
pub async fn search<C: ConnectionTrait>(db: &C, search: &Search) -> Result<Vec<Run>, Refusal> {
  let names = serde_json::to_string(&search.names).unwrap_or_else(|_| "[]".to_owned());

  all(
    db,
    &format!(
      r#"SELECT {RUN_FIELDS} FROM "runs"
         WHERE "id" > $1
           AND ($2::text IS NULL OR "agent" = $2)
           AND ($3::bigint IS NULL OR "chat_id" = $3)
           AND ($4::text IS NULL OR "round" = $4)
           AND ($5::jsonb = '[]'::jsonb
                OR "unit" IN (SELECT jsonb_array_elements_text($5::jsonb))
                OR "round" IN (SELECT jsonb_array_elements_text($5::jsonb)))
         ORDER BY "id" LIMIT $6"#
    ),
    vec![
      search.after.into(),
      search.agent.clone().into(),
      search.room.into(),
      search.round.clone().into(),
      names.into(),
      search.limit.into(),
    ],
  )
  .await?
  .iter()
  .map(Run::of)
  .collect()
}

/// The message a run's answer went out as, attached on an open or a closed run alike; whether the ledger
/// holds the run.
pub async fn attach_message<C: ConnectionTrait>(db: &C, id: i64, message: i64) -> Result<bool, Refusal> {
  Ok(
    run(
      db,
      r#"UPDATE "runs" SET "message_id" = $2 WHERE "id" = $1"#,
      vec![id.into(), message.into()],
    )
    .await?
      > 0,
  )
}

/// The ids of the runs that carried a search, in id order; the search is matched as the text it is kept as.
pub async fn of_search<C: ConnectionTrait>(db: &C, search: &SearchId) -> Result<Vec<i64>, Refusal> {
  all(
    db,
    r#"SELECT "id" FROM "runs" WHERE "search_id" = $1 ORDER BY "id""#,
    vec![search.text().into()],
  )
  .await?
  .iter()
  .map(|row| Ok(row.try_get("", "id")?))
  .collect()
}

/// The program a run's runner is: its words, the folder it starts in and the variables it is handed.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Program {
  pub program: String,
  pub args: Vec<String>,
  pub cwd: String,
  pub environment: BTreeMap<String, String>,
}

impl From<Program> for Command {
  fn from(program: Program) -> Self {
    Self {
      args: std::iter::once(program.program).chain(program.args).collect(),
      cwd: program.cwd,
      env: program.environment,
      stdin: None,
    }
  }
}

/// What a run's runner is launched with: what a person watching reads it as, the agent whose host takes it,
/// and its program.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Launch {
  #[serde(default, alias = "runId")]
  pub run_id: Option<i64>,
  pub title: String,
  pub agent: String,
  pub command: Program,
}

/// How a launch came out short of starting the runner.
#[derive(Debug)]
pub enum NotLaunched {
  /// The ledger holds no run of that id.
  Unknown,
  /// The run is closed, and nothing was started for it.
  Closed,
  /// The host did not start the runner, for the reason its code names; the run is closed as an error.
  Refused(String),
  Store(Refusal),
}

impl From<Refusal> for NotLaunched {
  fn from(refusal: Refusal) -> Self {
    Self::Store(refusal)
  }
}

/// Leaves a run's runner in its agent's host, rooted on the run's unit, so the unit reads as alive while it
/// runs and a stop of it reaches the runner. The runner closes its own run; one the host did not start has
/// its run closed here as an error, finished now.
pub async fn launch(ground: &Ground, id: i64, launch: Launch) -> Result<(), NotLaunched> {
  let run = read(&ground.db, id).await?.ok_or(NotLaunched::Unknown)?;

  if run.finished_at.is_some() {
    return Err(NotLaunched::Closed);
  }

  let left = Left {
    agent: &launch.agent,
    for_agent: None,
    name: &run.unit,
    title: &launch.title,
    command: launch.command.into(),
  };
  let Err(code) = host_run::leave(ground, left).await else {
    return Ok(());
  };
  let closing = Closing {
    finished_at: crate::now_ms() / 1000,
    outcome: LAUNCH_ERROR.to_owned(),
    context_tokens: None,
  };

  if let Err(NotClosed::Store(why)) = close(&ground.db, id, &closing).await {
    eprintln!("{}: the run's row could not be closed: {}", run.unit, why.detail);
  }

  Err(NotLaunched::Refused(code))
}
