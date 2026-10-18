//! The run ledger (docs/api.md, 「Run log and run records」): one row of `runs` a run of an agent's unit,
//! opened as the job unit starts the unit and closed as it ends, its times in epoch seconds. A run keeps the
//! first closing it was given. A run's runner is left in its agent's host under the run's unit, and a
//! runner the host did not start closes its run as an error.

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
pub struct Opening {
  pub agent: String,
  #[serde(default)]
  pub chat_id: Option<i64>,
  pub unit: String,
  pub depth: u32,
  #[serde(default)]
  pub cap_ms: Option<i64>,
  pub started_at: i64,
  #[serde(default)]
  pub started_by: Option<String>,
  #[serde(default)]
  pub search_id: Option<SearchId>,
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
    r#"INSERT INTO "runs" ("agent", "chat_id", "unit", "depth", "cap_ms", "started_at", "started_by", "search_id")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING "id""#,
    vec![
      opening.agent.clone().into(),
      opening.chat_id.into(),
      opening.unit.clone().into(),
      i64::from(opening.depth).into(),
      opening.cap_ms.into(),
      opening.started_at.into(),
      opening.started_by.clone().into(),
      opening.search_id.as_ref().map(SearchId::text).into(),
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

/// A run by its id, or none where the ledger holds no row of it.
pub async fn read<C: ConnectionTrait>(db: &C, id: i64) -> Result<Option<Run>, Refusal> {
  one(
    db,
    r#"SELECT "id", "agent", "chat_id", "unit", "depth", "started_at", "finished_at", "outcome", "context_tokens"
       FROM "runs" WHERE "id" = $1"#,
    vec![id.into()],
  )
  .await?
  .as_ref()
  .map(Run::of)
  .transpose()
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
