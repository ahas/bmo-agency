//! The work a harness hands the agency (docs/api.md, 「Assignments and jobs」): a worker held for an item,
//! the item handed over with its lease and run on that worker, and what a worker's session carries of the
//! item and its documents.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;

use agent_session::{
  JobMark, Mapping as WorkMapping, Resume, Scope, SessionId, Sessions, Start, Turn, TurnContext, WorkParts,
};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use sea_orm::{QueryResult, Value as DbValue};
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::control::claude_accounts::{Judging, carried_for_user, reopens_at, walk_for_user};
use crate::events::Named;
use crate::herdr_backend::turn_spent;
use crate::lines::{Trees, worker};
use crate::now_ms;
use crate::places::{AgentRow, Ground};
use crate::queue::routes::JOB_WORK_KEY;
use crate::run_ledger::{self, Closing, NotClosed, Opening, Spent};
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::worker_session::PROVIDER;
use store::{all, one, run};

/// The longest a worker is held for an item where the caller names no hold, and the longest it may be held.
const HELD_MS: i64 = 15 * 60 * 1000;
const HELD_MOST_MS: i64 = 24 * 60 * 60 * 1000;

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

fn missing(what: impl std::fmt::Display) -> Refused {
  Refused::new(StatusCode::NOT_FOUND, format!("no {what} stands"))
}

async fn body(call: &mut Call) -> Result<Value, Refused> {
  call.body().await
}

fn text<'a>(body: &'a Value, field: &str) -> Result<&'a str, Refused> {
  body
    .get(field)
    .and_then(Value::as_str)
    .filter(|value| !value.trim().is_empty())
    .ok_or_else(|| bad(format!("{field} (a string) is required")))
}

fn number(body: &Value, field: &str) -> Option<i64> {
  body.get(field).and_then(Value::as_i64)
}

fn page(items: Vec<Value>, more: bool) -> Value {
  json!({ "items": items, "more": more })
}

fn limit(call: &Call) -> usize {
  call
    .query("limit")
    .and_then(|limit| limit.parse::<usize>().ok())
    .unwrap_or(50)
    .clamp(1, 500)
}

/// Told to the streams: one event of the agency's own, its body the member as its route answers it.
pub fn publish(ground: &Ground, kind: &str, member: Value) {
  ground.events.publish_named(Named {
    id: 0,
    at: 0,
    name: Cow::Owned(kind.to_owned()),
    body: member,
  });
}

// ---- assignments ----

/// One assignment as its routes answer it.
fn assignment_value(row: &QueryResult) -> Result<Value, sea_orm::DbErr> {
  Ok(json!({
    "id": row.try_get::<i64>("", "id")?,
    "itemId": row.try_get::<i64>("", "item_id")?,
    "holderName": row.try_get::<String>("", "holder_name")?,
    "workerName": row.try_get::<String>("", "worker_name")?,
    "state": row.try_get::<String>("", "state")?,
    "expiresAt": row.try_get::<i64>("", "expires_at")?,
    "createdAt": row.try_get::<i64>("", "created_at")?,
    "changedAt": row.try_get::<i64>("", "changed_at")?,
  }))
}

const ASSIGNMENT_COLUMNS: &str = r#""id", "item_id", "holder_name", "worker_name", "state", "expires_at",
  "created_at", "changed_at""#;

