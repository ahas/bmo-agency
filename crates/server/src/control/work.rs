//! The work a harness hands the agency (docs/api.md, 「Assignments and jobs」): a worker held for an item,
//! the item handed over with its lease and run on that worker, and what a worker's session carries of the
//! item and its documents.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;

use agent_session::{
  Apart, JobMark, Mapping as WorkMapping, Resume, Scope, SessionId, Sessions, Start, Turn, TurnContext,
  TurnEnd, WorkParts,
};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use sea_orm::{QueryResult, TransactionTrait, Value as DbValue};
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::events::Named;
use crate::herdr_backend::turn_spent;
use crate::lines::{Trees, worker};
use crate::now_ms;
use crate::order::Entry;
use crate::order_run::dispatch::{self, Request};
use crate::places::{AgentRow, Ground};
use crate::queue::routes::JOB_WORK_KEY;
use crate::repositories::{AGENT_LABEL, Pin, ROLE_LABEL};
use crate::road::{self, Closed, Road};
use crate::run_ledger::{self, Closing, NotClosed, Opening, Spent};
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use store::{all, one, run};

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
    "roleName": row.try_get::<String>("", "role")?,
    "state": row.try_get::<String>("", "state")?,
    "createdAt": row.try_get::<i64>("", "created_at")?,
    "changedAt": row.try_get::<i64>("", "changed_at")?,
  }))
}

const ASSIGNMENT_COLUMNS: &str =
  r#""id", "item_id", "holder_name", "worker_name", "role", "state", "created_at", "changed_at""#;

async fn assignment(db: &sea_orm::DatabaseConnection, id: i64) -> Result<Value, Refused> {
  let sql = format!(r#"SELECT {ASSIGNMENT_COLUMNS} FROM "assignments" WHERE "id" = $1"#);
  let row = one(db, &sql, vec![id.into()]).await?;

  match row {
    Some(row) => {
      let mut value = assignment_value(&row)?;

      name_repositories(db, std::slice::from_mut(&mut value)).await?;
      Ok(value)
    }
    None => Err(missing(format!("assignment {id}"))),
  }
}

/// Each assignment's or job's `repositoryName` beside its `workerName`: the repository its checkout is of, or
/// null for a checkout no repository holds any longer (docs/api.md, 「assignments and jobs」).
async fn name_repositories(db: &sea_orm::DatabaseConnection, members: &mut [Value]) -> Result<(), Refused> {
  let repositories = crate::queue::workers::repositories_by_checkout(db).await?;

  for member in members {
    let repository = member["workerName"].as_str().and_then(|checkout| repositories.get(checkout)).cloned();

    member["repositoryName"] = json!(repository);
  }

  Ok(())
}

/// The checkouts of the repository a list is narrowed to, as the list's query parameter, or none where it is
/// narrowed to none.
async fn checkouts_named(
  db: &sea_orm::DatabaseConnection,
  repository: Option<&str>,
) -> Result<Option<String>, Refused> {
  let Some(repository) = repository else { return Ok(None) };
  let checkouts: Vec<String> = crate::queue::workers::repositories_by_checkout(db)
    .await?
    .into_iter()
    .filter(|(_, of)| of == repository)
    .map(|(checkout, _)| checkout)
    .collect();

  Ok(Some(serde_json::to_string(&checkouts).unwrap_or_else(|_| "[]".to_owned())))
}

/// What opens the holder's name of an assignment a job took for itself, the job's number after it: a checkout it
/// borrowed from another repository.
pub const JOB_HOLDER: &str = "job:";

/// A running assignment whose job has ended is released before anything reads them, and so is one a job borrowed
/// that no longer runs. Any other held one stands until its job starts or its harness releases it: no clock lets
/// a hold go.
async fn release_ended(db: &sea_orm::DatabaseConnection) -> Result<(), Refused> {
  let now = now_ms();
  run(
    db,
    r#"UPDATE "assignments" a SET "state" = 'released', "changed_at" = $1
       WHERE a."state" = 'running'
         AND EXISTS (SELECT 1 FROM "jobs" j WHERE j."assignment_id" = a."id")
         AND NOT EXISTS (SELECT 1 FROM "jobs" j
               WHERE j."assignment_id" = a."id"
                 AND (j."state" IN ('starting', 'running') OR (j."state" = 'reviewing' AND NOT j."review_parked")))"#,
    vec![now.into()],
  )
  .await?;
  // A checkout a job borrowed is lent for as long as that job runs (docs/worker-slots.md, 「A job writes only in
  // its own checkout」).
  run(
    db,
    r#"UPDATE "assignments" a SET "state" = 'released', "changed_at" = $1
       WHERE a."state" = 'held'
         AND a."holder_name" LIKE $2
         AND NOT EXISTS (SELECT 1 FROM "jobs" j
               WHERE $3 || j."id"::text = a."holder_name"
                 AND (j."state" IN ('starting', 'running') OR (j."state" = 'reviewing' AND NOT j."review_parked")))"#,
    vec![now.into(), format!("{JOB_HOLDER}%").into(), JOB_HOLDER.into()],
  )
  .await?;

  Ok(())
}

/// That an assignment's job has ended: the worker it held is free, whichever state the hold stood in and
/// however the job ended (docs/api.md, 「assignments and jobs」).
async fn release(db: &sea_orm::DatabaseConnection, assignment_id: i64) -> Result<(), Refused> {
  run(
    db,
    r#"UPDATE "assignments" SET "state" = 'released', "changed_at" = $1 WHERE "id" = $2 AND "state" IN ('held', 'running')"#,
    vec![now_ms().into(), assignment_id.into()],
  )
  .await?;

  Ok(())
}

/// A session ended: every hold it took that no running job owns is released, so a worker held for a turn
/// that never started its job does not stay held once that turn's session is gone (docs/api.md,
/// 「assignments and jobs」). An assignment a job owns is left to that job's own end.
pub async fn release_held(db: &sea_orm::DatabaseConnection, holder: &str) -> Result<u64, Refused> {
  Ok(
    run(
      db,
      r#"UPDATE "assignments" a SET "state" = 'released', "changed_at" = $1
         WHERE a."state" = 'held' AND a."holder_name" = $2
           AND NOT EXISTS (SELECT 1 FROM "jobs" j
                 WHERE j."assignment_id" = a."id" AND j."state" IN ('starting', 'running', 'reviewing'))"#,
      vec![now_ms().into(), holder.into()],
    )
    .await?,
  )
}

/// The newest hold an ask already stands in: a `held` assignment for the same item and holder, as its
/// routes answer it (docs/api.md, 「assignments and jobs」).
async fn standing_hold(
  db: &sea_orm::DatabaseConnection,
  item_id: i64,
  holder: &str,
) -> Result<Option<Value>, Refused> {
  let row = one(
    db,
    r#"SELECT "id" FROM "assignments"
       WHERE "item_id" = $1 AND "holder_name" = $2 AND "state" = 'held'
       ORDER BY "id" DESC LIMIT 1"#,
    vec![item_id.into(), holder.into()],
  )
  .await?;

  match row {
    Some(row) => {
      let id: i64 = row.try_get("", "id")?;
      Ok(Some(assignment(db, id).await?))
    }
    None => Ok(None),
  }
}

// The most repository names a refusal lists before it points at `GET /repositories` instead.
const NAMES_LISTED: usize = 12;

/// The refusal of an assignee no repository holds a checkout by: the name it gave, and the repositories' names
/// where they are few enough to list.
async fn unknown_repository(db: &sea_orm::DatabaseConnection, assignee: &str) -> Result<Refused, Refused> {
  let names: Vec<String> = crate::repositories::every(db)
    .await?
    .into_iter()
    .map(|repository| repository.name)
    .collect();
  let which = if names.len() <= NAMES_LISTED {
    format!("a repository is named exactly as GET /repositories lists it: {}", names.join(", "))
  } else {
    "a repository is named exactly as GET /repositories lists it".to_owned()
  };

  Ok(
    Refused::new(
      StatusCode::UNPROCESSABLE_ENTITY,
      format!("no repository holds a checkout named {assignee}; {which}"),
    )
    .with_code("unknown-repository"),
  )
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
    let db = &call.ground().db;
    // The role the item's job runs as, `worker` where it names none; any role the store holds, and an unknown
    // one is refused (docs/repositories.md, 「A job」).
    let named = asked
      .get("roleName")
      .and_then(Value::as_str)
      .map(str::trim)
      .filter(|name| !name.is_empty());

    if let Some(role) = named
      && !crate::agent_roles::holds(db, role).await?
    {
      return Err(
        Refused::new(StatusCode::UNPROCESSABLE_ENTITY, format!("no role is named {role}"))
          .with_code("unknown-role"),
      );
    }

    let role = named.unwrap_or_else(|| crate::agent_roles::job_role()).to_owned();

    release_ended(db).await?;

    // A checkout of a repository that no assignment or job holds now, the named one where one is named.
    let mut picked = None;
    let checkouts = crate::queue::workers::workers(db, &call.ground().source).await?;

    if let Some(assignee) = assignee
      && !checkouts.iter().any(|checkout| checkout.name == assignee)
    {
      return Err(unknown_repository(db, assignee).await?);
    }

    // An ask for the same item by the same holder answers the hold it already stands in, so a lost answer
    // read again is no busy worker; an ask naming another worker or role replaces that hold (docs/api.md,
    // 「assignments and jobs」).
    if let Some(standing) = standing_hold(db, item_id, &holder).await? {
      let same_worker = assignee.is_none_or(|assignee| standing["workerName"].as_str() == Some(assignee));
      let same_role = named.is_none_or(|role| standing["roleName"].as_str() == Some(role));

      if same_worker && same_role {
        return Ok(answer(StatusCode::OK, standing));
      }

      let standing_id = standing["id"].as_i64().unwrap_or_default();

      if let Err(why) = release(db, standing_id).await {
        eprintln!("assignment {standing_id} was not released before its replacement: {}", why.error);
      }
    }

    for checkout in checkouts {
      if checkout.builtin || assignee.is_some_and(|assignee| assignee != checkout.name) {
        continue;
      }

      let held = one(
        db,
        r#"SELECT 1 AS "held" WHERE EXISTS (SELECT 1 FROM "assignments" WHERE "worker_name" = $1 AND "state" IN ('held', 'running'))
             OR EXISTS (SELECT 1 FROM "jobs" WHERE "worker_name" = $1
                   AND ("state" IN ('starting', 'running') OR ("state" = 'reviewing' AND NOT "review_parked")))"#,
        vec![checkout.name.clone().into()],
      )
      .await?;

      if held.is_none() {
        picked = Some(checkout);
        break;
      }
    }
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
    let worker_name: String = picked.name;
    let now = now_ms();
    let row = one(
      db,
      r#"INSERT INTO "assignments" ("item_id", "holder_name", "worker_name", "role", "state", "created_at",
              "changed_at")
         VALUES ($1, $2, $3, $5, 'held', $4, $4) RETURNING "id""#,
      vec![item_id.into(), holder.into(), worker_name.into(), now.into(), role.into()],
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
    release_ended(db).await?;

    let after = call.query("after").and_then(|after| after.parse::<i64>().ok()).unwrap_or(0);
    let limit = limit(&call);
    let holder = call.query("holderName").unwrap_or_default();
    let worker = call.query("workerName").unwrap_or_default();
    let state = call.query("state").unwrap_or_default();
    let repository = call
      .query("repositoryName")
      .map(|name| name.trim().to_owned())
      .filter(|name| !name.is_empty());
    let checkouts = checkouts_named(db, repository.as_deref()).await?;
    let sql = format!(
      r#"SELECT {ASSIGNMENT_COLUMNS} FROM "assignments"
         WHERE "id" > $1 AND ($2 = '' OR "holder_name" = $2) AND ($3 = '' OR "worker_name" = $3)
           AND ($4 = '' OR "state" = $4)
           AND ($6::jsonb IS NULL OR "worker_name" IN (SELECT jsonb_array_elements_text($6::jsonb)))
         ORDER BY "id" LIMIT $5"#
    );
    let rows = all(
      db,
      &sql,
      vec![
        after.into(),
        holder.into(),
        worker.into(),
        state.into(),
        ((limit + 1) as i64).into(),
        checkouts.into(),
      ],
    )
    .await?;
    let mut items: Vec<Value> = rows.iter().map(assignment_value).collect::<Result<_, _>>()?;
    let more = items.len() > limit;
    items.truncate(limit);
    name_repositories(db, &mut items).await?;

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
    release_ended(&call.ground().db).await?;

    Ok(answer(StatusCode::OK, assignment(&call.ground().db, id).await?))
  }
}

