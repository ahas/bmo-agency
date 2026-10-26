#![allow(
  dead_code,
  unused_imports,
  unused_variables,
  clippy::needless_lifetimes,
  clippy::let_and_return,
  clippy::unnecessary_to_owned
)]
//! The workers' routes across the boundary (docs/api.md, 「Workers」): a job's stop mark, a road's block and
//! whether an item has a road open now, runs asked to stop, which names are alive, and the deployment's
//! rest. Agency keeps each mark and each block and judges nothing of what a consumer's job is.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::claude_accounts::Judging;
use super::{Call, Refused, Route, answer, whole};
use crate::now_ms;
use crate::queue::routes::{
  block_id, block_route, clear_block_by_id, kept_blocks, route_blocked, walked_keys,
};
use crate::queue::stops::{StoppedRun, clear_job_stop, job_stop_reason, mark_job_stopped};
use crate::rest::{RestKind, clear_passed_rests, rest_kind, rest_until, write_rest};

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

fn ok() -> Response {
  answer(StatusCode::OK, json!({ "ok": true }))
}

fn job_of(value: Option<&Value>) -> Result<i64, Refused> {
  value.and_then(whole).ok_or_else(|| bad("job (a whole number) is required"))
}

fn text<'a>(body: &'a Value, field: &str) -> Result<&'a str, Refused> {
  body
    .get(field)
    .and_then(Value::as_str)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| bad(format!("{field} (string) is required")))
}

fn names(body: &Value, field: &str) -> Vec<String> {
  body
    .get(field)
    .and_then(Value::as_array)
    .map(|names| names.iter().filter_map(Value::as_str).map(str::to_owned).collect())
    .unwrap_or_default()
}

/// A job marked stopped with its reason, before its unit is asked to stop.
pub struct JobStopMark;

impl Route for JobStopMark {
  const METHOD: Method = Method::PUT;
  const PATH: &'static str = "/stop-marks/{jobId}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let job: i64 = call
      .param("jobId")
      .and_then(|job| job.parse().ok())
      .ok_or_else(|| bad("jobId is a number"))?;
    let reason = text(&body, "reason")?;

    mark_job_stopped(call.store(), job, reason).await?;

    Ok(answer(StatusCode::CREATED, json!({ "jobId": job, "reason": reason, "at": now_ms() })))
  }
}

crate::moved!(JobStopMark);

/// A job's stop mark, where one stands.
pub struct JobStopRead;

impl Route for JobStopRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/stop-marks/{jobId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let job: i64 = call
      .param("jobId")
      .and_then(|job| job.parse().ok())
      .ok_or_else(|| bad("jobId is a number"))?;
    let reason = job_stop_reason(call.store(), job).await?;

    match reason {
      Some(reason) => Ok(answer(StatusCode::OK, json!({ "jobId": job, "reason": reason, "at": now_ms() }))),
      None => Err(Refused::new(StatusCode::NOT_FOUND, format!("job {job} carries no stop mark"))),
    }
  }
}

crate::moved!(JobStopRead);

/// A job's stop mark taken away with its record.
pub struct JobStopClear;

impl Route for JobStopClear {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/stop-marks/{jobId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let job: i64 = call
      .param("jobId")
      .and_then(|job| job.parse().ok())
      .ok_or_else(|| bad("jobId is a number"))?;

    clear_job_stop(call.store(), job).await?;

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(JobStopClear);

/// A selection's road blocked until a moment, in epoch milliseconds.
pub struct RoadBlock;

impl Route for RoadBlock {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/road-blocks";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let selection = text(&body, "selection")?;
    let until = body
      .get("until")
      .and_then(whole)
      .ok_or_else(|| bad("until (a whole number of epoch milliseconds) is required"))?;
    let reason = body.get("reason").and_then(Value::as_str).unwrap_or_default();

    if !block_route(call.store(), selection, until, reason).await? {
      return Err(bad(format!("{selection} names no provider's model this build carries")));
    }

    let id = block_id(call.store(), selection)
      .await?
      .ok_or_else(|| Refused::failed("the block was written but not read"))?;