async fn assignment(db: &sea_orm::DatabaseConnection, id: i64) -> Result<Value, Refused> {
  let sql = format!(r#"SELECT {ASSIGNMENT_COLUMNS} FROM "assignments" WHERE "id" = $1"#);
  let row = one(db, &sql, vec![id.into()]).await?;

  match row {
    Some(row) => Ok(assignment_value(&row)?),
    None => Err(missing(format!("assignment {id}"))),
  }
}

/// A hold that ran out is expired, and a running assignment whose job has ended is released, before
/// anything reads them.
async fn expire(db: &sea_orm::DatabaseConnection) -> Result<(), Refused> {
  let now = now_ms();
  run(
    db,
    r#"UPDATE "assignments" SET "state" = 'expired', "changed_at" = $1
       WHERE "state" = 'held' AND "expires_at" <= $1"#,
    vec![now.into()],
  )
  .await?;
  run(
    db,
    r#"UPDATE "assignments" a SET "state" = 'released', "changed_at" = $1
       WHERE a."state" = 'running'
         AND EXISTS (SELECT 1 FROM "jobs" j WHERE j."assignment_id" = a."id")
         AND NOT EXISTS (SELECT 1 FROM "jobs" j
               WHERE j."assignment_id" = a."id" AND j."state" IN ('starting', 'running'))"#,
    vec![now.into()],
  )
  .await?;

  Ok(())
}

/// That an assignment's job has ended: the worker it held is free.
async fn release(db: &sea_orm::DatabaseConnection, assignment_id: i64) -> Result<(), Refused> {
  run(
    db,
    r#"UPDATE "assignments" SET "state" = 'released', "changed_at" = $1 WHERE "id" = $2 AND "state" = 'running'"#,
    vec![now_ms().into(), assignment_id.into()],
  )
  .await?;

  Ok(())
}

/// Why no job can be launched now: every login that carries the job's work key is closed. None where one
/// is open, or where no login carries the key and the run goes out as it stands.
async fn logins_closed(ground: &Ground) -> Result<Option<String>, Refused> {
  let judging = Judging::of(ground);

  if !walk_for_user(judging, JOB_WORK_KEY, None).await?.is_empty() {
    return Ok(None);
  }

  let carried = carried_for_user(&ground.db, JOB_WORK_KEY, None).await?;

  if carried.is_empty() {
    return Ok(None);
  }

  let names = carried.iter().map(|account| account.id.as_str()).collect::<Vec<_>>().join(", ");

  Ok(Some(match reopens_at(judging, &carried).await? {
    Some(at) => format!(
      "no login can carry a job now ({names}); the first reopens at {}",
      chrono::DateTime::from_timestamp(at, 0).map_or_else(|| at.to_string(), |moment| moment.to_rfc3339())
    ),
    None => format!("no login can carry a job now ({names}); each waits for a person to sign in again"),
  }))
}

/// A worker held for an item for the harness that asked.
pub struct CreateAssignment;

impl Route for CreateAssignment {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/assignments";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let asked = body(&mut call).await?;
    let item_id = number(&asked, "itemId").ok_or_else(|| bad("itemId (a number) is required"))?;
    let holder = text(&asked, "holderName")?.trim().to_owned();
    if asked.get("item").is_none_or(Value::is_null) {
      return Err(bad("item is required"));
    }
    let assignee = asked
      .get("assigneeName")
      .and_then(Value::as_str)
      .map(str::trim)
      .filter(|name| !name.is_empty());
    let hold = number(&asked, "holdMs").unwrap_or(HELD_MS).clamp(1000, HELD_MOST_MS);
    let db = &call.ground().db;

    expire(db).await?;

    let sql = r#"SELECT a."name", a."cwd" FROM "agents" a
      WHERE a."kind" = 'worker' AND a."resting_since" IS NULL
        AND ($1 = '' OR a."name" = $1)
        AND NOT EXISTS (SELECT 1 FROM "assignments" held
              WHERE held."worker_name" = a."name" AND held."state" IN ('held', 'running'))
        AND NOT EXISTS (SELECT 1 FROM "jobs" j
              WHERE j."worker_name" = a."name" AND j."state" IN ('starting', 'running'))
      ORDER BY a."name" LIMIT 1"#;
    let picked = one(db, sql, vec![assignee.unwrap_or_default().into()]).await?;
    let Some(picked) = picked else {
      return Err(
        Refused::new(
          StatusCode::UNPROCESSABLE_ENTITY,
          if let Some(assignee) = assignee {
            format!("no worker {assignee} is free")
          } else {
            "no worker is free".to_owned()
          },
        )
        .with_code("no-worker-free"),
      );
    };
    let worker_name: String = picked.try_get("", "name")?;
    let now = now_ms();
    let row = one(
      db,
      r#"INSERT INTO "assignments" ("item_id", "holder_name", "worker_name", "state", "expires_at",
              "created_at", "changed_at")
         VALUES ($1, $2, $3, 'held', $4, $5, $5) RETURNING "id""#,
      vec![item_id.into(), holder.into(), worker_name.into(), (now + hold).into(), now.into()],
    )
    .await?
    .ok_or_else(|| Refused::failed("the assignment was written but not answered"))?;
    let id: i64 = row.try_get("", "id")?;
    let member = assignment(db, id).await?;

    publish(call.ground(), "assignment.created", member.clone());

    Ok(answer(StatusCode::CREATED, member))
  }
}

crate::moved!(CreateAssignment);

/// The assignments, by holder, worker or state.
pub struct ListAssignments;

impl Route for ListAssignments {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/assignments";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let db = &call.ground().db;
    expire(db).await?;

    let after = call.query("after").and_then(|after| after.parse::<i64>().ok()).unwrap_or(0);
    let limit = limit(&call);
    let holder = call.query("holderName").unwrap_or_default();
    let worker = call.query("workerName").unwrap_or_default();
    let state = call.query("state").unwrap_or_default();
    let sql = format!(
      r#"SELECT {ASSIGNMENT_COLUMNS} FROM "assignments"
         WHERE "id" > $1 AND ($2 = '' OR "holder_name" = $2) AND ($3 = '' OR "worker_name" = $3)
           AND ($4 = '' OR "state" = $4)
         ORDER BY "id" LIMIT $5"#
    );
    let rows = all(
      db,
      &sql,
      vec![after.into(), holder.into(), worker.into(), state.into(), ((limit + 1) as i64).into()],
    )
    .await?;
    let mut items: Vec<Value> = rows.iter().map(assignment_value).collect::<Result<_, _>>()?;
    let more = items.len() > limit;
    items.truncate(limit);

    Ok(answer(StatusCode::OK, page(items, more)))
  }
}

crate::moved!(ListAssignments);

/// One assignment.
pub struct ReadAssignment;

impl Route for ReadAssignment {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/assignments/{assignmentId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let id: i64 = call
      .param("assignmentId")
      .and_then(|id| id.parse().ok())
      .ok_or_else(|| bad("assignmentId is a number"))?;
    expire(&call.ground().db).await?;

    Ok(answer(StatusCode::OK, assignment(&call.ground().db, id).await?))
  }
}

crate::moved!(ReadAssignment);

/// Release an assignment, or hold it longer.
pub struct ChangeAssignment;

impl Route for ChangeAssignment {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/assignments/{assignmentId}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let id: i64 = call
      .param("assignmentId")
      .and_then(|id| id.parse().ok())
      .ok_or_else(|| bad("assignmentId is a number"))?;
    let asked = body(&mut call).await?;
    let db = &call.ground().db;
    let now = now_ms();
    let current = assignment(db, id).await?;
    let state = current["state"].as_str().unwrap_or_default();

    if state != "held" && state != "running" {
      return Err(Refused::new(StatusCode::CONFLICT, format!("assignment {id} is {state}")));
    }

    match asked.get("state").and_then(Value::as_str) {
      Some("released") => {
        run(
          db,
          r#"UPDATE "assignments" SET "state" = 'released', "changed_at" = $1 WHERE "id" = $2"#,
          vec![now.into(), id.into()],
        )
        .await?;
      }
      Some(other) => return Err(bad(format!("an assignment is released, not {other}"))),
      None => {}
    }