crate::moved!(ReadAssignment);

/// Release an assignment.
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

    let member = assignment(db, id).await?;
    publish(call.ground(), "assignment.changed", member.clone());

    Ok(answer(StatusCode::OK, member))
  }
}

crate::moved!(ChangeAssignment);

// ---- jobs ----

const JOB_COLUMNS: &str = r#""id", "item_id", "lease_id", "holder_name", "assignment_id", "worker_name", "role",
  "session_id", "run_id", "run_name", "room_id", "state", "outcome", "report", "token_count",
  "input_tokens", "output_tokens", "cache_read_tokens", "cache_write_tokens", "instruction",
  "started_at", "ended_at", "prompt", "prompt_english", "roads", "held_for", "chosen_by", "review_stages",
  "review_stage", "review_round", "review_check", "rejections", "review_question", "review_parked", "item_agent""#;

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
    "roleName": row.try_get::<String>("", "role")?,
    "agentName": row.try_get::<Option<String>>("", "item_agent")?,
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
    "roads": row
      .try_get::<Option<String>>("", "roads")?
      .and_then(|text| serde_json::from_str::<Value>(&text).ok())
      .unwrap_or_else(|| json!([])),
    "heldFor": row.try_get::<Option<String>>("", "held_for")?,
    "chosenBy": row.try_get::<Option<String>>("", "chosen_by")?,
    "review": crate::job_review::value(row)?,
    "startedAt": row.try_get::<i64>("", "started_at")?,
    "endedAt": row.try_get::<Option<i64>>("", "ended_at")?,
  }))
}

pub(crate) async fn job(db: &sea_orm::DatabaseConnection, id: i64) -> Result<(Value, String), Refused> {
  let sql = format!(r#"SELECT {JOB_COLUMNS} FROM "jobs" WHERE "id" = $1"#);
  let row = one(db, &sql, vec![id.into()]).await?;

  match row {
    Some(row) => {
      let mut value = job_value(&row)?;

      name_repositories(db, std::slice::from_mut(&mut value)).await?;
      Ok((value, row.try_get("", "prompt")?))
    }
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
    let texts: Vec<&str> = notes
      .iter()
      .map(|note| note.get("text").and_then(Value::as_str).or_else(|| note.as_str()).unwrap_or_default())
      .collect();

    prompt.push_str("\n## Notes\n\n");
    prompt.push_str(&notes_within(&texts, policy::job_prompt::JOB_PROMPT.notes.most_bytes));
  }

  prompt
}

/// An item's notes as a job's prompt carries them: the newest that fit `most` bytes, in their order, after a
/// line saying how many older ones were left out where any were.
pub fn notes_within(notes: &[&str], most: usize) -> String {
  let mut kept = Vec::new();
  let mut bytes = 0;

  for note in notes.iter().rev() {
    let mut line = format!("- {note}\n");

    // The newest note alone past the bytes is cut at them rather than left out whole.
    if kept.is_empty() && line.len() > most {
      let mut cut = most.saturating_sub(5);

      while !line.is_char_boundary(cut) {
        cut -= 1;
      }

      line.truncate(cut);
      line.push_str(" …\n");
    }

    if bytes + line.len() > most && !kept.is_empty() {
      break;
    }

    bytes += line.len();
    kept.push(line);
  }

  let left = notes.len() - kept.len();
  let mut text = String::new();

  if left > 0 {
    text.push_str(&policy::fill_marks(
      &policy::job_prompt::JOB_PROMPT.notes.left_out,
      &[("n", &left.to_string())],
    ));
    text.push('\n');
  }

  for line in kept.into_iter().rev() {
    text.push_str(&line);
  }

  text
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

    release_ended(db).await?;
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
    // Every job names its agent, which carries its role and its road (docs/repositories.md, 「An agent an item
    // names」); a role label is still judged, and decides nothing more.
    let asked_role = item_role(db, item_id, &item).await?.unwrap_or_else(|| {
      held["roleName"].as_str().unwrap_or_else(|| crate::agent_roles::job_role()).to_owned()
    });
    let item_agent = item_agent(&call.shared_ground(), item_id, &item, &asked_role).await?;
    let role = item_agent.role.clone();
    let item_model = item_agent.entry().map(|entry| Pin {
      provider: entry.provider,
      model: entry.model,
    });
    // A job is never refused for quota (docs/api.md, 「assignments and jobs」): where nothing in its walk can
    // answer now, it is taken and waits `starting`, held with the reason, until a road can. The walk read
    // here only says which chose the job's road: its agent's road is always one to follow.
    let resolution = road::for_job(
      call.ground(),
      Some(&role),
      Some(&worker_name),
      item_model.as_ref(),
      &[],
      Some(holder.as_str()),
    )
    .await
    .map_err(|why| Refused::new(StatusCode::SERVICE_UNAVAILABLE, why))?;
    let prompt = prompt_of(&item, instruction.as_ref());
    let room_id = number(&asked, "roomId")
      .or_else(|| number(&item, "roomId"))
      .or_else(|| item.get("scope").and_then(Value::as_str).and_then(|scope| scope.trim().parse().ok()))
      .or_else(|| number(&item, "scope"));
    // Agency starts no job by itself (docs/role-agents.md, 「Which agent a run takes」): a launch onto an agent
    // that carries a run is refused now rather than queued to start once the agent frees. The agent's row is
    // held while it is read and the job written, so two launches onto one free agent never both go.
    let written = db.begin().await.map_err(|why| Refused::failed(why.to_string()))?;

    one(
      &written,
      r#"SELECT 1 FROM "role_agents" WHERE "name" = $1 FOR UPDATE"#,
      vec![item_agent.name.clone().into()],
    )
    .await?;

    if let Some(carried) = agent_carries(&written, &item_agent.name).await? {
      return Err(
        Refused::new(
          StatusCode::CONFLICT,
          format!(
            "Item #{item_id} names the agent {}, which carries {carried}; launch it once the agent is free",
            item_agent.name
          ),
        )
        .with_code("agent-busy"),
      );
    }

    let row = one(
      &written,
      r#"INSERT INTO "jobs" ("item_id", "lease_id", "holder_name", "assignment_id", "worker_name",
              "room_id", "state", "token_count", "started_at", "prompt", "instruction", "role", "item_provider",
              "item_model", "chosen_by", "item_agent")
         VALUES ($1, $2, $3, $4, $5, $6, 'starting', 0, $7, $8, $9, $10, $11, $12, $13, $14) RETURNING "id""#,
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
        role.into(),
        item_model.as_ref().map(|named| named.provider.clone()).into(),
        item_model.as_ref().map(|named| named.model.clone()).into(),
        resolution.chosen_by.map(|chosen| chosen.word().to_owned()).into(),
        Some(item_agent.name.clone()).into(),
      ],
    )
    .await?
    .ok_or_else(|| Refused::failed("the job was written but not answered"))?;
    let id: i64 = row.try_get("", "id")?;

    written.commit().await.map_err(|why| Refused::failed(why.to_string()))?;
    crate::watches::queue_idle::agent_told_let_go(db, item_id).await;

    run(
      db,
      r#"UPDATE "assignments" SET "state" = 'running', "changed_at" = $1 WHERE "id" = $2"#,
      vec![now.into(), assignment_id.into()],
    )
    .await?;
    let (member, _) = job(db, id).await?;
    crate::watches::queue_idle::job_started(db).await;
    publish(call.ground(), "job.created", member.clone());

    let ground = call.shared_ground();
    let sessions = call.sessions();
    tokio::spawn(async move { launch(ground, sessions, id).await });

    Ok(answer(StatusCode::CREATED, member))
  }
}

