//! A checkout of another repository lent to a running job (docs/api.md, 「assignments and jobs」;
//! docs/worker-slots.md, 「A job writes only in its own checkout」): a free support worktree first, the base
//! checkout where none is, each on the machine the job's own checkout stands on, held as an assignment in the
//! job's name, so everything that reads what is held reads it and nothing is launched into it. It stands until
//! the job no longer runs. The job's own program asks for it with its session token, since it holds no control
//! token, and is lent only for the job running in its own checkout.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use bmo_agency_client::{BorrowAsk, Borrowing};
use store::{all, one};

use super::work::{JOB_HOLDER, hold, is_free, release_ended};
use super::{Bearer, Call, Refused, Route, answer};
use crate::queue::workers::{Worker, workers};

/// The repository's checkouts on the machine the job runs on, in the order a job takes them.
pub fn candidates<'a>(known: &'a [Worker], repository: &str, node: Option<&str>) -> Vec<&'a Worker> {
  let of = |checkout: &&Worker| {
    (checkout.name == repository || checkout.support_of.as_deref() == Some(repository))
      && checkout.node.as_deref() == node
      && !checkout.builtin
  };
  let support = known.iter().filter(of).filter(|checkout| checkout.is_support());
  let base = known.iter().filter(of).filter(|checkout| !checkout.is_support());

  support.chain(base).collect()
}

fn refused(status: StatusCode, code: &str, error: String) -> Refused {
  Refused::new(status, error).with_code(code)
}

fn lent(status: StatusCode, checkout: &Worker, assignment_id: i64) -> Result<Response, Refused> {
  let borrowing = Borrowing {
    worker_name: checkout.name.clone(),
    cwd: checkout.cwd.clone(),
    assignment_id,
  };

  Ok(answer(status, serde_json::to_value(borrowing).map_err(Refused::failed)?))
}

/// A checkout lent to the job working in the run the ask names.
pub struct Borrow;

impl Route for Borrow {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/borrowings";
  const TAKES_RUN_TOKEN: bool = true;

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let bad = |why: String| refused(StatusCode::BAD_REQUEST, "bad-request", why);
    let ask: BorrowAsk =
      serde_json::from_value(call.body().await?).map_err(|why| bad(format!("a borrowing: {why}")))?;
    let run_name = ask.run_name.trim();
    let repository = ask.repository_name.trim();

    if run_name.is_empty() || repository.is_empty() {
      return Err(bad("a borrowing names its runName and its repositoryName".to_owned()));
    }

    let db = call.store();
    let job = one(
      db,
      r#"SELECT "id", "item_id", "worker_name" FROM "jobs"
         WHERE "run_name" = $1 AND "state" IN ('starting', 'running')
         ORDER BY "id" DESC LIMIT 1"#,
      vec![run_name.into()],
    )
    .await?
    .ok_or_else(|| refused(StatusCode::NOT_FOUND, "no-job", format!("no job runs under {run_name}")))?;
    let job_id: i64 = job.try_get("", "id")?;
    let item_id: i64 = job.try_get("", "item_id")?;
    let own: String = job.try_get("", "worker_name")?;

    if let Bearer::Run(holder) = call.bearer()
      && *holder != own
    {
      return Err(refused(
        StatusCode::FORBIDDEN,
        "not-this-job",
        format!("the session token is {holder}'s, and the job working in {run_name} runs in {own}"),
      ));
    }

    release_ended(db).await?;

    let known = workers(db, &call.ground().source).await?;
    let own_checkout = known.iter().find(|checkout| checkout.name == own);
    let own_base = own_checkout.map_or(own.as_str(), |checkout| {
      checkout.support_of.as_deref().unwrap_or(checkout.name.as_str())
    });

    if own_base == repository {
      return Err(refused(
        StatusCode::UNPROCESSABLE_ENTITY,
        "own-repository",
        format!("{repository} is this job's own repository; work in this job's own checkout"),
      ));
    }

    let node = own_checkout.and_then(|checkout| checkout.node.as_deref());
    let members = candidates(&known, repository, node);

    if members.is_empty() {
      return Err(refused(
        StatusCode::UNPROCESSABLE_ENTITY,
        "no-checkout-here",
        format!("{repository} is no repository with a checkout on this job's machine"),
      ));
    }

    let holder = format!("{JOB_HOLDER}{job_id}");

    // A checkout of the repository this job holds already is the answer, asked twice.
    for row in all(
      db,
      r#"SELECT "id", "worker_name" FROM "assignments" WHERE "holder_name" = $1 AND "state" = 'held'"#,
      vec![holder.clone().into()],
    )
    .await?
    {
      let worker: String = row.try_get("", "worker_name")?;

      if let Some(held) = members.iter().find(|member| member.name == worker) {
        return lent(StatusCode::OK, held, row.try_get("", "id")?);
      }
    }

    for member in &members {
      if is_free(db, &member.name).await? {
        let assignment =
          hold(call.ground(), item_id, &holder, &member.name, crate::agent_roles::job_role()).await?;

        return lent(StatusCode::CREATED, member, assignment["id"].as_i64().unwrap_or_default());
      }
    }

    let names: Vec<&str> = members.iter().map(|member| member.name.as_str()).collect();

    Err(refused(
      StatusCode::UNPROCESSABLE_ENTITY,
      "no-worker-free",
      format!(
        "every checkout of {repository} is at work ({}); read which with bmo run list, and borrow once one is free",
        names.join(", ")
      ),
    ))
  }
}

crate::moved!(Borrow);