    if let Some(hold) = number(&asked, "holdMs").filter(|hold| *hold > 0) {
      let expires = now + hold.clamp(1000, HELD_MOST_MS);
      run(
        db,
        r#"UPDATE "assignments" SET "expires_at" = $1, "changed_at" = $2 WHERE "id" = $3"#,
        vec![expires.into(), now.into(), id.into()],
      )
      .await?;
    }

    let member = assignment(db, id).await?;
    publish(call.ground(), "assignment.changed", member.clone());

    Ok(answer(StatusCode::OK, member))
  }
}

crate::moved!(ChangeAssignment);

// ---- jobs ----

const JOB_COLUMNS: &str = r#""id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
  "session_id", "run_id", "run_name", "room_id", "state", "outcome", "report", "token_count",
  "input_tokens", "output_tokens", "cache_read_tokens", "cache_write_tokens", "instruction",
  "started_at", "ended_at", "prompt", "prompt_english""#;

/// What a job's turn spent, apart; none until the turn ends and where its road reported no figures.
fn usage_of(row: &QueryResult) -> Result<Value, sea_orm::DbErr> {
  let figure = |column: &str| row.try_get::<Option<i64>>("", column);

  Ok(
    match (
      figure("input_tokens")?,
      figure("output_tokens")?,
      figure("cache_read_tokens")?,
      figure("cache_write_tokens")?,
    ) {
      (Some(input), Some(output), Some(read), Some(write)) => json!({
        "inputTokens": input,
        "outputTokens": output,
        "cacheReadTokens": read,
        "cacheWriteTokens": write,
      }),
      _ => Value::Null,
    },
  )
}

/// One job as its routes answer it.
fn job_value(row: &QueryResult) -> Result<Value, sea_orm::DbErr> {
  Ok(json!({
    "id": row.try_get::<i64>("", "id")?,
    "itemId": row.try_get::<i64>("", "item_id")?,
    "leaseId": row.try_get::<i64>("", "lease_id")?,
    "holderName": row.try_get::<String>("", "holder_name")?,
    "assignmentId": row.try_get::<i64>("", "assignment_id")?,
    "workerName": row.try_get::<String>("", "worker_name")?,
    "sessionId": row.try_get::<Option<String>>("", "session_id")?,
    "runId": row.try_get::<Option<i64>>("", "run_id")?,
    "runName": row.try_get::<Option<String>>("", "run_name")?,
    "roomId": row.try_get::<Option<i64>>("", "room_id")?,
    "state": row.try_get::<String>("", "state")?,
    "outcome": row.try_get::<Option<String>>("", "outcome")?,
    "report": row.try_get::<Option<String>>("", "report")?,
    "tokenCount": row.try_get::<i64>("", "token_count")?,
    "usage": usage_of(row)?,
    "instruction": row
      .try_get::<Option<String>>("", "instruction")?
      .and_then(|text| serde_json::from_str::<Value>(&text).ok()),
    "startedAt": row.try_get::<i64>("", "started_at")?,
    "endedAt": row.try_get::<Option<i64>>("", "ended_at")?,
  }))
}

async fn job(db: &sea_orm::DatabaseConnection, id: i64) -> Result<(Value, String), Refused> {
  let sql = format!(r#"SELECT {JOB_COLUMNS} FROM "jobs" WHERE "id" = $1"#);
  let row = one(db, &sql, vec![id.into()]).await?;

  match row {
    Some(row) => Ok((job_value(&row)?, row.try_get("", "prompt")?)),
    None => Err(missing(format!("job {id}"))),
  }
}

// The four fields of a small-model instruction, in the order a prompt and a step say them.
const INSTRUCTION_LISTS: [&str; 3] = ["documents", "sentences", "checks"];

/// A small-model instruction as a caller names it (docs/worker-bench.md, 「A small-model instruction」):
/// its repository and its documents, sentences and checks, every one present and none empty; none where
/// it names none.
pub fn instruction_of(named: Option<&Value>) -> Result<Option<Value>, Refused> {
  let Some(named) = named.filter(|named| !named.is_null()) else {
    return Ok(None);
  };
  let repository = named
    .get("repository")
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|repository| !repository.is_empty())
    .ok_or_else(|| bad("instruction.repository (a string) is required"))?;
  let mut instruction = json!({ "repository": repository });

  for field in INSTRUCTION_LISTS {
    let entries = named
      .get(field)
      .and_then(Value::as_array)
      .filter(|entries| !entries.is_empty())
      .ok_or_else(|| bad(format!("instruction.{field} (a list that is not empty) is required")))?;
    let mut kept = Vec::with_capacity(entries.len());

    for entry in entries {
      match entry.as_str().map(str::trim).filter(|entry| !entry.is_empty()) {
        Some(entry) => kept.push(Value::from(entry)),
        None => return Err(bad(format!("instruction.{field}: every entry is a string that is not empty"))),
      }
    }

    instruction[field] = Value::Array(kept);
  }

  Ok(Some(instruction))
}

fn entries<'a>(instruction: &'a Value, field: &str) -> Vec<&'a str> {
  instruction[field]
    .as_array()
    .map(|entries| entries.iter().filter_map(Value::as_str).collect())
    .unwrap_or_default()
}