crate::moved!(CreateJob);

/// What an agent carries now, in words a launch's refusal names it by (docs/repositories.md, 「An agent an item
/// names」): a job standing on it — starting, running, or under a review that has not paused it — else the run
/// a slot request let in on it stands for. None where it carries nothing.
async fn agent_carries<C: sea_orm::ConnectionTrait>(
  db: &C,
  agent: &str,
) -> Result<Option<String>, Refused> {
  let job = one(
    db,
    r#"SELECT "id", "item_id" FROM "jobs"
       WHERE "item_agent" = $1
         AND ("state" IN ('starting', 'running') OR ("state" = 'reviewing' AND NOT "review_parked"))
       ORDER BY "id" LIMIT 1"#,
    vec![agent.into()],
  )
  .await?;

  if let Some(job) = job {
    let id: i64 = job.try_get("", "id")?;
    let item: i64 = job.try_get("", "item_id")?;

    return Ok(Some(format!("job {id} of item #{item}")));
  }

  let run = one(
    db,
    r#"SELECT "source", "title" FROM "role_slot_requests" WHERE "seat" = $1 AND "state" = 'active'
       ORDER BY "sequence" LIMIT 1"#,
    vec![agent.into()],
  )
  .await?;

  let Some(run) = run else {
    return Ok(None);
  };
  let source: String = run.try_get("", "source")?;
  let title: Option<String> = run.try_get("", "title")?;

  Ok(Some(match title.filter(|title| !title.trim().is_empty()) {
    Some(title) => format!("a {source} run, {title}"),
    None => format!("a {source} run"),
  }))
}

/// The work parts a run of `role` at a checkout is carried with on a road: its provider and model, and the
/// effort and answer ceiling the road's entry names for it.
pub(crate) async fn role_work_parts(
  ground: &Ground,
  role: &str,
  worker_name: &str,
  room: i64,
  name: &str,
  title: &str,
  entry: &Entry,
) -> Result<WorkParts, String> {
  // The entry names the provider, model, effort and answer ceiling the turn goes out with, whichever road it
  // is: an agent's own, an item's model, a repository's pin, or the road a role that names none follows.
  let provider = entry.provider.clone();
  let effort = entry.effort.clone();
  let answer_tokens = entry.answer_tokens;
  let model = entry.model.clone();
  let row = AgentRow::named(&ground.db, worker_name)
    .await
    .map_err(|why| why.detail)?
    .ok_or_else(|| format!("no worker is named {worker_name}"))?;
  let cwd = row.cwd.unwrap_or_else(|| ground.source.to_string_lossy().into_owned());
  let trees = Trees::of(ground);
  let mut line = worker::worker_run(ground, &trees, Some(worker_name), None, &model, &cwd)
    .await
    .map_err(|why| why.detail)?;
  line.effort = effort.clone().unwrap_or(line.effort);
  let mut variables = BTreeMap::new();
  variables.insert("BMO_DATA_DIR".to_owned(), ground.roots.data.to_string_lossy().into_owned());

  Ok(WorkParts {
    provider,
    model: model.clone(),
    mapping: WorkMapping {
      model,
      effort,
      tier: None,
      answer_tokens: answer_tokens.and_then(|tokens| u64::try_from(tokens).ok()),
    },
    resume: Resume::Auto,
    cwd,
    name: name.to_owned(),
    title: title.to_owned(),
    chat: room,
    unit: Some(name.to_owned()),
    user: None,
    work_key: role.to_owned(),
    hears_words: true,
    variables,
  })
}

// The jobs this process carries now, from their launch to their end: one waiting for a road, or between one
// road and the next, has no live execution behind it and is still no job the process lost.
static CARRYING: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<i64>>> =
  std::sync::LazyLock::new(Default::default);

/// Whether this process carries a turn of the job now.
pub fn is_carrying(id: i64) -> bool {
  carrying().contains(&id)
}

fn carrying() -> std::sync::MutexGuard<'static, std::collections::HashSet<i64>> {
  CARRYING.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// The run of one job: the worker's session started, the item's turns carried across its roads, and the
// job's end written, its assignment released, and an end that is no answer noted in the job's log. A job
// ended from outside while it waited for a road keeps the end written for it.
async fn launch(ground: Arc<Ground>, sessions: Option<crate::herdr_backend::Shared>, id: i64) {
  carrying().insert(id);

  let ended = launched(&ground, sessions, id, None).await;

  carrying().remove(&id);

  finish(&ground, id, ended).await;
}

/// A turn a job's review asks for (docs/job-review.md, 「The turns the review asks for」): carried as the job's
/// first turn is, in the job's own session where it can be resumed, and its end written as any job turn's end
/// is, back into the review where it answered.
pub async fn carry_review_turn(
  ground: Arc<Ground>,
  sessions: Option<crate::herdr_backend::Shared>,
  id: i64,
  turn: crate::job_review::Turn,
) {
  carrying().insert(id);

  let ended = launched(&ground, sessions, id, Some(turn)).await;

  carrying().remove(&id);

  finish(&ground, id, ended).await;
}

/// A job's turn that ended after the process that launched it went away, ended as the launch would have
/// ended it (docs/api.md, 「assignments and jobs」): its run closed with the turn's end, the job ended with
/// its report, its assignment released, its lease returned and its end told to the room.
pub async fn job_turn_ended(ground: &Ground, id: i64, end: agent_session::TurnEnd) {
  let ledger = match job(&ground.db, id).await {
    Ok((member, _)) => member["runId"].as_i64().unwrap_or_default(),
    Err(why) => {
      eprintln!("job {id}: its turn's end was not written: {}", why.error);
      return;
    }
  };
  let ended = ended_of(ground, ledger, end).await;

  finish(ground, id, Ok(Some(ended))).await;
}

// A job's end written whole, however its turn came to it: a job a stop marked ends stopped with the stop's
// reason, and nothing is written where the job was ended from outside, while it waited for a road or after.
async fn finish(ground: &Ground, id: i64, ended: Result<Option<Ended>, String>) {
  let stop = crate::queue::stops::job_stop_reason(&ground.db, id).await.ok().flatten();
  let _ = crate::queue::stops::clear_job_stop(&ground.db, id).await;

  if ended_outside(ground, id).await {
    return;
  }

  let (state, outcome, report, spent) = match (stop, ended) {
    (Some(why), ended) => {
      let spent = ended.ok().flatten().and_then(|(_, _, _, spent)| spent);

      println!("job {id}: stop consumed: {}", why.trim());

      ("stopped".to_owned(), Some("stopped".to_owned()), Some(why), spent)
    }
    (None, Ok(Some(end))) => end,
    (None, Ok(None)) => return,
    (None, Err(why)) => ("stopped".to_owned(), Some("failed".to_owned()), Some(why), None),
  };

  // What was said into the job's turn while it ran stands in its report, read off its run's own log.
  let report = match crate::words::with_words(&ground.db, id, report.clone()).await {
    Ok(report) => report,
    Err(why) => {
      eprintln!("job {id}: the words said into its turn were not added to its report: {}", why.detail);

      report
    }
  };

  // An answered turn ends no job: its report goes to the job's review (docs/job-review.md).
  if state == "ended" && outcome.is_none() {
    crate::job_review::reported(ground, id, report.as_deref().unwrap_or_default(), spent).await;

    return;
  }

  written_end(ground, id, &state, outcome, report, spent).await;
}

