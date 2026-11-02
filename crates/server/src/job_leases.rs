//! A job's lease kept on the harness's behalf (docs/api.md, 「assignments and jobs」): every job still
//! starting or running has its item's lease renewed in the job service, and a lease the service no longer holds
//! taken again on the same item for the same holder, and its assignee where it names one, the job keeping
//! the new lease's id. An ended job whose lease stands for the item's close has it renewed the same way until
//! the service holds it no more — the close moved the item to done, or the lease was given back — and is
//! never leased again. A deployment that names no job service keeps nothing.

use std::collections::BTreeMap;

use policy::job_leases::JOB_LEASES;
use reqwest::StatusCode;
use serde_json::{Value, json};
use store::{all, run};

use crate::Refusal;
use crate::job_end::{address, token};

/// What a pass did with one running job's lease.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kept {
  /// The lease stood and was renewed.
  Renewed,
  /// The lease had run out and the item was leased again, under this id.
  Taken(i64),
  /// An ended job's lease the service holds no more: the item's close ended it, or it was given back.
  Closed,
  /// Neither: the item is another holder's or no longer open, or the job service could not be asked; why.
  Left(String),
}

/// One pass over the running jobs, each one's lease renewed or taken again, and the ended jobs whose lease
/// stands for the item's close, each one's renewed or let go once the service holds it no more. Answers each
/// job with what was done; nothing where the deployment names no job service.
pub async fn pass(
  db: &sea_orm::DatabaseConnection,
  vars: &BTreeMap<String, String>,
) -> Result<Vec<(i64, Kept)>, Refusal> {
  let (Some(url), Some(token)) =
    (address(vars, "BMO_JOB_URL"), token(vars, "BMO_JOB_TOKEN", "BMO_JOB_TOKEN_FILE"))
  else {
    return Ok(vec![]);
  };
  let rows = all(
    db,
    r#"SELECT "id", "item_id", "lease_id", "holder_name", "state" FROM "jobs"
       WHERE "state" IN ('starting', 'running') OR ("state" = 'ended' AND "lease_kept") ORDER BY "id""#,
    vec![],
  )
  .await?;
  let client = reqwest::Client::new();
  let service = Service {
    url: &url,
    token: &token,
    client: &client,
  };
  let mut kept = vec![];

  for row in rows {
    let (Ok(id), Ok(item), Ok(lease), Ok(holder), Ok(state)) = (
      row.try_get::<i64>("", "id"),
      row.try_get::<i64>("", "item_id"),
      row.try_get::<i64>("", "lease_id"),
      row.try_get::<String>("", "holder_name"),
      row.try_get::<String>("", "state"),
    ) else {
      continue;
    };

    if item <= 0 {
      continue;
    }

    if state == "ended" {
      let done = match service.renew(lease).await {
        Ok(true) => Kept::Renewed,
        Ok(false) => {
          run(db, r#"UPDATE "jobs" SET "lease_kept" = false WHERE "id" = $1"#, vec![id.into()]).await?;
          println!("job {id}: the lease {lease} on item #{item} has ended with the item's close");
          Kept::Closed
        }
        Err(why) => {
          eprintln!(
            "job {id}: the lease {lease} on item #{item}, kept for its close, was not renewed: {why}"
          );
          Kept::Left(why)
        }
      };

      kept.push((id, done));
      continue;
    }

    let done = match service.renew(lease).await {
      Ok(true) => Kept::Renewed,
      Ok(false) => match service.take(item, &holder).await {
        Ok(taken) => {
          run(
            db,
            r#"UPDATE "jobs" SET "lease_id" = $1 WHERE "id" = $2"#,
            vec![taken.into(), id.into()],
          )
          .await?;
          println!("job {id}: the lease on item #{item} had run out, taken again as lease {taken}");
          Kept::Taken(taken)
        }
        Err(why) => {
          eprintln!("job {id}: the lease on item #{item} had run out and was not taken again: {why}");
          Kept::Left(why)
        }
      },
      Err(why) => {
        eprintln!("job {id}: the lease {lease} on item #{item} was not renewed: {why}");
        Kept::Left(why)
      }
    };

    kept.push((id, done));
  }

  Ok(kept)
}

// The job service as the pass reaches it.
struct Service<'a> {
  url: &'a str,
  token: &'a str,
  client: &'a reqwest::Client,
}

impl Service<'_> {
  // Whether the lease stood and was renewed; false where the service holds it no more.
  async fn renew(&self, lease: i64) -> Result<bool, String> {
    if lease <= 0 {
      return Ok(false);
    }

    let answer = self
      .client
      .patch(format!("{}/leases/{lease}", self.url))
      .bearer_auth(self.token)
      .json(&json!({ "ttlMs": JOB_LEASES.ttl_ms }))
      .send()
      .await
      .map_err(|why| why.to_string())?;

    match answer.status() {
      status if status.is_success() => Ok(true),
      StatusCode::GONE | StatusCode::NOT_FOUND => Ok(false),
      status => Err(format!("the job service answered {status}")),
    }
  }

  // The item leased again for the job's holder and worker, where it is open: the new lease's id.
  async fn take(&self, item: i64, holder: &str) -> Result<i64, String> {
    let read = self
      .client
      .get(format!("{}/items/{item}", self.url))
      .bearer_auth(self.token)
      .send()
      .await
      .map_err(|why| why.to_string())?;

    if !read.status().is_success() {
      return Err(format!("the item could not be read: {}", read.status()));
    }

    let held: Value = read.json().await.map_err(|why| why.to_string())?;
    let state = held["state"].as_str().unwrap_or_default();

    if state != "open" {
      return Err(format!("the item is {state}, so it is left as it stands"));
    }

    let mut asked = json!({
      "queueName": held["queueName"],
      "itemId": item,
      "holderName": holder,
      "ttlMs": JOB_LEASES.ttl_ms,
    });

    // An item with an assignee is taken only by a lease naming that assignee.
    if held["assigneeName"].as_str().is_some_and(|assignee| !assignee.is_empty()) {
      asked["assigneeName"] = held["assigneeName"].clone();
    }

    let answer = self
      .client
      .post(format!("{}/leases", self.url))
      .bearer_auth(self.token)
      .json(&asked)
      .send()
      .await
      .map_err(|why| why.to_string())?;
    let status = answer.status();
    let taken: Value = answer.json().await.unwrap_or(Value::Null);

    match taken["id"].as_i64() {
      Some(lease) if status.is_success() => Ok(lease),
      _ => Err(format!("the job service answered {status} to the lease: {}", taken["error"])),
    }
  }
}
