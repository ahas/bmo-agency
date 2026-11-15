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

    if !block_route(call.store(), selection, until, reason, None).await? {
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

/// Usage by model, checkout, repository, holder, room or round over a span, with session contexts.
pub struct UsageRoute;

/// What a usage query groups the jobs by (docs/api.md, 「accounts, rest and usage」).
enum Grouping {
  /// A column of the job, matched to the roles' sessions on the column named beside it, where one is.
  Column {
    jobs: &'static str,
    sessions: Option<&'static str>,
  },
  /// The repository the job's checkout belongs to.
  Repository,
}

impl Grouping {
  fn named(word: &str) -> Option<Self> {
    let column = |jobs, sessions| Some(Self::Column { jobs, sessions });

    match word {
      "model" => column("model", Some("model")),
      "agent" => column("worker_name", Some("role")),
      "user" => column("holder_name", Some("account")),
      "room" => column("room_id", None),
      "repository" => Some(Self::Repository),
      _ => None,
    }
  }

  /// The groups the jobs started in the span make, the most tokens first.
  async fn groups(
    &self,
    db: &sea_orm::DatabaseConnection,
    after: i64,
    before: i64,
  ) -> Result<Vec<Value>, Refused> {
    let (jobs_by, sessions_by) = match self {
      Self::Column { jobs, sessions } => (*jobs, *sessions),
      Self::Repository => ("worker_name", None),
    };
    let contexts = match sessions_by {
      Some(column) => session_contexts(db, column, after, before).await?,
      None => std::collections::HashMap::new(),
    };
    let mut spends = JobSpend::by_column(db, jobs_by, after, before).await?;

    if let Self::Repository = self {
      spends = by_repository(db, spends).await?;
    }

    spends.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.key.cmp(&b.key)));

    Ok(spends.iter().map(|spend| spend.value(contexts.get(&spend.key).copied())).collect())
  }
}

/// The tokens the roles' sessions answered in the span carry, by the value of one of their columns.
async fn session_contexts(
  db: &sea_orm::DatabaseConnection,
  column: &str,
  after: i64,
  before: i64,
) -> Result<std::collections::HashMap<String, i64>, Refused> {
  let rows = store::all(
    db,
    &format!(
      r#"SELECT COALESCE(CAST("{column}" AS text), '') AS "key", COALESCE(SUM("context_tokens"), 0)::bigint AS "tokens"
         FROM "role_sessions" WHERE "answered_at" >= $1 AND "answered_at" <= $2
         GROUP BY 1"#
    ),
    vec![after.into(), before.into()],
  )
  .await?;

  rows
    .iter()
    .map(|row| Ok((row.try_get("", "key")?, row.try_get("", "tokens")?)))
    .collect::<Result<_, sea_orm::DbErr>>()
    .map_err(Refused::from)
}