/// A job's end written: its state, outcome and report, what its last turn spent added to the rest, its slot
/// given back, its assignment released, and its end delivered to its item's orderer (docs/api.md, 「assignments
/// and jobs」).
pub(crate) async fn written_end(
  ground: &Ground,
  id: i64,
  state: &str,
  outcome: Option<String>,
  report: Option<String>,
  spent: Option<Spent>,
) {
  let now = now_ms();

  println!("job {id}: ended {state}, outcome {}", outcome.as_deref().unwrap_or("answered"));

  if let Some(outcome) = &outcome {
    noted(ground, id, outcome, report.as_deref()).await;
  }

  if let Err(why) = run(
    &ground.db,
    &format!(
      r#"UPDATE "jobs" SET "state" = $1, "outcome" = $2, "report" = $3, "ended_at" = $5, {SPENT_ADDED}
         WHERE "id" = $6"#
    ),
    vec![
      state.to_owned().into(),
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

  crate::job_end::slot_given_back(&ground.db, &ground.events, id, &ending(state, outcome.as_deref())).await;

  if let Ok((member, _)) = job(&ground.db, id).await {
    let assignment_id = member["assignmentId"].as_i64().unwrap_or_default();

    if let Err(why) = release(&ground.db, assignment_id).await {
      eprintln!("job {id}: its assignment {assignment_id} was not released: {}", why.error);
    }

    crate::job_end::delivered(ground, &member).await;
    publish(ground, if state == "ended" { "job.ended" } else { "job.changed" }, member);
  }
}

/// What a job's turn spent added to what its earlier turns spent: `$4` the total, `$7` to `$10` the input,
/// output, cache-read and cache-write tokens, each figure left as it stood where the turn reported none.
pub const SPENT_ADDED: &str = r#""token_count" = COALESCE("token_count", 0) + $4,
  "input_tokens" = CASE WHEN $7::bigint IS NULL THEN "input_tokens" ELSE COALESCE("input_tokens", 0) + $7 END,
  "output_tokens" = CASE WHEN $8::bigint IS NULL THEN "output_tokens" ELSE COALESCE("output_tokens", 0) + $8 END,
  "cache_read_tokens" = CASE WHEN $9::bigint IS NULL THEN "cache_read_tokens"
    ELSE COALESCE("cache_read_tokens", 0) + $9 END,
  "cache_write_tokens" = CASE WHEN $10::bigint IS NULL THEN "cache_write_tokens"
    ELSE COALESCE("cache_write_tokens", 0) + $10 END"#;

// The reason a job's end gives its slot back with: its state and, where it names one, its outcome.
fn ending(state: &str, outcome: Option<&str>) -> String {
  match outcome.filter(|outcome| !outcome.is_empty() && *outcome != state) {
    Some(outcome) => format!("{state}, {outcome}"),
    None => state.to_owned(),
  }
}

/// How long after it began a job is old enough to be read as gone when no live execution stands behind it:
/// past any launch window, so a launch still placing its turn is never mistaken for one the process lost.
pub const REAP_GRACE_MS: i64 = 2 * 60 * 1000;

/// What a stopped job's report says where its stop named no reason.
pub const STOPPED: &str = "The job was stopped from outside before it finished.";

/// What a job with no live execution behind it is told: the process that ran it went away.
pub const REAPED: &str = "The agency process that ran this job went away before the job could write its own end (a restart), so the job is stopped.";

// A job's live execution: an open execution row of its worker's, laid on the worker line, begun at the job's
// own start. The worker runs one job at a time, so one that begins in the job's first minute is its own.
async fn live_execution(
  db: &sea_orm::DatabaseConnection,
  worker: &str,
  started_at: i64,
) -> Result<bool, Refused> {
  Ok(
    one(
      db,
      r#"SELECT "id" FROM "agent_executions"
         WHERE ("body"::jsonb)->>'agent' = $1
           AND ("body"::jsonb)->>'line' = 'worker'
           AND (("body"::jsonb)->>'endedAt') IS NULL
           AND (("body"::jsonb)->>'startedAt')::bigint >= $2
           AND (("body"::jsonb)->>'startedAt')::bigint <= $3
         LIMIT 1"#,
      vec![worker.to_owned().into(), (started_at - 1_000).into(), (started_at + 60_000).into()],
    )
    .await?
    .is_some(),
  )
}

/// A job found `starting` or `running` with no live execution behind it, ended as stopped with the reason
/// (docs/api.md, 「assignments and jobs」): the process that ran it went away, so no later end would ever be
/// written. Its assignment is released, its open run closed, its lease returned and its report told to the
/// room, exactly as any other end. A job whose worker stands on another machine is left, since its execution
/// is no record this machine keeps. Answers how many jobs ended.
pub async fn reap_stale(ground: &Ground) -> Result<usize, Refused> {
  let now = now_ms();
  let rows = all(
    &ground.db,
    r#"SELECT "id", "worker_name", "assignment_id", "run_id", "run_name", "started_at", "state"
       FROM "jobs" WHERE "state" IN ('starting', 'running') ORDER BY "id""#,
    vec![],
  )
  .await?;
  let mut reaped = 0;

  for row in rows {
    let id: i64 = row.try_get("", "id")?;
    let worker: String = row.try_get("", "worker_name")?;
    let run_id: Option<i64> = row.try_get("", "run_id")?;
    let started_at: i64 = row.try_get("", "started_at")?;
    let state: String = row.try_get("", "state")?;

    if now - started_at < REAP_GRACE_MS || carrying().contains(&id) {
      continue;
    }

    // A worker on a node runs elsewhere: its execution is no row this machine holds.
    let node: Option<String> = crate::queue::workers::worker_row(&ground.db, &worker, &ground.source)
      .await?
      .and_then(|checkout| checkout.node)
      .filter(|node| !node.trim().is_empty());

    if node.is_some() {
      println!("job {id}: left {state}: its worker {worker} stands on {}", node.unwrap_or_default());
      continue;
    }

    // A turn kept in flight is the pickup's: it hands the job its end once the turn's record holds one.
    let run_name: Option<String> = row.try_get("", "run_name").ok().flatten();

    if let Some(run_name) = run_name.filter(|name| !name.is_empty())
      && crate::turn_pickup::kept(&ground.db, &run_name).await
    {
      continue;
    }

    let live = live_execution(&ground.db, &worker, started_at).await?;
    let run_open = match run_id.filter(|run| *run > 0) {
      Some(run) => run_ledger::read(&ground.db, run)
        .await
        .map(|held| held.is_some_and(|held| held.finished_at.is_none()))
        .unwrap_or(false),
      None => false,
    };

    if live && (run_open || run_id.is_none()) {
      continue;
    }

    let report = format!("{REAPED} It was {state} when the process ended.");

    if run(
      &ground.db,
      r#"UPDATE "jobs" SET "state" = 'stopped', "outcome" = 'stopped', "report" = $1, "ended_at" = $2
         WHERE "id" = $3 AND "state" IN ('starting', 'running')"#,
      vec![report.clone().into(), now.into(), id.into()],
    )
    .await?
      == 0
    {
      continue;
    }

    noted(ground, id, "stopped", Some("the agency process that ran it went away (a restart)")).await;
    crate::job_end::slot_given_back(
      &ground.db,
      &ground.events,
      id,
      "stopped: the agency process that ran it went away",
    )
    .await;

    if let Some(assignment) = row.try_get::<Option<i64>>("", "assignment_id").ok().flatten()
      && assignment > 0
      && let Err(why) = release(&ground.db, assignment).await
    {
      eprintln!("job {id}: its assignment {assignment} was not released: {}", why.error);
    }

    if let Some(run_id) = run_id.filter(|run| *run > 0) {
      let closing = Closing {
        finished_at: now,
        outcome: "stopped".to_owned(),
        context_tokens: None,
        spent: None,
      };

      if let Err(why) = run_ledger::close(&ground.db, run_id, &closing).await {
        println!("job {id}: its run {run_id} was not closed: {why:?}");
      }
    }

    if let Ok((member, _)) = job(&ground.db, id).await {
      crate::job_end::delivered(ground, &member).await;
      publish(ground, "job.changed", member);
    }

    reaped += 1;
  }

  Ok(reaped)
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

// What a job's report says where its turn stopped on a tool call written as text.
const TEXT_TOOL_CALL_REPORT: &str = "The turn stopped on a tool call written as text rather than made. The \
  call did not run, so the job left no report. Check that the tool it called is offered to the run, then \
  run the job again.";

/// What a job's end takes from its turn's end: the outcome — none where the turn answered — and the report
/// it leaves. An answer that is a tool call written as text is no report: the call never ran and the work
/// never happened, so the job ends failed with a line saying so rather than an empty outcome holding the
/// call's text (docs/api.md, 「A job ends by itself as its turn ends」).
fn ended_by(
  outcome: agent_session::Outcome,
  answer: Option<String>,
  reason: Option<String>,
) -> (Option<String>, Option<String>) {
  if matches!(outcome, agent_session::Outcome::Answered)
    && answer.as_deref().is_some_and(crate::door::record::text_tool_call)
  {
    return (Some("failed".to_owned()), Some(TEXT_TOOL_CALL_REPORT.to_owned()));
  }

  let outcome = match outcome {
    agent_session::Outcome::Answered => None,
    other => Some(format!("{other:?}").to_lowercase()),
  };
  let report = match outcome {
    None => answer,
    Some(_) => answer.or(reason),
  };

  (outcome, report)
}

/// What carries one turn of a job on one road: the worker's session through the session contract, or a
/// test's stand-in for it. A turn that could not be carried at all is its error; any turn that was is its
/// end, however it ended. The entry is the road's, with the effort and answer ceiling the turn goes out on.
pub trait Carrier {
  fn carry(
    &mut self,
    taken: &Road,
    entry: &Entry,
    prompt: &str,
  ) -> impl Future<Output = Result<TurnEnd, String>>;
}

/// The job a walk of roads carries: its id, its worker, the holder that asked it, its `holderName`, and the
/// prompt it was launched with. A holder the store holds a supervisor for is the supervisor a role that names
/// no road follows.
pub struct Carried<'a> {
  pub id: i64,
  pub worker: &'a str,
  pub holder: &'a str,
  pub prompt: &'a str,
}

/// Why a turn's end says its road stopped answering rather than the work failing, where it does
/// (docs/cache-fallback.md, 「Automatic walks only on providers, in order」; docs/provider-order.md, 「Falling
/// over and coming back」): the provider refusing its quota, its window, its balance or its credential, its
/// last login closing under it, answering with a server error or refusing the connection, or the road
/// switched off — read off the end's own failure, its engine's error, and its words through the one failure
/// reading every run asks. A turn that answered, was stopped from outside, or whose request the provider
/// refused for its shape never says so.
pub fn road_stopped(end: &TurnEnd) -> Option<String> {
  use agent_session::{AccountFailure, Failure, Outcome};

  if matches!(end.outcome, Outcome::Answered | Outcome::Stopped | Outcome::Reprepared) {
    return None;
  }

  let words = end
    .reason
    .clone()
    .or_else(|| end.answer.clone())
    .or_else(|| end.failure.as_ref().map(Failure::words))
    .unwrap_or_default();
  let said = || {
    Some(if words.trim().is_empty() {
      "its road stopped answering".to_owned()
    } else {
      words.clone()
    })
  };

  match &end.failure {
    Some(Failure::Stopped) => return None,
    Some(
      Failure::Limit { .. }
      | Failure::Unavailable { .. }
      | Failure::Account {
        failure: AccountFailure::Limit | AccountFailure::Login | AccountFailure::Outage,
        ..
      },
    ) => return said(),
    _ => {}
  }

  use crate::job_run::claude::failure::{self as reading, AccountFailure as Read};

  let engine = end.engine_error.as_ref().and_then(|engine| engine.kind.as_deref());

  if reading::request_refused(&words, engine) {
    return None;
  }

  if matches!(
    engine,
    Some(
      "rate_limit"
        | "billing_error"
        | "authentication_error"
        | "authentication_failed"
        | "server_error"
        | "overloaded"
    )
  ) {
    return said();
  }

  match reading::of_errored(&words, JOB_WORK_KEY) {
    reading::Failure::Limit { .. }
    | reading::Failure::Account {
      failure: Read::Limit | Read::Login | Read::Outage,
      ..
    } => said(),
    _ => None,
  }
}

/// The line a job's prompt is opened with as it goes out again on the next road, ahead of its own words.
pub fn continued(from: &Road, why: &str, prompt: &str) -> String {
  let line =
    policy::fill_marks(&policy::roads::ROADS.job.continued, &[("from", &from.words()), ("why", why)]);

  format!("{line}\n\n{prompt}")
}

// One road a job's turn went out on, as the job keeps and answers it.
fn road_value(taken: &Road, reason: Option<&str>) -> Value {
  json!({
    "provider": taken.provider,
    "model": taken.model,
    "account": taken.account,
    "at": now_ms(),
    "reason": reason,
  })
}

// The line a job's report ends with where its turns went out on more than one road: each road it left, why,
// and the road it ended on.
fn moved_line(roads: &[(Road, Option<String>)]) -> Option<String> {
  if roads.len() < 2 {
    return None;
  }

  let words = &policy::roads::ROADS.job;
  let mut parts = vec![];

  for (at, (taken, _)) in roads.iter().enumerate() {
    match roads.get(at + 1).and_then(|(_, why)| why.as_deref()) {
      Some(why) => parts.push(policy::fill_marks(&words.road, &[("road", &taken.words()), ("why", why)])),
      None => parts.push(taken.words()),
    }
  }

  Some(policy::fill_marks(&words.moved, &[("roads", &parts.join(", then "))]))
}

// A job's report with the account of its roads at its end, where it moved.
fn with_roads(report: Option<String>, roads: &[(Road, Option<String>)]) -> Option<String> {
  match (report, moved_line(roads)) {
    (Some(report), Some(moved)) => Some(format!("{report}\n\n{moved}")),
    (None, Some(moved)) => Some(moved),
    (report, None) => report,
  }
}

fn summed(spent: Option<Spent>, more: Option<Spent>) -> Option<Spent> {
  match (spent, more) {
    (Some(spent), Some(more)) => Some(Spent {
      input_tokens: spent.input_tokens + more.input_tokens,
      output_tokens: spent.output_tokens + more.output_tokens,
      cache_read_tokens: spent.cache_read_tokens + more.cache_read_tokens,
      cache_write_tokens: spent.cache_write_tokens + more.cache_write_tokens,
    }),
    (spent, more) => spent.or(more),
  }
}

// The roads a job went out on written onto its row, with the model of the last and what chose it, and the
// job told.
async fn kept_roads(
  ground: &Ground,
  id: i64,
  roads: &[(Road, Option<String>)],
  values: &[Value],
  chosen_by: Option<road::ChosenBy>,
) {
  let model = roads.last().map(|(taken, _)| taken.model.clone());

  if let Err(why) = run(
    &ground.db,
    r#"UPDATE "jobs" SET "roads" = $1, "model" = COALESCE($2, "model"), "chosen_by" = COALESCE($4, "chosen_by")
       WHERE "id" = $3"#,
    vec![
      Value::Array(values.to_vec()).to_string().into(),
      model.into(),
      id.into(),
      chosen_by.map(|chosen| chosen.word().to_owned()).into(),
    ],
  )
  .await
  {
    eprintln!("job {id}: its roads were not written: {why}");
  }
}

// That a job waits for a road, with the reason, or no longer does; the job told either way, and its item's
// orderer as the wait begins.
async fn held_for(ground: &Ground, id: i64, why: Option<&str>) {
  hold_job(ground, id, why, why.is_some()).await;
}

/// That a job waits, with the reason — for a road, or for its turn's card (docs/own-llm.md, 「An engine the
/// arbiter stands」) — or no longer does: its `heldFor` written and the job published, and its item's orderer
/// told of the wait where `tell` says this is where the wait begins.
pub async fn hold_job(ground: &Ground, id: i64, why: Option<&str>, tell: bool) {
  if let Err(why) = run(
    &ground.db,
    r#"UPDATE "jobs" SET "held_for" = $1 WHERE "id" = $2"#,
    vec![why.map(str::to_owned).into(), id.into()],
  )
  .await
  {
    eprintln!("job {id}: its hold was not written: {why}");
  }

  if let Ok((member, _)) = job(&ground.db, id).await {
    if let Some(why) = why.filter(|_| tell) {
      tell_waiting(ground, &member, why).await;
    }

    publish(ground, "job.changed", member);
  }
}

// A job's wait told to its item's orderer as it begins (docs/agent-control.md, 「An item's change is told to its
// orderer」); a test's ground tells no one.
async fn tell_waiting(ground: &Ground, member: &Value, why: &str) {
  if ground.testing() {
    return;
  }

  let item = member["itemId"].as_i64().unwrap_or_default();
  let worker = member["workerName"].as_str().unwrap_or_default();

  crate::job_notices::tell_waiting(ground.vars(), item, worker, why).await;
}

// Whether a job was ended or stopped from outside while this process carried it.
async fn ended_outside(ground: &Ground, id: i64) -> bool {
  job(&ground.db, id)
    .await
    .is_ok_and(|(member, _)| matches!(member["state"].as_str(), Some("ended" | "stopped")))
}

// Whether a job stopped from outside, or asked to stop, is to go no further while it waits.
async fn called_off(ground: &Ground, id: i64) -> bool {
  ended_outside(ground, id).await
    || crate::queue::stops::job_stop_reason(&ground.db, id).await.ok().flatten().is_some()
}

// A job waiting for a road: held with the closed road's sentence and asked again as quota or a road moves, and
// on the policy's clock, until a provider of its walk can answer. False where the job was ended from outside,
// or asked to stop, while it waited.
async fn wait_for_road(
  ground: &Ground,
  job: &Carried<'_>,
  resolution: &road::Resolution,
) -> Result<bool, String> {
  // Asked again as each usage reading, account change, rest or road fact comes in, and on the policy's clock
  // where none does.
  let mut moved = crate::order::quota_watch(ground);
  let sentence = resolution.no_road.clone().unwrap_or_else(|| resolution.sentence());

  held_for(ground, job.id, Some(&sentence)).await;
  // A role with no road to follow has no walk to report: the sentence is the whole of it.
  if resolution.no_road.is_none() {
    road::went(ground, resolution, None, &[]).await;
  }
  noted_line(ground, job.id, &sentence).await;

  let every = std::time::Duration::from_secs(policy::roads::ROADS.job.wait_seconds);

  loop {
    if let Ok(None) = tokio::time::timeout(every, moved.next()).await {
      tokio::time::sleep(every).await;
    }

    if called_off(ground, job.id).await {
      held_for(ground, job.id, None).await;

      return Ok(false);
    }

    let item = item_model_of_job(ground, job.id).await;
    let asked_by = Some(job.holder).filter(|name| !name.is_empty());

    if road::for_job(
      ground,
      Some(&role_of_job(ground, job.id).await),
      Some(job.worker),
      item.as_ref(),
      &[],
      asked_by,
    )
    .await?
    .road()
    .is_some()
    {
      held_for(ground, job.id, None).await;

      return Ok(true);
    }
  }
}

/// A job carried across its roads (docs/cache-fallback.md, 「One road resolution」): each turn goes out on
/// the road the resolution answers for the job's worker now; a turn whose road stopped answering is carried
/// again on the next road, the same job in the same checkout, at most once on each provider, its prompt
/// opened with the line saying it continues the work; and where nothing in the order can answer, the job
/// waits, held with the reason, until a provider can. The end it answers carries every road's spending and,
/// in its report, where the job ran and why it moved; none where the job was ended from outside.
pub async fn carry_job<C: Carrier>(
  ground: &Ground,
  job: &Carried<'_>,
  carrier: &mut C,
) -> Result<Option<Ended>, String> {
  // A support worker's worktree stands before any road takes its turn, since the turn's program is stood
  // in it and a program handed a folder that is not there never starts (docs/worker-slots.md, 「Names and
  // places」).
  crate::places::checkouts::ensure_support_checkout(ground, job.worker).await?;

  let role = role_of_job(ground, job.id).await;
  let role = Some(role.as_str());
  let asked_by = Some(job.holder).filter(|name| !name.is_empty());
  let item = item_model_of_job(ground, job.id).await;
  let item = item.as_ref();
  let mut left: Vec<Closed> = vec![];
  let mut roads: Vec<(Road, Option<String>)> = vec![];
  let mut values: Vec<Value> = vec![];
  let mut spent: Option<Spent> = None;
  let mut prompt = job.prompt.to_owned();
  let mut last: Option<Ended> = None;

  loop {
    let resolution = road::for_job(ground, role, Some(job.worker), item, &left, asked_by).await?;
    let Some((taken, entry)) = resolution.going().map(|(road, entry)| (road.clone(), entry.clone())) else {
      // Every road that can answer now was already tried in this job: it ends as its last turn did.
      if let Some((state, outcome, report, _)) = last.take()
        && road::for_job(ground, role, Some(job.worker), item, &[], asked_by)
          .await?
          .road()
          .is_some()
      {
        return Ok(Some((state, outcome, with_roads(report, &roads), spent)));
      }

      if !wait_for_road(ground, job, &resolution).await? {
        return Ok(None);
      }

      left.clear();
      continue;
    };
    let why = roads.last().and_then(|_| left.last().map(|left| left.reason.clone()));

    road::went(ground, &resolution, Some(&taken), &[]).await;
    values.push(road_value(&taken, why.as_deref()));
    roads.push((taken.clone(), why));
    kept_roads(ground, job.id, &roads, &values, resolution.chosen_by).await;

    let end = carrier.carry(&taken, &entry, &prompt).await?;

    spent = summed(spent, turn_spent(&end));

    let (outcome, report) = ended_by(end.outcome, end.answer.clone(), end.reason.clone());

    // A job asked to stop goes out on no further road, whatever its turn came to.
    let stopped = match road_stopped(&end) {
      Some(stopped) if !called_off(ground, job.id).await => stopped,
      _ => return Ok(Some(("ended".to_owned(), outcome, with_roads(report, &roads), spent))),
    };

    noted_line(
      ground,
      job.id,
      &format!("The job's road {} stopped answering: {stopped}", taken.words()),
    )
    .await;
    road::leave(ground, &taken, &stopped).await;
    prompt = continued(&taken, &stopped, job.prompt);
    left.push(Closed::plain(&taken.provider, stopped));
    last = Some(("ended".to_owned(), outcome, report, None));
  }
}

/// The agent a job's item names, as its row keeps it from the launch (docs/repositories.md, 「An agent an item
/// names」); none where its item named none.
pub async fn item_agent_of_job(ground: &Ground, id: i64) -> Option<String> {
  one(&ground.db, r#"SELECT "item_agent" FROM "jobs" WHERE "id" = $1"#, vec![id.into()])
    .await
    .ok()
    .flatten()?
    .try_get::<Option<String>>("", "item_agent")
    .ok()
    .flatten()
    .filter(|name| !name.is_empty())
}

/// The model a job's item names, as its row keeps it from the launch (docs/repositories.md, 「A model an item
/// names」); none where its item named none.
pub async fn item_model_of_job(ground: &Ground, id: i64) -> Option<Pin> {
  let row = one(
    &ground.db,
    r#"SELECT "item_provider", "item_model" FROM "jobs" WHERE "id" = $1"#,
    vec![id.into()],
  )
  .await
  .ok()
  .flatten()?;

  Pin::of_parts(
    row.try_get("", "item_provider").ok().flatten(),
    row.try_get("", "item_model").ok().flatten(),
  )
}

/// The agent the item `POST /jobs` is handed names with its `agent` label (docs/repositories.md, 「An agent an
/// item names」): 422 `agent-unnamed` for none or `auto`, listing the agents of the role it would run as so its
/// supervisor names one; 422 `unknown-agent` for one the agency does not register; and 422 `agent-names-no-road`
/// for one that follows a supervisor's road, since no supervisor asks a job. A refusal is told to the item's
/// orderer once for that label.
async fn item_agent(
  ground: &Arc<Ground>,
  item_id: i64,
  item: &Value,
  role: &str,
) -> Result<crate::role_agents::RoleAgent, Refused> {
  let label = item["labels"][AGENT_LABEL].as_str().map(str::trim).unwrap_or_default();
  let db = &ground.db;
  let (sentence, code) = if label.is_empty() || label == crate::repositories::AUTO {
    let agents = crate::role_agents::listing(db, role).await?;
    let sentence = policy::fill_marks(
      &policy::job_review::JOB_REVIEW.agent_unnamed,
      &[("item", &format!("Item #{item_id}")), ("role", role), ("agents", &agents)],
    );

    (sentence, "agent-unnamed")
  } else {
    match crate::role_agents::named(db, label).await? {
      Some(agent) if agent.entry().is_some() => return Ok(agent),
      Some(_) => (
        format!(
          "Item #{item_id} names {AGENT_LABEL}={label}, which follows a supervisor's road and no supervisor asks a job"
        ),
        "agent-names-no-road",
      ),
      None => (
        format!("Item #{item_id} names {AGENT_LABEL}={label}, which is no agent the agency registers"),
        "unknown-agent",
      ),
    }
  };

  if !ground.testing() {
    let ground = ground.clone();
    let item = item.clone();
    let told = if label.is_empty() { crate::repositories::AUTO.to_owned() } else { label.to_owned() };
    let why = sentence.clone();

    tokio::spawn(async move {
      if let Err(fault) = crate::watches::queue_idle::tell_agent_refused(
        &ground.db,
        ground.vars(),
        item_id,
        &item,
        &told,
        &why,
      )
      .await
      {
        eprintln!("job of item #{item_id}: its refused agent was not told: {fault}");
      }
    });
  }

  Err(Refused::new(StatusCode::UNPROCESSABLE_ENTITY, sentence).with_code(code))
}

/// The role the item `POST /jobs` is handed names with its `role` label (docs/repositories.md, 「A role an item
/// names」): none for `auto` or no label, and 422 `unknown-role` with the label's text for one the store does not
/// hold.
async fn item_role(
  db: &sea_orm::DatabaseConnection,
  item_id: i64,
  item: &Value,
) -> Result<Option<String>, Refused> {
  let label = item["labels"][ROLE_LABEL].as_str().map(str::trim).unwrap_or_default();

  if label.is_empty() || label == crate::repositories::AUTO {
    return Ok(None);
  }

  match crate::agent_roles::named(db, label).await {
    Ok(Some(_)) => Ok(Some(label.to_owned())),
    Ok(None) => Err(
      Refused::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        format!("Item #{item_id} names {ROLE_LABEL}={label}, which is no role the store holds"),
      )
      .with_code("unknown-role"),
    ),
    Err(why) => Err(Refused::new(StatusCode::INTERNAL_SERVER_ERROR, why.to_string())),
  }
}

/// The role a job runs as: the one its row names, `worker` where it names none (docs/repositories.md, 「A
/// job」).
pub async fn role_of_job(ground: &Ground, id: i64) -> String {
  one(&ground.db, r#"SELECT "role" FROM "jobs" WHERE "id" = $1"#, vec![id.into()])
    .await
    .ok()
    .flatten()
    .and_then(|row| row.try_get::<String>("", "role").ok())
    .filter(|role| !role.is_empty())
    .unwrap_or_else(|| crate::agent_roles::job_role().to_owned())
}

// One line written into a job's log under its run.
async fn noted_line(ground: &Ground, id: i64, text: &str) {
  let Ok((member, _)) = job(&ground.db, id).await else {
    return;
  };
  let mut log = RunLog::new(Target {
    agent: member["workerName"].as_str().unwrap_or_default().to_owned(),
    run: member["runName"].as_str().map_or_else(|| format!("job-{id}"), str::to_owned),
    chat_id: member["roomId"].as_i64(),
    job_id: Some(id),
  });

  log.write(&ground.db, Line::note(text.to_owned())).await;
}

// How often a job waiting for its role's slot asks for it again: the line counts asks, never time
// (docs/role-slots.md, 「The line」).
const SLOT_ASK_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

// A job's turns, each a run of its role at its checkout (docs/agents.md, 「A run」): on the work parts of its
// road, holding a slot of its role while it goes, in a session of the role's started for the turn and ended
// with it, with a run of the ledger opened for it and closed as it ends.
struct OnSession<'a> {
  ground: &'a Ground,
  sessions: crate::herdr_backend::Shared,
  member: &'a Value,
  worker: &'a str,
  role: &'a str,
  // The one agent its item named, where it named one.
  agent: Option<&'a str>,
  room: i64,
  name: &'a str,
  title: &'a str,
  roads: usize,
  // When this turn of the job began, which keeps each turn's slot requests apart from those of the job's
  // earlier turns, a review asking for several (docs/job-review.md, 「The turns the review asks for」).
  began: i64,
  // The session a turn of the job's review continues, where the job's last turn left one.
  resume: Option<crate::job_review::Resumed>,
  // The prompt a turn of its review goes out with where that session cannot be continued on the road taken.
  carried: Option<String>,
}

impl OnSession<'_> {
  // The seat one turn of the job goes out on, on its road and keyed by its checkout so one checkout carries one
  // run at a time.
  async fn slot(&self, id: i64, taken: &Road, parts: &WorkParts) -> Result<Option<Request>, String> {
    let fixed = crate::order::Entry {
      provider: taken.provider.clone(),
      model: taken.model.clone(),
      effort: parts.mapping.effort.clone(),
      thinking: None,
      answer_tokens: parts.mapping.answer_tokens.and_then(|tokens| i64::try_from(tokens).ok()),
    };
    let asked = dispatch::Asked {
      request_key: format!("job-{id}-{}-{}", self.began, self.roads),
      owner: format!("{}{id}", dispatch::JOB_OWNER),
      wanted: dispatch::Wanted {
        role: self.role,
        path: Some(&parts.cwd),
        fixed: Some(fixed),
        agent: self.agent,
      },
      place_key: Some(self.worker),
      title: Some(self.title),
      ..dispatch::Asked::default()
    };

    job_seat(self.ground, id, &asked).await
  }
}

/// The seat a turn of a job goes out on (docs/role-slots.md, 「The line」). The seat the job already holds is
/// taken as it stands, asking nothing, so a later turn of the job — a stage handed over, a question or a rework
/// of its review — runs before any other job waiting for its agent. A job that holds none, at its first turn or
/// after its review paused it, asks for a slot of its role and asks again until the line lets it in, however
/// long that takes. None where the job was stopped while it waited, its ask then cancelled.
///
/// The lease is bound to no run: the job's turns each start and end a session of their own and its review waits
/// with none, so a lease bound to a turn's run would be given back as that turn ended. Only the job's end, or
/// its review's pause, gives it back.
pub async fn job_seat(
  ground: &Ground,
  id: i64,
  asked: &dispatch::Asked<'_>,
) -> Result<Option<Request>, String> {
  if let Some(kept) = kept_seat(ground, id).await? {
    return Ok(Some(kept));
  }

  let mut held =
    dispatch::enqueue(&ground.db, &ground.events, asked, dispatch::JOB, async |_, _| Ok(())).await?;
  let mut told = false;

  loop {
    paused_seats_given_back(ground).await;

    if let Some(let_in) = dispatch::acquire(ground, &held).await? {
      if told {
        hold_job(ground, id, None, false).await;
      }

      return Ok(Some(let_in));
    }

    // Not let in at its first ask: the job names the wait as its hold until it is let in, and the wait is
    // told to the item's orderer once.
    if !told {
      told = true;

      let role = asked.wanted.role;
      let why = match asked.wanted.agent {
        Some(agent) => format!("it waits for the agent {agent} of the {role} role"),
        None => format!("it waits for a slot of the {role} role"),
      };

      hold_job(ground, id, Some(&why), true).await;
    }

    if called_off(ground, id).await {
      dispatch::finish(&ground.db, &ground.events, &held.token, "cancelled: the job was stopped", true)
        .await?;

      return Ok(None);
    }

    tokio::time::sleep(SLOT_ASK_EVERY).await;
    held = dispatch::request(&ground.db, &held.token)
      .await?
      .ok_or("The job's slot request is gone")?;
  }
}

/// The seat a job holds now, kept from an earlier turn: its newest slot request still let in.
async fn kept_seat(ground: &Ground, id: i64) -> Result<Option<Request>, String> {
  let row = one(
    &ground.db,
    r#"SELECT "token" FROM "role_slot_requests" WHERE "owner" = $1 AND "state" = 'active'
       ORDER BY "sequence" DESC LIMIT 1"#,
    vec![format!("{}{id}", dispatch::JOB_OWNER).into()],
  )
  .await
  .map_err(|why| why.to_string())?;
  let Some(token) = row
    .map(|row| row.try_get::<String>("", "token"))
    .transpose()
    .map_err(|why| why.to_string())?
  else {
    return Ok(None);
  };

  dispatch::request(&ground.db, &token).await
}

/// The seats of jobs their review paused given back (docs/job-review.md, 「The owner's word」): a job whose owner's
/// question passed its due time gave its checkout up and answers for its agent no longer until it takes them
/// again, so a job waiting for that agent is let in on it.
pub async fn paused_seats_given_back(ground: &Ground) {
  let paused = all(
    &ground.db,
    r#"SELECT j."id" FROM "jobs" j
       WHERE j."state" = 'reviewing' AND j."review_parked"
         AND EXISTS (SELECT 1 FROM "role_slot_requests" r WHERE r."owner" = $1 || j."id" AND r."state" = 'active')"#,
    vec![dispatch::JOB_OWNER.into()],
  )
  .await;

  for row in paused.unwrap_or_default() {
    if let Ok(id) = row.try_get::<i64>("", "id") {
      crate::job_end::slot_given_back(
        &ground.db,
        &ground.events,
        id,
        "paused: its review waits on the owner",
      )
      .await;
    }
  }
}

impl Carrier for OnSession<'_> {
  async fn carry(&mut self, taken: &Road, entry: &Entry, prompt: &str) -> Result<TurnEnd, String> {
    let ground = self.ground;
    let member = self.member;
    let id = member["id"].as_i64().unwrap_or_default();
    let parts =
      role_work_parts(ground, self.role, self.worker, self.room, self.name, self.title, entry).await?;

    self.roads += 1;

    // The seat stays the job's past this turn: its end, or its review's pause, gives it back.
    let Some(_seat) = self.slot(id, taken, &parts).await? else {
      return Err(format!("job {id} was stopped while it waited for a slot of the {} role", self.role));
    };
    // A turn of the review continues the job's own session on the road that session stands on; on another road
    // it opens a new one, carrying the job's prompt and its review so far.
    let resume = self
      .resume
      .as_ref()
      .filter(|resumed| resumed.on(taken))
      .map(|resumed| resumed.session.clone());
    let prompt = match (&resume, &self.carried) {
      (None, Some(carried)) => carried.as_str(),
      _ => prompt,
    };
    let instead = resume.as_ref().and(self.carried.clone());
    let carried = self.on_session(id, taken, &parts, prompt, resume, instead).await;

    if let Ok(end) = &carried {
      crate::job_review::session_left(&ground.db, id, taken, end).await;
    }

    carried
  }
}

