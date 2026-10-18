//! The workers' routes across the boundary (docs/api.md, 「Workers」): a job's stop mark, a road's block and
//! whether an item has a road open now, runs asked to stop, which names are alive, and the deployment's
//! rest. Agency keeps each mark and each block and judges nothing of what a consumer's job is.

use axum::http::{Method, StatusCode};
use axum::response::Response;
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
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/jobs/stop-mark";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;

    mark_job_stopped(call.store(), job_of(body.get("job"))?, text(&body, "reason")?).await?;

    Ok(ok())
  }
}

crate::moved!(JobStopMark);

/// A job's stop mark, where one stands.
pub struct JobStopRead;

impl Route for JobStopRead {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/jobs/stop-mark";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let job = call
      .query("job")
      .and_then(|job| job.parse::<i64>().ok())
      .ok_or_else(|| bad("job is required"))?;

    Ok(answer(StatusCode::OK, json!({ "reason": job_stop_reason(call.store(), job).await? })))
  }
}

crate::moved!(JobStopRead);

/// A job's stop mark taken away with its record.
pub struct JobStopClear;

impl Route for JobStopClear {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/jobs/stop-mark/clear";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;

    clear_job_stop(call.store(), job_of(body.get("job"))?).await?;

    Ok(ok())
  }
}

crate::moved!(JobStopClear);

/// A selection's road blocked until a moment, in epoch milliseconds, or opened again where none is given.
pub struct RoadBlock;

impl Route for RoadBlock {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/roads/block";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let selection = text(&body, "selection")?;

    match body.get("until").and_then(whole) {
      Some(until) => {
        let reason = body.get("reason").and_then(Value::as_str).unwrap_or_default();
        let blocked = block_route(call.store(), selection, until, reason).await?;

        Ok(answer(StatusCode::OK, json!({ "blocked": blocked })))
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
  const PATH: &'static str = "/roads/open";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let judging = Judging::of(call.ground());
    let now = now_ms();

    if let Some(selection) = call.query("selection") {
      let blocked = route_blocked(judging, &selection, now).await.map_err(|why| Refused::failed(why.0))?;

      return Ok(answer(StatusCode::OK, json!({ "selection": selection, "open": !blocked })));
    }

    let (model, worker) = (call.query("model"), call.query("worker"));
    let open = item_route_open(judging, call.machine(), model.as_deref(), worker.as_deref(), now).await;

    Ok(answer(StatusCode::OK, json!({ "open": open })))
  }
}

crate::moved!(RoadOpen);

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

crate::moved!(RunsStop);

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

crate::moved!(RunsAlive);

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
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/rest";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
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

    Ok(answer(StatusCode::OK, json!({ "silenced": silenced })))
  }
}

crate::moved!(RestWrite);

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

crate::moved!(RestPassed);