/// An instruction as a job's prompt renders it, ahead of the item's brief.
pub fn instruction_prompt(instruction: &Value) -> String {
  let mut text = format!(
    "## Instruction\n\nRepository: `{}`\n",
    instruction["repository"].as_str().unwrap_or_default()
  );
  let headings = [
    ("documents", "Read first, the canonical documents the change answers to:"),
    ("sentences", "The change must make or keep each of these true:"),
    ("checks", "It is done when each of these passes:"),
  ];

  for (field, heading) in headings {
    text.push_str(&format!("\n{heading}\n\n"));

    for entry in entries(instruction, field) {
      if field == "checks" {
        text.push_str(&format!("- `{entry}`\n"));
      } else {
        text.push_str(&format!("- {entry}\n"));
      }
    }
  }

  text
}

/// An instruction as the one line a work-order step is written as.
pub fn instruction_line(instruction: &Value) -> String {
  format!(
    "In `{}`, reading {}: {} Checked by {}.",
    instruction["repository"].as_str().unwrap_or_default(),
    entries(instruction, "documents").join(", "),
    entries(instruction, "sentences").join(" "),
    entries(instruction, "checks")
      .iter()
      .map(|check| format!("`{check}`"))
      .collect::<Vec<_>>()
      .join(", ")
  )
}

/// An instruction's lines as the gate's reference road reads them: its repository and each document,
/// sentence and check, one a line.
fn instruction_lines(instruction: &Value) -> Vec<String> {
  let mut lines = Vec::new();

  if let Some(repository) = instruction["repository"].as_str() {
    lines.push(repository.to_owned());
  }

  for field in INSTRUCTION_LISTS {
    lines.extend(entries(instruction, field).into_iter().map(str::to_owned));
  }

  lines
}

/// A prompt with the reference the gate picked attached, its entries under a heading of their own, each
/// named by its path and source; the prompt stands alone where no entry was picked.
fn with_reference(prompt: &str, entries: &[bmo_gate_client::Entry]) -> String {
  crate::peers::gate::attach(prompt, entries)
}

/// The prompt a job's worker is given, with the reference the gate picked over the job's instruction
/// attached ahead of the run; a job with no instruction, or a gate that picks nothing, stands as it was.
async fn referenced(ground: &Ground, instruction: Option<&Value>, prompt: &str) -> String {
  let Some(instruction) = instruction.filter(|instruction| !instruction.is_null()) else {
    return prompt.to_owned();
  };
  let lines = instruction_lines(instruction);

  if lines.is_empty() {
    return prompt.to_owned();
  }

  match ground.peers.gate.reference(lines).await {
    Ok(entries) => with_reference(prompt, &entries),
    Err(why) => {
      eprintln!("job: the gate's reference was not picked: {why}");

      prompt.to_owned()
    }
  }
}

/// The prompt a job's worker is given: the item as the harness read it, its instruction first where it
/// carries one.
fn prompt_of(item: &Value, instruction: Option<&Value>) -> String {
  let title = item.get("title").and_then(Value::as_str).unwrap_or("the item");
  let brief = item.get("brief").and_then(Value::as_str).unwrap_or_default();
  let mut prompt = format!("# {title}\n\n");

  if let Some(instruction) = instruction {
    prompt.push_str(&instruction_prompt(instruction));
    prompt.push('\n');
  }

  if !brief.trim().is_empty() {
    prompt.push_str(brief);
    prompt.push('\n');
  }

  if let Some(order) = item.get("workOrder").filter(|order| !order.is_null()) {
    prompt.push_str("\n## Work order\n\n");
    prompt.push_str(&serde_json::to_string_pretty(order).unwrap_or_default());
    prompt.push('\n');
  }

  if let Some(notes) = item.get("notes").and_then(Value::as_array).filter(|notes| !notes.is_empty()) {
    prompt.push_str("\n## Notes\n\n");
    for note in notes {
      let text = note.get("text").and_then(Value::as_str).or_else(|| note.as_str()).unwrap_or_default();
      prompt.push_str("- ");
      prompt.push_str(text);
      prompt.push('\n');
    }
  }

  prompt
}

/// Run an item on its assigned worker.
pub struct CreateJob;

impl Route for CreateJob {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/jobs";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let asked = body(&mut call).await?;
    let assignment_id =
      number(&asked, "assignmentId").ok_or_else(|| bad("assignmentId (a number) is required"))?;
    let item_id = number(&asked, "itemId").ok_or_else(|| bad("itemId (a number) is required"))?;
    let lease_id = number(&asked, "leaseId").ok_or_else(|| bad("leaseId (a number) is required"))?;
    let holder = text(&asked, "holderName")?.trim().to_owned();
    let item = asked.get("item").cloned().ok_or_else(|| bad("item is required"))?;
    let instruction = instruction_of(asked.get("instruction").or_else(|| item.get("instruction")))?;
    let db = &call.ground().db;
    let now = now_ms();