impl OnSession<'_> {
  // One turn of the job in a session of its role started for it at the checkout on the road, the session
  // ended with the turn.
  async fn on_session(
    &self,
    id: i64,
    taken: &Road,
    parts: &WorkParts,
    prompt: &str,
    resume: Option<String>,
    resume_else: Option<String>,
  ) -> Result<TurnEnd, String> {
    let ground = self.ground;
    let member = self.member;
    let session = self
      .sessions
      .start(Start {
        agent: self.worker.to_owned(),
        scope: Scope::Apart(Apart {
          provider: taken.provider.clone(),
          model: taken.model.clone(),
          effort: parts.mapping.effort.clone(),
          cwd: parts.cwd.clone(),
          role: Some(self.role.to_owned()),
          resume,
          resume_else,
          ..Apart::default()
        }),
      })
      .await
      .map_err(|fault| fault.message)?;
    let ledger = run_ledger::open(
      &ground.db,
      &Opening {
        agent: self.worker.to_owned(),
        chat_id: member["roomId"].as_i64(),
        unit: self.name.to_owned(),
        depth: 0,
        cap_ms: None,
        started_at: now_ms(),
        started_by: Some(member["holderName"].as_str().unwrap_or_default().to_owned()),
        search_id: None,
        round: None,
        message_id: None,
      },
    )
    .await
    .map_err(|why| why.detail)?;
    let marked = run(
      &ground.db,
      r#"UPDATE "jobs" SET "state" = 'running', "session_id" = $1, "run_name" = $2, "model" = $3, "run_id" = $5
         WHERE "id" = $4"#,
      vec![
        session.id.0.clone().into(),
        self.name.to_owned().into(),
        parts.mapping.model.clone().into(),
        id.into(),
        ledger.into(),
      ],
    )
    .await;

    // A run opened for a turn that never goes out is closed with it.
    if let Err(why) = marked {
      closed(ground, ledger, "failed", None, None).await;
      self.ended(&session.id).await;
      return Err(why.detail);
    }

    if let Ok((member, _)) = job(&ground.db, id).await {
      publish(ground, "job.changed", member);
    }

    let sent = self
      .sessions
      .send_turn(Turn {
        session: session.id.clone(),
        prompt: prompt.to_owned(),
        context: TurnContext {
          job: Some(JobMark {
            job: id,
            item: member["itemId"].as_i64(),
          }),
          work: Some(parts.clone()),
          ..TurnContext::default()
        },
      })
      .await;

    self.ended(&session.id).await;

    let end = match sent {
      Ok(end) => end,
      Err(fault) => {
        closed(ground, ledger, "failed", None, None).await;
        return Err(fault.message);
      }
    };
    let outcome = match end.outcome {
      agent_session::Outcome::Answered => "answered".to_owned(),
      other => format!("{other:?}").to_lowercase(),
    };

    closed(
      ground,
      ledger,
      &outcome,
      end.reply.as_ref().and_then(|reply| reply.context_tokens),
      turn_spent(&end),
    )
    .await;

    Ok(end)
  }

  // A turn's session ended with it, its place closed (docs/agents.md, 「A run in a pane of its own」).
  async fn ended(&self, session: &SessionId) {
    if let Err(fault) = self.sessions.end(session).await {
      eprintln!("{}: the job's session was not ended: {}", session.0, fault.message);
    }
  }
}

