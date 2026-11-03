//! A job's lease kept on the harness's behalf (docs/api.md, 「assignments and jobs」): every job still
//! starting or running has its item's lease renewed in the job service, and a lease the service no longer holds
//! taken again on the same item for the same holder, and its assignee where it names one, the job keeping
//! the new lease's id. An item that moved under a running job — held, dropped, merged or leased by another
//! holder — is left as it stands and told once to its orderer. An ended job whose lease stands for the item's
//! close has it renewed the same way until the service holds it no more — the close moved the item to done, or
//! the lease was given back — and is never leased again; an item still leased at the pass after the one that
//! first found its job ended is told once to its orderer (docs/agent-control.md, 「An item's change is told to
//! its orderer」). A deployment that names no job service keeps nothing.

use std::collections::{BTreeMap, BTreeSet};

use policy::job_leases::JOB_LEASES;
use reqwest::StatusCode;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use store::{all, run};

use crate::Refusal;
use crate::job_end::{address, token};
use crate::job_notices::{self, Receiver};
use crate::queue::orderer::JobService;
use crate::queue::settings;

/// The settings rows, one a running job by its id, of the state its item was told to have moved to.
pub const MOVED_TOLD_PREFIX: &str = "job_moved_told:";
/// The settings rows, one an ended job by its id, of its item's close: `seen` once a pass found the lease
/// still standing, `told` once the orderer was told.
pub const CLOSE_PREFIX: &str = "job_close_mark:";

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
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
) -> Result<Vec<(i64, Kept)>, Refusal> {
  let (Some(url), Some(token)) =
    (address(vars, "BMO_JOB_URL"), token(vars, "BMO_JOB_TOKEN", "BMO_JOB_TOKEN_FILE"))
  else {
    return Ok(vec![]);
  };
  let rows = all(
    db,
    r#"SELECT "id", "item_id", "lease_id", "holder_name", "worker_name", "state" FROM "jobs"
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
  let tells = Tells {
    db,
    service: JobService {
      url: url.clone(),
      token: token.clone(),
    },
    receiver: Receiver::of(vars),
  };
  let mut kept = vec![];
  let mut running = BTreeSet::new();
  let mut ended = BTreeSet::new();

  for row in rows {
    let (Ok(id), Ok(item), Ok(lease), Ok(holder), Ok(worker), Ok(state)) = (
      row.try_get::<i64>("", "id"),
      row.try_get::<i64>("", "item_id"),
      row.try_get::<i64>("", "lease_id"),
      row.try_get::<String>("", "holder_name"),
      row.try_get::<String>("", "worker_name"),
      row.try_get::<String>("", "state"),
    ) else {
      continue;
    };

    if item <= 0 {
      continue;
    }

    if state == "ended" {
      let done = match service.renew(lease).await {
        Ok(true) => {
          tells.unclosed(id, item, lease).await;
          Kept::Renewed
        }
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

      // A lease that ended lets its close mark go with this pass.
      if done != Kept::Closed {
        ended.insert(id);
      }

      kept.push((id, done));
      continue;
    }

    running.insert(id);

    let done = match service.renew(lease).await {
      Ok(true) => Kept::Renewed,
      Ok(false) => match service.take(item, &holder).await {
        Ok(Took::Leased(taken)) => {
          run(
            db,
            r#"UPDATE "jobs" SET "lease_id" = $1 WHERE "id" = $2"#,
            vec![taken.into(), id.into()],
          )
          .await?;
          println!("job {id}: the lease on item #{item} had run out, taken again as lease {taken}");
          Kept::Taken(taken)
        }
        Ok(Took::Moved(held)) => {
          let why = format!("the item is {}, so it is left as it stands", job_notices::moved_state(&held));

          eprintln!("job {id}: the lease on item #{item} had run out and was not taken again: {why}");
          tells.moved(id, &worker, &held).await;
          Kept::Left(why)
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

  tells.let_go(&running, &ended).await;

  Ok(kept)
}

// What became of a lease asked again: taken, under its id, or not, the item having moved, as it stands.
enum Took {
  Leased(i64),
  Moved(Value),
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

  // The item leased again for the job's holder and worker, where it is open: the new lease's id; the item as
  // it stands where it is no longer open, or another holder took it first.
  async fn take(&self, item: i64, holder: &str) -> Result<Took, String> {
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

    if held["state"].as_str() != Some("open") {
      return Ok(Took::Moved(held));
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
      Some(lease) if status.is_success() => Ok(Took::Leased(lease)),
      // Another holder took the item first: it is leased, but not to this job.
      _ if status == StatusCode::CONFLICT => {
        let mut moved = held;

        moved["state"] = json!("leased");
        Ok(Took::Moved(moved))
      }
      _ => Err(format!("the job service answered {status} to the lease: {}", taken["error"])),
    }
  }
}

// What the pass tells an item's orderer, and the marks it keeps so each is told once.
struct Tells<'a> {
  db: &'a DatabaseConnection,
  service: JobService,
  receiver: Option<Receiver>,
}

impl Tells<'_> {
  async fn mark(&self, key: &str) -> Option<String> {
    settings::get(self.db, key).await.ok().flatten()
  }

  async fn set(&self, key: &str, value: &str) {
    if let Err(why) = settings::set(self.db, key, value).await {
      eprintln!("jobs' leases: the mark {key} was not kept: {}", why.detail);
    }
  }

  // An item that moved under a running job, told to its orderer once for that job and that state, and for a
  // merge to the carrying item's orderer too where it answers another room.
  async fn moved(&self, job: i64, worker: &str, item: &Value) {
    let Some(receiver) = &self.receiver else { return };
    let key = format!("{MOVED_TOLD_PREFIX}{job}");
    let state = job_notices::moved_state(item);

    if self.mark(&key).await.as_deref() == Some(state.as_str()) {
      return;
    }

    let told = async {
      let orderer = self.service.orderer_of(item).await?;

      receiver.instruct(&job_notices::moved(item, job, worker, &orderer)).await?;

      if let Some(into) = item["mergedInto"].as_i64() {
        let (_, carrying) = self.service.orderer_of_id(into).await?;

        if carrying.room != orderer.room {
          receiver.instruct(&job_notices::moved(item, job, worker, &carrying)).await?;
        }
      }

      Ok::<_, String>(())
    };

    match told.await {
      Ok(()) => {
        self.set(&key, &state).await;
        println!("job {job}: its item moved to {state} under it, told to its orderer");
      }
      Err(why) => eprintln!("job {job}: its item's move was not told: {why}"),
    }
  }

  // An ended job's item still leased: marked at the first pass that finds it so, told to its orderer once at
  // the next.
  async fn unclosed(&self, job: i64, item: i64, lease: i64) {
    let Some(receiver) = &self.receiver else { return };
    let key = format!("{CLOSE_PREFIX}{job}");

    match self.mark(&key).await.as_deref() {
      None => self.set(&key, "seen").await,
      Some("seen") => {
        let told = async {
          let (_, orderer) = self.service.orderer_of_id(item).await?;

          receiver.instruct(&job_notices::awaits_close(item, lease, &orderer)).await
        };

        match told.await {
          Ok(_) => {
            self.set(&key, "told").await;
            println!("job {job}: item #{item} still leased after its end, told to its orderer");
          }
          Err(why) => eprintln!("job {job}: item #{item} awaiting its close was not told: {why}"),
        }
      }
      Some(_) => {}
    }
  }

  // The marks of jobs that moved on let go: a moved item's once its job no longer runs, a close's once its job's
  // lease is no longer kept.
  async fn let_go(&self, running: &BTreeSet<i64>, ended: &BTreeSet<i64>) {
    for (prefix, standing) in [(MOVED_TOLD_PREFIX, running), (CLOSE_PREFIX, ended)] {
      let Ok(marked) = settings::numbered(self.db, prefix).await else { continue };

      for job in marked.into_iter().filter(|job| !standing.contains(job)) {
        if let Err(why) = settings::forget(self.db, &format!("{prefix}{job}")).await {
          eprintln!("jobs' leases: the mark {prefix}{job} was not let go: {}", why.detail);
        }
      }
    }
  }
}