    Ok(answer(
      StatusCode::CREATED,
      json!({ "id": id, "selection": selection, "until": until, "reason": reason }),
    ))
  }
}

crate::moved!(RoadBlock);

/// Every road blocked now, by the model or the worker that would walk it; none means open.
pub struct RoadOpen;

impl Route for RoadOpen {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/road-blocks";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let now = now_ms();
    let (model, worker) = (
      call.query("model").filter(|model| !model.trim().is_empty()),
      call
        .query("workerName")
        .or_else(|| call.query("worker"))
        .filter(|worker| !worker.trim().is_empty()),
    );
    let asked = model.is_some() || worker.is_some();
    let walked = if asked {
      Some(walked_keys(call.store(), call.machine(), model.as_deref(), worker.as_deref()).await?)
    } else {
      None
    };
    let items: Vec<Value> = kept_blocks(call.store())
      .await?
      .into_iter()
      .filter(|block| block.until > now)
      .filter(|block| walked.as_ref().is_none_or(|walked| walked.contains(&block.key)))
      .map(|block| {
        json!({
          "id": block.id,
          "selection": block.selection,
          "until": block.until,
          "reason": block.reason,
        })
      })
      .collect();

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": false })))
  }
}

crate::moved!(RoadOpen);

/// A road opened again before its block ran out.
pub struct RoadUnblock;

impl Route for RoadUnblock {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/road-blocks/{blockId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let id = call
      .param("blockId")
      .and_then(|id| id.parse::<i64>().ok())
      .ok_or_else(|| bad("blockId (a whole number) is required"))?;

    match clear_block_by_id(call.store(), id).await? {
      Some(_) => Ok(StatusCode::NO_CONTENT.into_response()),
      None => Err(Refused::new(StatusCode::NOT_FOUND, format!("no road block has the id {id}"))),
    }
  }
}

crate::moved!(RoadUnblock);

/// Runs asked to stop by their names: one at a time as `run`, or several at once as `runs`, each a unit and
/// the pool request it held, all confirmed gone first.
pub struct RunsStop;

impl Route for RunsStop {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/runs/stop";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let stops = call.stops();

    if let Some(run) = body.get("run").and_then(Value::as_str) {
      stops.stop(run).await;

      return Ok(ok());
    }

    let runs: Vec<StoppedRun> = body
      .get("runs")
      .and_then(Value::as_array)
      .ok_or_else(|| bad("run (string) or runs (array) is required"))?
      .iter()
      .map(|run| StoppedRun {
        unit: run.get("unit").and_then(Value::as_str).map(str::to_owned),
        pool_request: run.get("poolRequest").and_then(Value::as_str).map(str::to_owned),
      })
      .collect();
    let why = body.get("reason").and_then(Value::as_str).unwrap_or("stopped");

    stops
      .stop_runs(&runs, why)
      .await
      .map_err(|why| Refused::new(StatusCode::CONFLICT, why))?;

    Ok(ok())
  }
}

/// Which of the names asked are running, or that nobody could say.
pub struct RunsAlive;

impl Route for RunsAlive {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/runs/alive";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let asked = names(&body, "names");
    let alive = call.running().answered(&asked).await?;

    Ok(answer(
      StatusCode::OK,
      match alive {
        Some(alive) => {
          let mut alive: Vec<String> = alive.into_iter().collect();

          alive.sort();
          json!({ "answered": true, "alive": alive })
        }
        None => json!({ "answered": false, "alive": null }),
      },
    ))
  }
}

/// Whether the deployment rests now, until when and why.
pub struct RestRead;

impl Route for RestRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/rest";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let now = now_ms() / 1000;
    let until = rest_until(Judging::of(call.ground()), now).await?;
    let kind = match until {
      Some(_) => Some(rest_kind(call.store(), now).await?),
      None => None,
    };

    Ok(answer(
      StatusCode::OK,
      json!({ "resting": until.is_some(), "until": until, "kind": kind }),
    ))
  }
}

crate::moved!(RestRead);

/// A rest written down: an account's limit or the service's outage, until a moment in epoch seconds.
pub struct RestWrite;

impl Route for RestWrite {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/rest";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;

    if body.get("clearPassed").and_then(Value::as_bool) == Some(true) {
      let kind = clear_passed_rests(Judging::of(call.ground()), now_ms() / 1000).await?;

      return Ok(answer(
        StatusCode::OK,
        json!({ "resting": false, "until": Value::Null, "kind": kind.map(RestKind::word), "accountId": Value::Null }),
      ));
    }

    let until = body
      .get("until")
      .and_then(Value::as_f64)
      .ok_or_else(|| bad("until (seconds) is required"))?;
    let kind = match body.get("kind").and_then(Value::as_str) {
      Some("limit") => RestKind::Limit,
      Some("outage") => RestKind::Outage,
      _ => return Err(bad("kind is limit or outage")),
    };
    let account = body.get("account").and_then(Value::as_str).unwrap_or_default();
    let silenced = write_rest(Judging::of(call.ground()), until, kind, account, now_ms() / 1000).await?;

    Ok(answer(
      StatusCode::OK,
      json!({
        "resting": true,
        "until": until as i64,
        "kind": body.get("kind").cloned().unwrap_or(Value::Null),
        "accountId": body.get("account").cloned().unwrap_or(Value::Null),
        "silenced": silenced,
      }),
    ))
  }
}