async fn launched(
  ground: &Ground,
  sessions: Option<crate::herdr_backend::Shared>,
  id: i64,
  turn: Option<crate::job_review::Turn>,
) -> Result<Option<Ended>, String> {
  let (member, prompt) = job(&ground.db, id).await.map_err(|why| why.error)?;
  let worker_name = member["workerName"].as_str().unwrap_or_default().to_owned();
  let holder = member["holderName"].as_str().unwrap_or_default().to_owned();
  let item = member["itemId"].as_i64().unwrap_or_default();

  println!("job {id}: starting on worker {worker_name} for item {item}");

  let role = role_of_job(ground, id).await;
  let room = member["roomId"].as_i64().unwrap_or(0);
  let title = prompt.lines().next().unwrap_or_default().trim_start_matches("# ").to_owned();
  let name = format!("job-{id}");
  let Some(sessions) = sessions else {
    return Err("agency stands no places to carry sessions on".to_owned());
  };

  // The gate picks the reference a job's instruction calls for, and the run carries it in its first
  // prompt; a job with no instruction stands as it was. The first prompt closes with what the review opens
  // with; a turn the review asks for goes out with its own words instead.
  let (prompt, resume, carried) = match turn {
    None => {
      let prompt = referenced(ground, member.get("instruction"), &prompt).await;
      let opening = crate::job_review::opened(ground, id).await;

      (format!("{prompt}\n\n{opening}"), None, None)
    }
    Some(turn) => {
      let carried = crate::job_review::carried(ground, &member, &prompt, &turn.words).await;

      (turn.words, turn.resume, Some(carried))
    }
  };
  let item_agent = item_agent_of_job(ground, id).await;
  let mut carrier = OnSession {
    ground,
    sessions,
    member: &member,
    worker: &worker_name,
    role: &role,
    agent: item_agent.as_deref(),
    room,
    name: &name,
    title: &title,
    roads: 0,
    began: now_ms(),
    resume,
    carried,
  };

  carry_job(
    ground,
    &Carried {
      id,
      worker: &worker_name,
      holder: &holder,
      prompt: &prompt,
    },
    &mut carrier,
  )
  .await
}