    expire(db).await?;
    let held = assignment(db, assignment_id).await?;
    let state = held["state"].as_str().unwrap_or_default();
    if state != "held" {
      return Err(Refused::new(StatusCode::CONFLICT, format!("assignment {assignment_id} is {state}")));
    }
    if held["itemId"].as_i64() != Some(item_id) || held["holderName"].as_str() != Some(holder.as_str()) {
      return Err(Refused::new(
        StatusCode::CONFLICT,
        "the assignment holds another item or another holder",
      ));
    }
    let worker_name: String = held["workerName"].as_str().unwrap_or_default().to_owned();
    // A job whose worker stands on another provider than Claude Code needs no Claude login to start: the
    // guard is about the login the job's work key runs on, and the key's walk stands on a registered
    // provider here.
    let (road, _) = worker_road(call.ground())
      .await
      .map_err(|why| Refused::new(StatusCode::SERVICE_UNAVAILABLE, why))?;
    if road == PROVIDER
      && let Some(why) = logins_closed(call.ground()).await?
    {
      return Err(Refused::new(StatusCode::SERVICE_UNAVAILABLE, why).with_code("no-login-open"));
    }
    let prompt = prompt_of(&item, instruction.as_ref());
    let room_id = number(&asked, "roomId").or_else(|| number(&item, "roomId"));
    let row = one(
      db,
      r#"INSERT INTO "jobs" ("item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
              "room_id", "state", "token_count", "started_at", "prompt", "instruction")
         VALUES ($1, $2, $3, $4, $5, $6, 'starting', 0, $7, $8, $9) RETURNING "id""#,
      vec![
        item_id.into(),
        lease_id.into(),
        holder.into(),
        assignment_id.into(),
        worker_name.clone().into(),
        room_id.map(Into::into).unwrap_or(DbValue::BigInt(None)),
        now.into(),
        prompt.clone().into(),
        instruction.as_ref().map(Value::to_string).into(),
      ],
    )
    .await?
    .ok_or_else(|| Refused::failed("the job was written but not answered"))?;
    let id: i64 = row.try_get("", "id")?;

    run(
      db,
      r#"UPDATE "assignments" SET "state" = 'running', "changed_at" = $1 WHERE "id" = $2"#,
      vec![now.into(), assignment_id.into()],
    )
    .await?;
    let (member, _) = job(db, id).await?;
    publish(call.ground(), "job.created", member.clone());

    let ground = call.shared_ground();
    let sessions = call.sessions();
    tokio::spawn(async move { launch(ground, sessions, id).await });

    Ok(answer(StatusCode::CREATED, member))
  }
}

crate::moved!(CreateJob);

/// The worker's road now: `claude-code` where its key stands on Claude, else the first enabled provider of
/// its fallback walk (`stand_in::walk_of`), with that provider's tier model. The same walk, order and
/// setting a chat turn crosses over on; the worker supplies only its own line.
async fn worker_road(ground: &Ground) -> Result<(String, super::tiers::Mapping), String> {
  let db = &ground.db;
  let claude = || async {
    super::tiers::resolve_execution(db, JOB_WORK_KEY, PROVIDER, None)
      .await
      .map(|m| (PROVIDER.to_owned(), m))
  };

  let Some(walk) = crate::stand_in::walk_of(ground, &ground.machine(), JOB_WORK_KEY).await else {
    return claude().await;
  };

  for provider in &walk.steps {
    if crate::control::harness::provider_enabled(db, provider).await.map_err(|why| why.error)?
      && let Ok(mapping) = super::tiers::resolve_execution(db, JOB_WORK_KEY, provider, None).await
    {
      return Ok((provider.clone(), mapping));
    }
  }

  // The walk named no enabled stand-in; the key goes out on Claude as it would have.
  claude().await
}

/// The work parts a job's turn is carried with.
async fn work_parts(
  ground: &Ground,
  worker_name: &str,
  room: i64,
  name: &str,
  title: &str,
) -> Result<WorkParts, String> {
  // The worker's road: Claude Code where its key stands on it, else the first enabled fallback step of its
  // walk (docs/cache-fallback.md). A worker whose primary (or Claude) is resting stands on its fallback by
  // the same walk a chat turn crosses over on, with no pin of its own.
  let (provider, mapping) = worker_road(ground).await?;
  let row = AgentRow::named(&ground.db, worker_name)
    .await
    .map_err(|why| why.detail)?
    .ok_or_else(|| format!("no worker is named {worker_name}"))?;
  let cwd = row.cwd.unwrap_or_else(|| ground.source.to_string_lossy().into_owned());
  let trees = Trees::of(ground);
  let mut line = worker::worker_run(ground, &trees, Some(worker_name), None, &mapping.model, &cwd)
    .await
    .map_err(|why| why.detail)?;
  line.effort = mapping.effort.clone().unwrap_or(line.effort);
  line.tier = Some(mapping.tier.clone());
  let mut variables = BTreeMap::new();
  variables.insert("BMO_DATA_DIR".to_owned(), ground.roots.data.to_string_lossy().into_owned());

  Ok(WorkParts {
    provider,
    model: mapping.model.clone(),
    mapping: WorkMapping {
      model: mapping.model,
      effort: mapping.effort,
      tier: Some(mapping.tier),
      answer_tokens: mapping.answer_tokens.and_then(|tokens| u64::try_from(tokens).ok()),
    },
    resume: Resume::Auto,
    cwd,
    name: name.to_owned(),
    title: title.to_owned(),
    chat: room,
    unit: Some(name.to_owned()),
    user: None,
    work_key: JOB_WORK_KEY.to_owned(),
    hears_words: true,
    variables,
  })
}

// The run of one job: the worker's session started, the item's turn carried, and the job's end written,
// its assignment released, and an end that is no answer noted in the job's log.
async fn launch(ground: Arc<Ground>, sessions: Option<crate::herdr_backend::Shared>, id: i64) {
  let ended = launched(&ground, sessions, id).await;
  let (state, outcome, report, spent) = match ended {
    Ok(end) => end,
    Err(why) => ("stopped".to_owned(), Some("failed".to_owned()), Some(why), None),
  };
  let now = now_ms();

  if let Some(outcome) = &outcome {
    noted(&ground, id, outcome, report.as_deref()).await;
  }

  if let Err(why) = run(
    &ground.db,
    r#"UPDATE "jobs" SET "state" = $1, "outcome" = $2, "report" = $3, "token_count" = $4, "ended_at" = $5,
         "input_tokens" = $7, "output_tokens" = $8, "cache_read_tokens" = $9, "cache_write_tokens" = $10
       WHERE "id" = $6"#,
    vec![
      state.clone().into(),
      outcome.clone().into(),
      report.clone().into(),
      spent.as_ref().map_or(0, Spent::total).into(),
      now.into(),
      id.into(),
      spent.map(|spent| spent.input_tokens).into(),
      spent.map(|spent| spent.output_tokens).into(),
      spent.map(|spent| spent.cache_read_tokens).into(),
      spent.map(|spent| spent.cache_write_tokens).into(),
    ],
  )
  .await
  {
    eprintln!("job {id}: its end was not written: {why}");
  }

  if let Ok((member, _)) = job(&ground.db, id).await {
    let assignment_id = member["assignmentId"].as_i64().unwrap_or_default();

    if let Err(why) = release(&ground.db, assignment_id).await {
      eprintln!("job {id}: its assignment {assignment_id} was not released: {}", why.error);
    }

    publish(&ground, if state == "ended" { "job.ended" } else { "job.changed" }, member);
  }
}

