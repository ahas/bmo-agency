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
use crate::queue::routes::{block_route, clear_route_block, item_route_open, route_blocked};
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

/// A selection's road blocked until a moment, in epoch milliseconds, or opened again where none is given.
pub struct RoadBlock;

impl Route for RoadBlock {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/road-blocks";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let selection = text(&body, "selection")?;

    match body.get("until").and_then(whole) {
      Some(until) => {
        let reason = body.get("reason").and_then(Value::as_str).unwrap_or_default();
        let blocked = block_route(call.store(), selection, until, reason).await?;

        Ok(answer(
          StatusCode::CREATED,
          json!({ "id": blocked, "selection": selection, "until": until, "reason": reason, "blocked": true }),
        ))
      }
      None => {
        clear_route_block(call.store(), selection).await?;

        Ok(answer(StatusCode::OK, json!({ "blocked": false })))
      }
    }
  }
}

crate::moved!(RoadBlock);

/// Whether a selection's road is shut now, and whether an item's job has a road open now.
pub struct RoadOpen;

impl Route for RoadOpen {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/road-blocks";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let judging = Judging::of(call.ground());
    let now = now_ms();
    let (model, worker) = (call.query("model"), call.query("workerName").or_else(|| call.query("worker")));
    let open = item_route_open(judging, call.machine(), model.as_deref(), worker.as_deref(), now).await;

    Ok(answer(StatusCode::OK, json!({ "items": [], "more": false, "open": open })))
  }
}

crate::moved!(RoadOpen);

/// A road opened again before its block ran out.
pub struct RoadUnblock;

impl Route for RoadUnblock {
  const METHOD: Method = Method::DELETE;
  const PATH: &'static str = "/road-blocks/{blockId}";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let block = call.param("blockId").unwrap_or_default();

    clear_route_block(call.store(), block).await?;

    Ok(StatusCode::NO_CONTENT.into_response())
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

/// Usage by model, agent, user or room over a span, with session contexts.
pub struct UsageRoute;

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
    let group_by = call.query("groupBy").unwrap_or_else(|| "model".to_owned());

    Ok(answer(
      StatusCode::OK,
      json!({ "groups": [], "after": after, "before": before, "groupBy": group_by }),
    ))
  }
}

crate::moved!(UsageRoute);

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