// What a job's turn's end makes of the job, its run closed with it.
async fn ended_of(ground: &Ground, ledger: i64, end: agent_session::TurnEnd) -> Ended {
  let spent = turn_spent(&end);
  let context = end.reply.as_ref().and_then(|reply| reply.context_tokens);
  let (outcome, report) = ended_by(end.outcome, end.answer, end.reason);

  closed(ground, ledger, outcome.as_deref().unwrap_or("answered"), context, spent).await;

  ("ended".to_owned(), outcome, report, spent)
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
    finished_at: now_ms(),
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
    let asked = job_search(&call)?;
    // After a cursor the nearest jobs are the oldest of the newer ones, so they are read upward.
    let upward = asked.after.is_some();
    let sql = format!(
      r#"SELECT {JOB_COLUMNS} FROM "jobs"
         WHERE ($1::bigint IS NULL OR "id" > $1) AND ($2::bigint IS NULL OR "id" < $2)
           AND ($3::text IS NULL OR "worker_name" = $3) AND ($4::text IS NULL OR "holder_name" = $4)
           AND ($5::jsonb = '[]'::jsonb OR "state" IN (SELECT jsonb_array_elements_text($5::jsonb)))
           AND ($6::boolean IS NULL OR ("state" IN ('starting', 'running', 'reviewing')) = $6)
           AND ($7::bigint IS NULL OR "room_id" = $7) AND ($8::bigint IS NULL OR "item_id" = $8)
           AND ($10::jsonb IS NULL OR "worker_name" IN (SELECT jsonb_array_elements_text($10::jsonb)))
           AND ($11::text IS NULL OR "role" = $11)
         ORDER BY "id" {} LIMIT $9"#,
      if upward { "ASC" } else { "DESC" }
    );
    let states = serde_json::to_string(&asked.states).unwrap_or_else(|_| "[]".to_owned());
    let rows = all(
      db,
      &sql,
      vec![
        asked.after.into(),
        asked.before.into(),
        asked.worker.into(),
        asked.holder.into(),
        states.into(),
        asked.running.into(),
        asked.room.into(),
        asked.item.into(),
        ((asked.limit + 1) as i64).into(),
        checkouts_named(db, asked.repository.as_deref()).await?.into(),
        asked.role.into(),
      ],
    )
    .await?;
    let mut items: Vec<Value> = rows.iter().map(job_value).collect::<Result<_, _>>()?;
    let more = items.len() > asked.limit;

    name_repositories(db, &mut items).await?;

    items.truncate(asked.limit);

    if upward {
      items.reverse();
    }

    Ok(answer(StatusCode::OK, page(items, more)))
  }
}