// A job's end that is no answer, written into its log under its run as a note: its outcome and why.
async fn noted(ground: &Ground, id: i64, outcome: &str, why: Option<&str>) {
  let Ok((member, _)) = job(&ground.db, id).await else {
    return;
  };
  let mut log = RunLog::new(Target {
    agent: member["workerName"].as_str().unwrap_or_default().to_owned(),
    run: member["runName"].as_str().map_or_else(|| format!("job-{id}"), str::to_owned),
    chat_id: member["roomId"].as_i64(),
    job_id: Some(id),
  });
  let text = match why.map(str::trim).filter(|why| !why.is_empty()) {
    Some(why) => format!("The job ended {outcome}: {why}"),
    None => format!("The job ended {outcome}, and its turn gave no reason."),
  };

  log.write(&ground.db, Line::note(text)).await;
}

type Ended = (String, Option<String>, Option<String>, Option<Spent>);

async fn launched(
  ground: &Ground,
  sessions: Option<crate::herdr_backend::Shared>,
  id: i64,
) -> Result<Ended, String> {
  let (member, prompt) = job(&ground.db, id).await.map_err(|why| why.error)?;
  let worker_name = member["workerName"].as_str().unwrap_or_default().to_owned();
  let room = member["roomId"].as_i64().unwrap_or(0);
  let title = prompt.lines().next().unwrap_or_default().trim_start_matches("# ").to_owned();
  let name = format!("job-{id}");
  let Some(sessions) = sessions else {
    return Err("agency stands no places to carry sessions on".to_owned());
  };

  // The gate picks the reference a job's instruction calls for, and the run carries it in its first
  // prompt; a job with no instruction stands as it was.
  let prompt = referenced(ground, member.get("instruction"), &prompt).await;

  let session = sessions
    .start(Start {
      agent: worker_name.clone(),
      scope: Scope::Worker,
    })
    .await
    .map_err(|fault| fault.message)?;
  let session_id = session.id.0;
  let parts = work_parts(ground, &worker_name, room, &name, &title).await?;
  let ledger = run_ledger::open(
    &ground.db,
    &Opening {
      agent: worker_name.clone(),
      chat_id: member["roomId"].as_i64(),
      unit: name.clone(),
      depth: 0,
      cap_ms: None,
      started_at: now_ms().div_euclid(1000),
      started_by: Some(member["holderName"].as_str().unwrap_or_default().to_owned()),
      search_id: None,
      round: None,
      message_id: None,
    },
  )
  .await
  .map_err(|why| why.detail)?;
  run(
    &ground.db,
    r#"UPDATE "jobs" SET "state" = 'running', "session_id" = $1, "run_name" = $2, "model" = $3, "run_id" = $5
       WHERE "id" = $4"#,
    vec![
      session_id.clone().into(),
      name.clone().into(),
      parts.mapping.model.clone().into(),
      id.into(),
      ledger.into(),
    ],
  )
  .await
  .map_err(|why| why.detail)?;
  if let Ok((member, _)) = job(&ground.db, id).await {
    publish(ground, "job.changed", member);
  }

  let sent = sessions
    .send_turn(Turn {
      session: SessionId(session_id),
      prompt,
      context: TurnContext {
        job: Some(JobMark {
          job: id,
          item: member["itemId"].as_i64(),
        }),
        work: Some(parts),
        ..TurnContext::default()
      },
    })
    .await;
  let end = match sent {
    Ok(end) => end,
    Err(fault) => {
      closed(ground, ledger, "failed", None, None).await;
      return Err(fault.message);
    }
  };

  let outcome = match end.outcome {
    agent_session::Outcome::Answered => None,
    other => Some(format!("{other:?}").to_lowercase()),
  };
  let spent = turn_spent(&end);

  closed(
    ground,
    ledger,
    outcome.as_deref().unwrap_or("answered"),
    end.reply.as_ref().and_then(|reply| reply.context_tokens),
    spent,
  )
  .await;

  let report = match outcome {
    None => end.answer,
    Some(_) => end.answer.or(end.reason),
  };

  Ok(("ended".to_owned(), outcome, report, spent))
}

// A job turn's run closed in the ledger as the turn ended, with what it spent.
async fn closed(
  ground: &Ground,
  ledger: i64,
  outcome: &str,
  context_tokens: Option<i64>,
  spent: Option<Spent>,
) {
  let closing = Closing {
    finished_at: now_ms().div_euclid(1000),
    outcome: outcome.to_owned(),
    context_tokens,
    spent,
  };

  if let Err(NotClosed::Store(why)) = run_ledger::close(&ground.db, ledger, &closing).await {
    eprintln!("run {ledger}: the job's run could not be closed in the ledger: {}", why.detail);
  }
}

/// The jobs: recent, by room, by worker, by holder, running.
pub struct ListJobs;

