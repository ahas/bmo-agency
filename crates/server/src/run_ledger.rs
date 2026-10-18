//! The run ledger (docs/api.md, 「Run log and run records」): one row of `runs` a run of an agent's unit,
//! opened as the job unit starts the unit and closed as it ends, its times in epoch seconds. A run keeps the
//! first closing it was given.

use sea_orm::{ConnectionTrait, QueryResult};
use serde::{Deserialize, Serialize};
use store::{one, run};

use crate::Refusal;

/// The search a run carried, as a caller names it: by its number or by its text.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum SearchId {
  Number(i64),
  Text(String),
}

impl SearchId {
  fn text(&self) -> String {
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