crate::moved!(RestWrite);

/// Usage by model, agent, user, room or round over a span, with session contexts.
pub struct UsageRoute;

// The column a job is grouped by, and the column a worker's session is grouped by for the same key.
fn grouped_by(word: &str) -> Option<(&'static str, Option<&'static str>)> {
  match word {
    "model" => Some(("model", Some("model"))),
    "agent" => Some(("worker_name", Some("worker"))),
    "user" => Some(("holder_name", Some("account"))),
    "room" => Some(("room_id", None)),
    _ => None,
  }
}

impl Route for UsageRoute {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/usage";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let now = now_ms();
    let after = call
      .query("after")
      .and_then(|after| after.parse::<i64>().ok())
      .unwrap_or(now - 86_400_000);
    let before = call.query("before").and_then(|before| before.parse::<i64>().ok()).unwrap_or(now);
    let word = call.query("groupBy").unwrap_or_else(|| "model".to_owned());

    if word == "round" {
      return Ok(answer(
        StatusCode::OK,
        json!({ "groups": by_round(call.store(), after, before).await?, "after": after, "before": before }),
      ));
    }

    let (jobs_by, sessions_by) = grouped_by(&word)
      .ok_or_else(|| bad(format!("groupBy is model, agent, user, room or round, not {word}")))?;
    let mut contexts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    if let Some(column) = sessions_by {
      let rows = store::all(
        call.store(),
        &format!(
          r#"SELECT COALESCE(CAST("{column}" AS text), '') AS "key", COALESCE(SUM("context_tokens"), 0)::bigint AS "tokens"
             FROM "worker_sessions" WHERE "last_reply_at" * 1000 >= $1 AND "last_reply_at" * 1000 <= $2
             GROUP BY 1"#
        ),
        vec![after.into(), before.into()],
      )
      .await?;

      for row in rows {
        contexts.insert(row.try_get("", "key")?, row.try_get("", "tokens")?);
      }
    }

    let rows = store::all(
      call.store(),
      &format!(
        r#"SELECT COALESCE(CAST("{jobs_by}" AS text), '') AS "key", SUM("token_count")::bigint AS "tokens",
                  COUNT(*) AS "jobs", MIN("started_at") AS "first", MAX(COALESCE("ended_at", "started_at")) AS "last"
           FROM "jobs" WHERE "started_at" >= $1 AND "started_at" <= $2
           GROUP BY 1 ORDER BY "tokens" DESC, "key""#
      ),
      vec![after.into(), before.into()],
    )
    .await?;
    let groups: Vec<Value> = rows
      .iter()
      .map(|row| {
        let key: String = row.try_get("", "key")?;
        let context = contexts.get(&key);

        Ok::<_, sea_orm::DbErr>(json!({
          "key": key,
          "tokens": row.try_get::<i64>("", "tokens")?,
          "jobs": row.try_get::<i64>("", "jobs")?,
          "firstAt": row.try_get::<i64>("", "first")?,
          "lastAt": row.try_get::<i64>("", "last")?,
          "contextTokens": context,
        }))
      })
      .collect::<Result<_, _>>()?;

    Ok(answer(StatusCode::OK, json!({ "groups": groups, "after": after, "before": before })))
  }
}

crate::moved!(UsageRoute);

// What the chat turns' runs started in a span spent, a group to a round (docs/run-records.md, 「The run
// ledger」), the span in epoch milliseconds as the route's.
async fn by_round(
  db: &sea_orm::DatabaseConnection,
  after: i64,
  before: i64,
) -> Result<Vec<Value>, Refused> {
  let rounds = crate::run_ledger::spent_by_round(db, after, before).await?;

  Ok(
    rounds
      .iter()
      .map(|round| {
        json!({
          "key": round.round,
          "tokens": round.spent.total(),
          "jobs": 0,
          "turns": round.turns,
          "inputTokens": round.spent.input_tokens,
          "outputTokens": round.spent.output_tokens,
          "cacheReadTokens": round.spent.cache_read_tokens,
          "cacheWriteTokens": round.spent.cache_write_tokens,
          "firstAt": round.first_at,
          "lastAt": round.last_at,
          "contextTokens": Value::Null,
        })
      })
      .collect(),
  )
}

/// Why the rest that has just passed silenced the deployment, the passed rests cleared as they are read.
pub struct RestPassed;

impl Route for RestPassed {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/rest/passed";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let kind = clear_passed_rests(Judging::of(call.ground()), now_ms() / 1000).await?;

    Ok(answer(StatusCode::OK, json!({ "kind": kind.map(RestKind::word) })))
  }
}
