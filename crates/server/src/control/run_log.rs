//! The run log and the run records read across the boundary (docs/api.md, 「Run log and run records」): a
//! run's lines after the last one a reader has, and the records kept under a run.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};
use store::all;

use super::{Call, Refused, Route, answer};

// The most lines one read answers.
const PAGE: i64 = 500;

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
