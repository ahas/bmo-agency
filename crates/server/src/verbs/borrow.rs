//! A checkout of another repository borrowed for the rest of a job (docs/worker-slots.md, 「A job writes only in
//! its own checkout」): `bmo job borrow`, run from the job's own shell. A free support worktree first, the base
//! checkout where none is, each on the job's own machine, taken as an assignment in the job's name, so everything
//! that reads what is held reads it and nothing is launched into it. It stands until the job no longer runs.

use std::path::Path;

use reqwest::Method;
use serde_json::json;
use store::one;

use super::{VerbError, control_call, control_token};
use crate::control::work::JOB_HOLDER;
use crate::queue::workers::{Worker, workers};

fn refused(why: impl std::fmt::Display) -> VerbError {
  VerbError::Ended {
    code: 1,
    said: format!("borrow: refused — {why}"),
  }
}

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

/// The named repository's checkout lent to the job this process runs for, and the lines saying where it is.
pub async fn run(data_root: &Path, repository: &str) -> Result<Vec<String>, VerbError> {
  let Some(run_name) = std::env::var("BMO_RUN_NAME").ok().filter(|name| !name.is_empty()) else {
    return Err(refused("a checkout is lent to a job, and this is not run inside one"));
  };
  let Some(token) = control_token() else {
    return Err(refused(format!("{} is not set in this process", super::CONTROL_TOKEN)));
  };
  let ground = super::ground(data_root).await?;
  let job = one(
    &ground.db,
    r#"SELECT "id", "item_id", "worker_name" FROM "jobs" WHERE "run_name" = $1 AND "state" IN ('starting', 'running')"#,
    vec![run_name.clone().into()],
  )
  .await?
  .ok_or_else(|| refused(format!("no job runs under {run_name}")))?;
  let job_id: i64 = job.try_get("", "id")?;
  let item: i64 = job.try_get("", "item_id")?;
  let own: String = job.try_get("", "worker_name")?;
  let known = workers(&ground.db, &ground.source).await?;
  let node = std::env::var("BMO_NODE").ok().filter(|node| !node.is_empty());
  let members = candidates(&known, repository, node.as_deref());

  if members.is_empty() {
    return Err(refused(format!("{repository} is no repository with a checkout on this job's machine")));
  }

  let own_base = known.iter().find(|checkout| checkout.name == own).map_or(own.as_str(), |checkout| {
    checkout.support_of.as_deref().unwrap_or(checkout.name.as_str())
  });

  if own_base == repository {
    return Err(refused(format!(
      "{repository} is this job's own repository; work in this job's own checkout"
    )));
  }

  let holder = format!("{JOB_HOLDER}{job_id}");

  // A checkout this job holds already is the answer, asked twice.
  let holding = one(
    &ground.db,
    r#"SELECT "worker_name" FROM "assignments" WHERE "holder_name" = $1 AND "state" = 'held'"#,
    vec![holder.clone().into()],
  )
  .await?
  .map(|row| row.try_get::<String>("", "worker_name"))
  .transpose()?;

  if let Some(held) = holding.and_then(|held| members.iter().find(|member| member.name == held).copied()) {
    return Ok(vec![format!("borrow: this job already has {}'s checkout: {}", held.name, held.cwd)]);
  }

  for member in &members {
    let body = json!({
      "itemId": item,
      "holderName": holder,
      "assigneeName": member.name,
      "item": { "id": item },
    });
    let (status, answered) = control_call(&token, Method::POST, "/assignments", Some(&body))
      .await
      .map_err(|why| refused(format!("agency's server did not answer: {why}")))?;

    if (200..300).contains(&status) {
      return Ok(vec![
        format!("borrow: {} is this job's until it ends — {}'s checkout.", member.cwd, member.name),
        "Work there, commit and push, and close it there the way that repository closes a job.".to_owned(),
      ]);
    }

    let code = answered.as_ref().and_then(|said| said["code"].as_str()).unwrap_or_default();

    if code != "no-worker-free" {
      return Err(refused(format!("agency's server answered {status} for {}: {answered:?}", member.name)));
    }
  }

  let names: Vec<&str> = members.iter().map(|member| member.name.as_str()).collect();

  Err(refused(format!(
    "every checkout of {repository} is at work ({}); read which with bmo run list, and borrow once one is free",
    names.join(", ")
  )))
}