impl Route for ListJobs {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/jobs";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let db = &call.ground().db;
    let after = call.query("after").and_then(|after| after.parse::<i64>().ok()).unwrap_or(0);
    let limit = limit(&call);
    let room = call.query("roomId").and_then(|room| room.parse::<i64>().ok());
    let worker = call.query("workerName").unwrap_or_default();
    let holder = call.query("holderName").unwrap_or_default();
    let state = call.query("state").unwrap_or_default();
    let sql = format!(
      r#"SELECT {JOB_COLUMNS} FROM "jobs"
         WHERE "id" > $1 AND ($2 = '' OR "worker_name" = $2) AND ($3 = '' OR "holder_name" = $3)
           AND ($4 = '' OR "state" = $4) AND ($5::bigint IS NULL OR "room_id" = $5)
         ORDER BY "id" LIMIT $6"#
    );
    let rows = all(
      db,
      &sql,
      vec![
        after.into(),
        worker.into(),
        holder.into(),
        state.into(),
        room.map(Into::into).unwrap_or(DbValue::BigInt(None)),
        ((limit + 1) as i64).into(),
      ],
    )
    .await?;
    let mut items: Vec<Value> = rows.iter().map(job_value).collect::<Result<_, _>>()?;
    let more = items.len() > limit;
    items.truncate(limit);

    Ok(answer(StatusCode::OK, page(items, more)))
  }
}

crate::moved!(ListJobs);

/// One job and the tokens its model has spent.
pub struct ReadJob;

impl Route for ReadJob {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/jobs/{jobId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let id: i64 = call
      .param("jobId")
      .and_then(|id| id.parse().ok())
      .ok_or_else(|| bad("jobId is a number"))?;

    Ok(answer(StatusCode::OK, job(&call.ground().db, id).await?.0))
  }
}

crate::moved!(ReadJob);

/// End a job with its outcome, or stop it.
pub struct ChangeJob;

impl Route for ChangeJob {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/jobs/{jobId}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let id: i64 = call
      .param("jobId")
      .and_then(|id| id.parse().ok())
      .ok_or_else(|| bad("jobId is a number"))?;
    let asked = body(&mut call).await?;
    let db = &call.ground().db;
    let now = now_ms();
    let (member, _) = job(db, id).await?;
    let state = member["state"].as_str().unwrap_or_default();
    if state == "ended" || state == "stopped" {
      return Err(Refused::new(StatusCode::CONFLICT, format!("job {id} is {state}")));
    }

    let wanted = asked
      .get("state")
      .and_then(Value::as_str)
      .map(|state| match state {
        "ended" => Ok("ended"),
        "stopped" => Ok("stopped"),
        other => Err(bad(format!("a job ends or stops, not {other}"))),
      })
      .transpose()?
      .unwrap_or("ended");
    let outcome = asked.get("outcome").and_then(Value::as_str).map(str::to_owned);
    let report = asked.get("report").and_then(Value::as_str).map(str::to_owned);

    run(
      db,
      r#"UPDATE "jobs" SET "state" = $1, "outcome" = COALESCE($2, "outcome"),
              "report" = COALESCE($3, "report"), "ended_at" = $4 WHERE "id" = $5"#,
      vec![wanted.into(), outcome.into(), report.into(), now.into(), id.into()],
    )
    .await?;
    let assignment_id = member["assignmentId"].as_i64().unwrap_or_default();
    run(db, r#"UPDATE "assignments" SET "state" = 'released', "changed_at" = $1 WHERE "id" = $2 AND "state" = 'running'"#, vec![now.into(), assignment_id.into()]).await?;

    let (member, _) = job(db, id).await?;
    publish(
      call.ground(),
      if wanted == "ended" { "job.ended" } else { "job.changed" },
      member.clone(),
    );

    Ok(answer(StatusCode::OK, member))
  }
}

crate::moved!(ChangeJob);

/// The line a work-order step is written as, composed from an instruction.
pub struct InstructionText;

impl Route for InstructionText {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/instruction-texts";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let asked = body(&mut call).await?;
    let instruction = instruction_of(Some(&asked))?.ok_or_else(|| bad("an instruction is required"))?;

    Ok(answer(StatusCode::OK, json!({ "text": instruction_line(&instruction) })))
  }
}

crate::moved!(InstructionText);

/// The prompt a job was launched with.
pub struct ReadJobPrompt;

impl Route for ReadJobPrompt {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/jobs/{jobId}/prompt";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let id: i64 = call
      .param("jobId")
      .and_then(|id| id.parse().ok())
      .ok_or_else(|| bad("jobId is a number"))?;
    let row = one(
      &call.ground().db,
      r#"SELECT "prompt", "prompt_english" FROM "jobs" WHERE "id" = $1"#,
      vec![id.into()],
    )
    .await?
    .ok_or_else(|| missing(format!("job {id}")))?;

    Ok(answer(
      StatusCode::OK,
      json!({
        "jobId": id,
        "text": row.try_get::<String>("", "prompt")?,
        "english": row.try_get::<Option<String>>("", "prompt_english")?,
      }),
    ))
  }
}

crate::moved!(ReadJobPrompt);

// ---- session work and loads ----

fn results_of(row: Option<&QueryResult>) -> Result<Vec<Value>, Refused> {
  let Some(row) = row else { return Ok(Vec::new()) };
  let text: String = row.try_get("", "results")?;

  Ok(serde_json::from_str(&text).unwrap_or_default())
}

fn session_work_value(session: &str, row: Option<&QueryResult>) -> Result<Value, Refused> {
  let item = row.and_then(|row| row.try_get::<Option<i64>>("", "item_id").ok().flatten());

  Ok(json!({ "sessionId": session, "itemId": item, "results": results_of(row)? }))
}