/// The checkouts' spends folded into their repositories' (docs/worker-slots.md), a checkout no repository
/// holds standing as a group of its own.
async fn by_repository(
  db: &sea_orm::DatabaseConnection,
  checkouts: Vec<JobSpend>,
) -> Result<Vec<JobSpend>, Refused> {
  let held: std::collections::HashSet<String> =
    store::all(db, r#"SELECT "name" FROM "repositories""#, vec![])
      .await?
      .iter()
      .map(|row| row.try_get("", "name"))
      .collect::<Result<_, _>>()?;
  let mut repositories: std::collections::HashMap<String, JobSpend> = std::collections::HashMap::new();

  for checkout in checkouts {
    let repository = crate::queue::workers::repository_named_by(&checkout.key, |name| held.contains(name))
      .unwrap_or(&checkout.key)
      .to_owned();
    let spend = match repositories.remove(&repository) {
      Some(gathered) => gathered.joined(checkout),
      None => JobSpend {
        key: repository.clone(),
        ..checkout
      },
    };

    repositories.insert(repository, spend);
  }

  Ok(repositories.into_values().collect())
}

/// What the jobs of one group spent: the total, the four kinds summed over the jobs that reported all four,
/// and the jobs that did not with what they spent.
struct JobSpend {
  key: String,
  tokens: i64,
  jobs: i64,
  first_at: i64,
  last_at: i64,
  input_tokens: i64,
  output_tokens: i64,
  cache_read_tokens: i64,
  cache_write_tokens: i64,
  unsplit_jobs: i64,
  unsplit_tokens: i64,
}

impl JobSpend {
  /// The jobs started in the span, a spend to each value of one of their columns.
  async fn by_column(
    db: &sea_orm::DatabaseConnection,
    column: &str,
    after: i64,
    before: i64,
  ) -> Result<Vec<Self>, Refused> {
    let rows = store::all(
      db,
      &format!(
        r#"SELECT "key", SUM("token_count")::bigint AS "tokens", COUNT(*) AS "jobs",
                  MIN("started_at") AS "first", MAX(COALESCE("ended_at", "started_at")) AS "last",
                  COALESCE(SUM("input_tokens") FILTER (WHERE "split"), 0)::bigint AS "input",
                  COALESCE(SUM("output_tokens") FILTER (WHERE "split"), 0)::bigint AS "output",
                  COALESCE(SUM("cache_read_tokens") FILTER (WHERE "split"), 0)::bigint AS "cache_read",
                  COALESCE(SUM("cache_write_tokens") FILTER (WHERE "split"), 0)::bigint AS "cache_write",
                  COUNT(*) FILTER (WHERE NOT "split") AS "unsplit_jobs",
                  COALESCE(SUM("token_count") FILTER (WHERE NOT "split"), 0)::bigint AS "unsplit_tokens"
           FROM (
             SELECT COALESCE(CAST("{column}" AS text), '') AS "key", "token_count", "started_at", "ended_at",
                    "input_tokens", "output_tokens", "cache_read_tokens", "cache_write_tokens",
                    ("input_tokens" IS NOT NULL AND "output_tokens" IS NOT NULL
                      AND "cache_read_tokens" IS NOT NULL AND "cache_write_tokens" IS NOT NULL) AS "split"
             FROM "jobs" WHERE "started_at" >= $1 AND "started_at" <= $2
           ) AS "spent"
           GROUP BY "key""#
      ),
      vec![after.into(), before.into()],
    )
    .await?;

    rows.iter().map(Self::read).collect::<Result<_, _>>().map_err(Refused::from)
  }

  fn read(row: &sea_orm::QueryResult) -> Result<Self, sea_orm::DbErr> {
    let figure = |column: &str| row.try_get::<i64>("", column);

    Ok(Self {
      key: row.try_get("", "key")?,
      tokens: figure("tokens")?,
      jobs: figure("jobs")?,
      first_at: figure("first")?,
      last_at: figure("last")?,
      input_tokens: figure("input")?,
      output_tokens: figure("output")?,
      cache_read_tokens: figure("cache_read")?,
      cache_write_tokens: figure("cache_write")?,
      unsplit_jobs: figure("unsplit_jobs")?,
      unsplit_tokens: figure("unsplit_tokens")?,
    })
  }

  /// Two groups' spends as one, under this one's key.
  fn joined(self, other: Self) -> Self {
    Self {
      key: self.key,
      tokens: self.tokens + other.tokens,
      jobs: self.jobs + other.jobs,
      first_at: self.first_at.min(other.first_at),
      last_at: self.last_at.max(other.last_at),
      input_tokens: self.input_tokens + other.input_tokens,
      output_tokens: self.output_tokens + other.output_tokens,
      cache_read_tokens: self.cache_read_tokens + other.cache_read_tokens,
      cache_write_tokens: self.cache_write_tokens + other.cache_write_tokens,
      unsplit_jobs: self.unsplit_jobs + other.unsplit_jobs,
      unsplit_tokens: self.unsplit_tokens + other.unsplit_tokens,
    }
  }

  /// The group as the route answers it, with the context its sessions carry where they are matched.
  fn value(&self, context: Option<i64>) -> Value {
    json!({
      "key": self.key,
      "tokens": self.tokens,
      "jobs": self.jobs,
      "firstAt": self.first_at,
      "lastAt": self.last_at,
      "inputTokens": self.input_tokens,
      "outputTokens": self.output_tokens,
      "cacheReadTokens": self.cache_read_tokens,
      "cacheWriteTokens": self.cache_write_tokens,
      "unsplitJobs": self.unsplit_jobs,
      "unsplitTokens": self.unsplit_tokens,
      "contextTokens": context,
    })
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
    let groups = if word == "round" {
      by_round(call.store(), after, before).await?
    } else {
      Grouping::named(&word)
        .ok_or_else(|| {
          bad(format!("groupBy is model, agent, repository, user, room or round, not {word}"))
        })?
        .groups(call.store(), after, before)
        .await?
    };

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
          "unsplitJobs": 0,
          "unsplitTokens": 0,
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
