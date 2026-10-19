//! The run log and the run records across the boundary (docs/api.md, 「Run log and run records」): a run's
//! lines after the last one a reader has, a job's lines and its last one, the records kept under a run, and
//! a note and a turn record written by whoever carries the run.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bmo_chat::{ErrorCode, WireError};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use store::{all, one, run};

use super::{Call, Refused, Route, answer};
use crate::run_log::lines::{LINE_STATES, Line};
use crate::run_log::{MAX_TOOL_CHARS, RunLog, Target, preview};

// The most lines one read answers.
const PAGE: i64 = 500;

// A tool call's arguments as the log keeps them: whole in its detail where the line was cut.
const CALLED: &str = r#"COALESCE("detail", "text")"#;

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

/// A run's log lines, after the id given.
pub struct RunLogLines;

impl Route for RunLogLines {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/run-log";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let run = call.query("run").ok_or_else(|| bad("run is required"))?;
    let after = call.query("after").and_then(|after| after.parse::<i64>().ok()).unwrap_or(0);
    let rows = all(
      call.store(),
      r#"SELECT "id", "agent", "run", "chat_id", "at", "kind", "name", "parent", "ref", "text", "job_id", "state",
           "truncated", "detail"
         FROM "agent_log" WHERE "run" = $1 AND "id" > $2 ORDER BY "id" LIMIT $3"#,
      vec![run.into(), after.into(), PAGE.into()],
    )
    .await?;
    let mut lines = Vec::with_capacity(rows.len());

    for row in rows {
      let text = |name: &str| row.try_get::<Option<String>>("", name);
      let number = |name: &str| row.try_get::<Option<i64>>("", name);

      lines.push(json!({
        "id": row.try_get::<i64>("", "id")?,
        "agent": row.try_get::<String>("", "agent")?,
        "run": row.try_get::<String>("", "run")?,
        "chatId": number("chat_id")?,
        "at": row.try_get::<i64>("", "at")?,
        "kind": row.try_get::<String>("", "kind")?,
        "name": text("name")?,
        "parent": text("parent")?,
        "ref": text("ref")?,
        "text": row.try_get::<String>("", "text")?,
        "jobId": number("job_id")?,
        "state": text("state")?,
        "truncated": row.try_get::<i64>("", "truncated")? != 0,
        "detail": text("detail")?,
      }));
    }

    Ok(answer(StatusCode::OK, json!({ "lines": lines })))
  }
}

crate::moved!(RunLogLines);

/// The records kept under a run.
pub struct RunRecords;

impl Route for RunRecords {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/run-records";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let run = call.query("run").ok_or_else(|| bad("run is required"))?;
    let rows = all(
      call.store(),
      r#"SELECT "id", "kind", "text", "record_id", "wakes", "at" FROM "run_records" WHERE "run" = $1 ORDER BY "id""#,
      vec![run.into()],
    )
    .await?;
    let mut records: Vec<Value> = Vec::with_capacity(rows.len());

    for row in rows {
      records.push(json!({
        "id": row.try_get::<i64>("", "id")?,
        "kind": row.try_get::<String>("", "kind")?,
        "text": row.try_get::<String>("", "text")?,
        "recordId": row.try_get::<Option<i64>>("", "record_id")?,
        "wakes": row.try_get::<i64>("", "wakes")?,
        "at": row.try_get::<i64>("", "at")?,
      }));
    }

    Ok(answer(StatusCode::OK, json!({ "records": records })))
  }
}

crate::moved!(RunRecords);

// The job the path names by its id.
fn job_of(call: &Call) -> Result<i64, Refused> {
  call
    .param("job")
    .and_then(|job| job.parse().ok())
    .ok_or_else(|| bad("a job's id is a whole number"))
}

async fn read_body<T: DeserializeOwned>(call: &mut Call, what: &str) -> Result<T, Refused> {
  serde_json::from_value(call.body().await?).map_err(|error| bad(format!("{what}: {error}")))
}

/// A job's last line.
pub struct JobLastLine;

impl Route for JobLastLine {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/log/jobs/{job}/last";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let job = job_of(&call)?;
    let row = one(
      call.store(),
      r#"SELECT "id", "at", "kind", "name" FROM "agent_log" WHERE "job_id" = $1 ORDER BY "id" DESC LIMIT 1"#,
      vec![job.into()],
    )
    .await?;
    let Some(row) = row else {
      return Ok(
        WireError::new(ErrorCode::NothingHeld, format!("job {job} wrote no line")).into_response(),
      );
    };

    Ok(answer(
      StatusCode::OK,
      json!({
        "id": row.try_get::<i64>("", "id")?,
        "at": row.try_get::<i64>("", "at")?,
        "kind": row.try_get::<String>("", "kind")?,
        "name": row.try_get::<Option<String>>("", "name")?,
      }),
    ))
  }
}

crate::moved!(JobLastLine);

// The command a call's arguments name, where they read as JSON naming one.
fn command_of(arguments: &str) -> Option<String> {
  let read: Value = serde_json::from_str(arguments).ok()?;

  read.get("command").and_then(Value::as_str).map(str::to_owned)
}

/// A job's top-level tool calls and their results, with how many model requests its runs made and how
/// often its context was compacted.
pub struct JobLines;