async fn session_work_row(
  db: &sea_orm::DatabaseConnection,
  session: &str,
) -> Result<Option<QueryResult>, Refused> {
  Ok(
    one(
      db,
      r#"SELECT "item_id", "results" FROM "session_work" WHERE "session_id" = $1"#,
      vec![session.into()],
    )
    .await?,
  )
}

/// The item and results a worker session holds.
pub struct ReadSessionWork;

impl Route for ReadSessionWork {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/session-work/{sessionId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let session = call.param("sessionId").unwrap_or_default();
    let row = session_work_row(&call.ground().db, session).await?;

    Ok(answer(StatusCode::OK, session_work_value(session, row.as_ref())?))
  }
}

crate::moved!(ReadSessionWork);

/// Record the item a session serves.
pub struct ChangeSessionWork;

impl Route for ChangeSessionWork {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/session-work/{sessionId}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let session = call.param("sessionId").unwrap_or_default().to_owned();
    let asked = body(&mut call).await?;
    let item = number(&asked, "itemId");
    let db = &call.ground().db;
    let now = now_ms();

    run(
      db,
      r#"INSERT INTO "session_work" ("session_id", "item_id", "results", "changed_at")
         VALUES ($1, $2, '[]', $3)
         ON CONFLICT ("session_id") DO UPDATE SET "item_id" = EXCLUDED."item_id", "changed_at" = EXCLUDED."changed_at""#,
      vec![session.clone().into(), item.map(Into::into).unwrap_or(DbValue::BigInt(None)), now.into()],
    )
    .await?;
    let row = session_work_row(db, &session).await?;

    Ok(answer(StatusCode::OK, session_work_value(&session, row.as_ref())?))
  }
}

crate::moved!(ChangeSessionWork);

/// Record a result sent into a session.
pub struct PutSessionResult;

impl Route for PutSessionResult {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/session-work/{sessionId}/results/{resultKey}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let session = call.param("sessionId").unwrap_or_default().to_owned();
    let key = call.param("resultKey").unwrap_or_default().to_owned();
    let result = body(&mut call).await?;
    let db = &call.ground().db;
    let row = session_work_row(db, &session).await?;
    let mut results = results_of(row.as_ref())?;
    results.retain(|held| held.get("key").and_then(Value::as_str) != Some(key.as_str()));
    results.push(json!({ "key": key, "result": result }));
    let text = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_owned());

    run(
      db,
      r#"INSERT INTO "session_work" ("session_id", "results", "changed_at") VALUES ($1, $2, $3)
         ON CONFLICT ("session_id") DO UPDATE SET "results" = EXCLUDED."results", "changed_at" = EXCLUDED."changed_at""#,
      vec![session.clone().into(), text.into(), now_ms().into()],
    )
    .await?;
    let row = session_work_row(db, &session).await?;

    Ok(answer(StatusCode::OK, session_work_value(&session, row.as_ref())?))
  }
}

crate::moved!(PutSessionResult);

/// Forget a session's results as it folds.
pub struct DeleteSessionResults;

impl Route for DeleteSessionResults {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/session-work/{sessionId}/results";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let session = call.param("sessionId").unwrap_or_default();
    run(
      &call.ground().db,
      r#"UPDATE "session_work" SET "results" = '[]', "changed_at" = $1 WHERE "session_id" = $2"#,
      vec![now_ms().into(), session.into()],
    )
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(DeleteSessionResults);

async fn loads_value(db: &sea_orm::DatabaseConnection, session: &str) -> Result<Value, Refused> {
  let row = one(
    db,
    r#"SELECT "documents" FROM "session_loads" WHERE "session_id" = $1"#,
    vec![session.into()],
  )
  .await?;
  let documents: Vec<Value> = row
    .as_ref()
    .and_then(|row| row.try_get::<String>("", "documents").ok())
    .and_then(|text| serde_json::from_str(&text).ok())
    .unwrap_or_default();

  Ok(json!({ "sessionId": session, "documents": documents }))
}

/// The documents loaded into a session.
pub struct ReadSessionLoads;

impl Route for ReadSessionLoads {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/session-loads/{sessionId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let session = call.param("sessionId").unwrap_or_default();

    Ok(answer(StatusCode::OK, loads_value(&call.ground().db, session).await?))
  }
}

crate::moved!(ReadSessionLoads);

/// Keep a session's loads.
pub struct PutSessionLoads;

impl Route for PutSessionLoads {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/session-loads/{sessionId}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let session = call.param("sessionId").unwrap_or_default().to_owned();
    let asked = body(&mut call).await?;
    let documents = asked.get("documents").cloned().filter(Value::is_array).unwrap_or_else(|| json!([]));
    let db = &call.ground().db;
    let text = serde_json::to_string(&documents).unwrap_or_else(|_| "[]".to_owned());

    run(
      db,
      r#"INSERT INTO "session_loads" ("session_id", "documents", "changed_at") VALUES ($1, $2, $3)
         ON CONFLICT ("session_id") DO UPDATE SET "documents" = EXCLUDED."documents", "changed_at" = EXCLUDED."changed_at""#,
      vec![session.clone().into(), text.into(), now_ms().into()],
    )
    .await?;

    Ok(answer(StatusCode::OK, loads_value(db, &session).await?))
  }
}

crate::moved!(PutSessionLoads);

/// Forget a session's loads.
pub struct DeleteSessionLoads;

impl Route for DeleteSessionLoads {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/session-loads/{sessionId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let session = call.param("sessionId").unwrap_or_default();
    run(
      &call.ground().db,
      r#"DELETE FROM "session_loads" WHERE "session_id" = $1"#,
      vec![session.into()],
    )
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(DeleteSessionLoads);