/// Every name the jobs' list is asked by (docs/api.md, 「assignments and jobs」).
const JOB_FILTERS: [&str; 11] = [
  "running",
  "itemId",
  "roomId",
  "repositoryName",
  "roleName",
  "workerName",
  "holderName",
  "state",
  "after",
  "before",
  "limit",
];

// What the jobs' list is narrowed by and where its page stands, as the query names it.
struct JobSearch {
  after: Option<i64>,
  before: Option<i64>,
  limit: usize,
  running: Option<bool>,
  states: Vec<String>,
  item: Option<i64>,
  room: Option<i64>,
  repository: Option<String>,
  worker: Option<String>,
  role: Option<String>,
  holder: Option<String>,
}

fn job_search(call: &Call) -> Result<JobSearch, Refused> {
  if let Some(name) = call.query_names().into_iter().find(|name| !JOB_FILTERS.contains(&name.as_str())) {
    return Err(bad(format!("the jobs are not narrowed by {name}")));
  }

  let named =
    |name: &str| call.query(name).map(|value| value.trim().to_owned()).filter(|value| !value.is_empty());
  let number = |name: &str| {
    named(name)
      .map(|value| value.parse::<i64>().map_err(|_| bad(format!("{name} is a whole number"))))
      .transpose()
  };
  let running = named("running")
    .map(|value| match value.as_str() {
      "true" => Ok(true),
      "false" => Ok(false),
      _ => Err(bad("running is true or false")),
    })
    .transpose()?;
  let limit = match number("limit")? {
    Some(limit) if limit < 1 => return Err(bad("limit is a whole number of at least 1")),
    Some(limit) => usize::try_from(limit).unwrap_or(500).min(500),
    None => 50,
  };

  Ok(JobSearch {
    after: number("after")?,
    before: number("before")?,
    limit,
    running,
    states: named("state")
      .map(|states| {
        states
          .split(',')
          .map(str::trim)
          .filter(|state| !state.is_empty())
          .map(str::to_owned)
          .collect()
      })
      .unwrap_or_default(),
    item: number("itemId")?,
    room: number("roomId")?,
    repository: named("repositoryName"),
    worker: named("workerName"),
    role: named("roleName"),
    holder: named("holderName"),
  })
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

// How often a stop looks again whether the job's turn has ended.
const STOP_LOOK: std::time::Duration = std::time::Duration::from_millis(250);

/// A job stopped by asking its turn to end (docs/api.md, 「assignments and jobs」): the reason kept as its stop
/// mark and its run cancelled, then waited on, however long the turn's own ending takes, until the turn's
/// end has written the job stopped with that reason. A job no turn carries is written stopped at once.
pub async fn stop_job(
  ground: &Ground,
  stops: &crate::queue::stops::Stops,
  id: i64,
  reason: &str,
) -> Result<Value, Refused> {
  let db = &ground.db;
  let (member, _) = job(db, id).await?;
  let state = member["state"].as_str().unwrap_or_default();

  if !matches!(state, "starting" | "running" | "reviewing") {
    return Err(Refused::new(StatusCode::CONFLICT, format!("job {id} is {state}")));
  }

  let run_name = member["runName"].as_str().filter(|name| !name.is_empty()).map(str::to_owned);
  let kept = match &run_name {
    Some(name) => crate::turn_pickup::kept(db, name).await,
    None => false,
  };

  crate::queue::stops::mark_job_stopped(db, id, reason).await?;

  if let Some(name) = &run_name {
    stops.cancel(name).await?;
  }

  if is_carrying(id) || kept {
    // The turn has ended once the job's end is written and nothing carries or keeps the turn any more, so
    // the worker reads idle by the time the stop answers.
    loop {
      let (member, _) = job(db, id).await?;
      let held = match &run_name {
        Some(name) => is_carrying(id) || crate::turn_pickup::kept(db, name).await,
        None => is_carrying(id),
      };

      if !held && !matches!(member["state"].as_str(), Some("starting" | "running" | "reviewing")) {
        if let Some(name) = &run_name {
          let _ = crate::queue::cancels::take_back(db, name).await;
        }

        return Ok(member);
      }

      tokio::time::sleep(STOP_LOOK).await;
    }
  }

  // No turn carries it: the stop is written here, as the turn's end would have written it.
  println!("job {id}: stop consumed: no turn carried it: {}", reason.trim());
  let _ = crate::queue::stops::clear_job_stop(db, id).await;
  let now = now_ms();

  run(
    db,
    r#"UPDATE "jobs" SET "state" = 'stopped', "outcome" = 'stopped', "report" = $1, "ended_at" = $2
       WHERE "id" = $3 AND "state" IN ('starting', 'running', 'reviewing')"#,
    vec![reason.into(), now.into(), id.into()],
  )
  .await?;
  crate::job_end::slot_given_back(db, &ground.events, id, &format!("stopped: {reason}")).await;
  release(db, member["assignmentId"].as_i64().unwrap_or_default()).await?;

  if let Some(name) = &run_name {
    let _ = crate::queue::cancels::take_back(db, name).await;
  }

  let (member, _) = job(db, id).await?;

  crate::job_end::delivered(ground, &member).await;
  publish(ground, "job.changed", member.clone());

  Ok(member)
}

/// A head pane that died: every job its worker is running ends failed with the pane's exit code and last
/// stderr line as its report; its lease is given back by the ordinary end.
pub async fn pane_died(ground: &Ground, worker: &str, code: i32, last: &str) -> Result<Vec<i64>, String> {
  let rows = all(
    &ground.db,
    r#"SELECT "id" FROM "jobs" WHERE "worker_name" = $1 AND "state" IN ('starting', 'running') ORDER BY "id""#,
    vec![worker.into()],
  )
  .await
  .map_err(|why| why.to_string())?;
  let mut ended = vec![];
  let reason = format!(
    "The pane for {worker} died with exit code {code}. Its last stderr line: {}",
    if last.trim().is_empty() { "none" } else { last.trim() }
  );

  for row in rows {
    let Ok(id) = row.try_get::<i64>("", "id") else { continue };

    job_turn_ended(ground, id, agent_session::TurnEnd::failed("pane-death", &reason, None)).await;
    ended.push(id);
  }

  Ok(ended)
}

/// A pane's death told to agency: the worker, its exit code and the last line it wrote on stderr.
pub struct PaneDeath;

impl Route for PaneDeath {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/pane-deaths";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let worker = call.query("agent").unwrap_or_default();
    let code: i32 = call
      .query("code")
      .unwrap_or_default()
      .parse()
      .map_err(|_| bad("code is a whole number"))?;
    let last = call.query("last").unwrap_or_default();

    if worker.trim().is_empty() {
      return Err(bad("agent is required"));
    }

    match pane_died(call.ground(), &worker, code, &last).await {
      Ok(jobs) => Ok(answer(StatusCode::ACCEPTED, json!({ "jobs": jobs }))),
      Err(why) => Err(Refused::failed(why)),
    }
  }
}

crate::moved!(PaneDeath);

/// End a job with its outcome, or stop it, its turn asked to end first.
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

    if wanted == "stopped" {
      let reason = report.unwrap_or_else(|| STOPPED.to_owned());

      return Ok(answer(StatusCode::OK, stop_job(call.ground(), &call.stops(), id, &reason).await?));
    }

    run(
      db,
      r#"UPDATE "jobs" SET "state" = $1, "outcome" = COALESCE($2, "outcome"),
              "report" = COALESCE($3, "report"), "ended_at" = $4 WHERE "id" = $5"#,
      vec![wanted.into(), outcome.clone().into(), report.into(), now.into(), id.into()],
    )
    .await?;
    crate::job_end::slot_given_back(db, call.events(), id, &ending(wanted, outcome.as_deref())).await;
    let assignment_id = member["assignmentId"].as_i64().unwrap_or_default();

    if let Err(why) = release(db, assignment_id).await {
      eprintln!("job {id}: its assignment {assignment_id} was not released: {}", why.error);
    }

    let (member, _) = job(db, id).await?;

    crate::job_end::delivered(call.ground(), &member).await;
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