impl Route for JobLines {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/log/jobs/{job}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let job = job_of(&call)?;
    let db = call.store();
    let called = all(
      db,
      &format!(
        r#"SELECT "id", "agent", "run", "ref", "name", "source", "at", {CALLED} AS "arguments"
           FROM "agent_log" WHERE "job_id" = $1 AND "kind" = 'tool' AND "parent" IS NULL ORDER BY "id""#
      ),
      vec![job.into()],
    )
    .await?;
    let answered = all(
      db,
      r#"SELECT "id", "agent", "run", "ref", "detail_complete", "detail_length", "at"
         FROM "agent_log" WHERE "job_id" = $1 AND "kind" = 'tool-result' AND "parent" IS NULL ORDER BY "id""#,
      vec![job.into()],
    )
    .await?;
    let counted = one(
      db,
      r#"SELECT
           (SELECT COUNT(DISTINCT "source_id") FROM "agent_log"
              WHERE "job_id" = $1 AND "parent" IS NULL AND "kind" IN ('tool', 'text', 'thinking')) AS "requests",
           (SELECT COUNT(*) FROM "agent_log"
              WHERE "job_id" = $1 AND "parent" IS NULL AND "kind" = 'compact') AS "compactions""#,
      vec![job.into()],
    )
    .await?;
    let mut calls = Vec::with_capacity(called.len());
    let mut results = Vec::with_capacity(answered.len());

    for row in called {
      let arguments = row.try_get::<String>("", "arguments")?;

      calls.push(json!({
        "id": row.try_get::<i64>("", "id")?,
        "agent": row.try_get::<String>("", "agent")?,
        "run": row.try_get::<String>("", "run")?,
        "reference": row.try_get::<Option<String>>("", "ref")?,
        "name": row.try_get::<Option<String>>("", "name")?.unwrap_or_default(),
        "source": row.try_get::<Option<String>>("", "source")?,
        "at": row.try_get::<i64>("", "at")?,
        "command": command_of(&arguments),
        "args": preview(&arguments, Some(MAX_TOOL_CHARS)).0,
      }));
    }

    for row in answered {
      results.push(json!({
        "id": row.try_get::<i64>("", "id")?,
        "agent": row.try_get::<String>("", "agent")?,
        "run": row.try_get::<String>("", "run")?,
        "reference": row.try_get::<Option<String>>("", "ref")?,
        "complete": row.try_get::<i64>("", "detail_complete")? != 0,
        "length": row.try_get::<i64>("", "detail_length")?,
        "at": row.try_get::<i64>("", "at")?,
      }));
    }

    let count = |name: &str| counted.as_ref().map_or(Ok(0), |row| row.try_get::<i64>("", name));

    Ok(answer(
      StatusCode::OK,
      json!({
        "calls": calls,
        "results": results,
        "requests": count("requests")?,
        "compactions": count("compactions")?,
      }),
    ))
  }
}

crate::moved!(JobLines);

/// A note as whoever carries a run writes it.
#[derive(Deserialize)]
struct Note {
  agent: String,
  run: String,
  chat: Option<i64>,
  job: Option<i64>,
  text: String,
  source: Option<String>,
  state: Option<String>,
}

/// A note in the bot's own words written into a run's log.
pub struct WriteNote;

impl Route for WriteNote {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/log/notes";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let note: Note = read_body(&mut call, "a note").await?;
    let state = match note.state.as_deref() {
      None => None,
      Some(state) => Some(
        LINE_STATES
          .into_iter()
          .find(|known| *known == state)
          .ok_or_else(|| bad(format!("{state} is none of the states a line stands in")))?,
      ),
    };

    if note.agent.is_empty() || note.run.is_empty() {
      return Err(bad("a note names the agent and the run it is kept under"));
    }

    let mut log = RunLog::new(Target {
      agent: note.agent,
      run: note.run,
      chat_id: note.chat,
      job_id: note.job,
    });
    let line = Line {
      source: note.source,
      state,
      ..Line::note(note.text)
    };

    match log.write(call.store(), line).await {
      Some(_) => Ok(StatusCode::NO_CONTENT.into_response()),
      None => Err(Refused::failed("the note could not be written")),
    }
  }
}

crate::moved!(WriteNote);

/// A turn record as the run that wrote it down hands it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnRecord {
  run: String,
  kind: String,
  text: String,
  record_id: Option<i64>,
  wakes: bool,
  at: i64,
}

/// A turn record kept under the name its run worked under.
pub struct KeepTurnRecord;

impl Route for KeepTurnRecord {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/turn-records";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let record: TurnRecord = read_body(&mut call, "a turn record").await?;

    if record.run.is_empty() || record.kind.is_empty() {
      return Err(bad("a turn record names its run and its kind"));
    }

    run(
      call.store(),
      r#"INSERT INTO "run_records" ("run", "kind", "text", "record_id", "wakes", "at")
         VALUES ($1, $2, $3, $4, $5, $6)"#,
      vec![
        record.run.into(),
        record.kind.into(),
        record.text.into(),
        record.record_id.into(),
        i64::from(record.wakes).into(),
        record.at.into(),
      ],
    )
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(KeepTurnRecord);
