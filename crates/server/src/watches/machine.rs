//! This machine's room for the launcher (docs/agent-control.md, 「Room on the machine」): the node service
//! reads the machine and judges its own limits, and agency counts the jobs at work, judges their limits,
//! keeps the launcher's notice of a held launch and says when the room has returned.

use agent_session::JobMark;
use policy::fill_marks;
use policy::machine_room::{CountLimit, MACHINE_ROOM};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use store::all;

use crate::Refusal;
use crate::events::{Bus, Event};
use crate::peers::node::{NodeService, Standing};
use crate::queue::settings;

const LIMIT_PREFIX: &str = "machine_limit:";
const LAUNCH_HELD: &str = "machine_launch_held";
const WORKER_JOB: &str = "worker_job:";

/// A worker at work on this machine, which is also the name of its place, with the job its turn named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtWork {
  pub worker: String,
  pub job: Option<JobMark>,
}

/// This machine's room as the launcher reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Room {
  pub held: Vec<String>,
  pub jobs: u64,
  pub browser_jobs: u64,
  pub browsers: u64,
}

/// The launcher's notice of launches held: when, and on what.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchHeld {
  pub at: i64,
  pub held: Vec<String>,
}

/// The room returned under a held launch's notice, which is let go (docs/api.md, 「Events」).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineRoomReturned {
  pub at: i64,
  pub held: Vec<String>,
}

impl Event for MachineRoomReturned {
  const NAME: &'static str = "machine-room-returned";
}

#[derive(Serialize, Deserialize)]
struct JobKept {
  job: i64,
  item: Option<i64>,
}

/// The job a worker's turn named kept beside the worker while the turn goes, or let go where none is
/// named. A store that cannot keep it is said on the error stream; the turn goes on all the same.
pub async fn keep_worker_job(db: &DatabaseConnection, worker: &str, job: Option<JobMark>) {
  let key = format!("{WORKER_JOB}{worker}");
  let kept = match job {
    Some(job) => {
      let value = serde_json::json!(JobKept {
        job: job.job,
        item: job.item
      });

      settings::set(db, &key, &value.to_string()).await
    }
    None => settings::forget(db, &key).await,
  };

  if let Err(refusal) = kept {
    eprintln!("agency: the job of {worker}'s turn could not be kept: {}", refusal.detail);
  }
}

/// Every worker standing on this machine, its row naming no work node, with a run open in the executions,
/// by name, each with the job its turn named.
pub async fn at_work(db: &DatabaseConnection) -> Result<Vec<AtWork>, Refusal> {
  let rows = all(
    db,
    r#"SELECT a."name", s."value" AS "job" FROM "agents" a
       LEFT JOIN "settings" s ON s."key" = $1 || a."name"
       WHERE a."kind" = 'worker' AND a."node" IS NULL
         AND EXISTS (SELECT 1 FROM "agent_executions" e
                     WHERE (e."body"::json ->> 'endedAt') IS NULL AND (e."body"::json ->> 'agent') = a."name")
       ORDER BY a."name""#,
    vec![WORKER_JOB.into()],
  )
  .await?;

  rows
    .iter()
    .map(|row| {
      let job = row
        .try_get::<Option<String>>("", "job")?
        .and_then(|job| serde_json::from_str::<JobKept>(&job).ok())
        .map(|kept| JobMark {
          job: kept.job,
          item: kept.item,
        });

      Ok(AtWork {
        worker: row.try_get("", "name")?,
        job,
      })
    })
    .collect()
}

// A limit as the settings table holds it, its default where the row is missing or no positive number.
async fn limit(db: &DatabaseConnection, name: &str, of: &CountLimit) -> Result<u64, Refusal> {
  let held = settings::get(db, &format!("{LIMIT_PREFIX}{name}")).await?;

  Ok(
    held
      .and_then(|value| value.trim().parse::<u64>().ok())
      .filter(|value| *value > 0)
      .unwrap_or(of.default),
  )
}

/// The jobs at work, and of them those whose place runs a browser in the standing.
pub fn counted(at_work: &[AtWork], standing: &Standing) -> (u64, u64) {
  let browsing = at_work
    .iter()
    .filter(|worker| standing.places.iter().any(|place| place.place == worker.worker && place.browsers > 0))
    .count();

  (at_work.len() as u64, browsing as u64)
}

/// This machine's room: what the node service holds, then agency's own job-count limits met. A node
/// service that cannot be asked holds nothing of its part, and a standing that cannot be read counts no
/// browser.
pub async fn room(db: &DatabaseConnection, nodes: &dyn NodeService) -> Result<Room, Refusal> {
  let (asked, standing) = tokio::join!(nodes.machine_room(), nodes.machine_standing());
  let mut held = asked.map(|room| room.held).unwrap_or_else(|fault| {
    eprintln!("agency: the machine's room: {fault}");
    vec![]
  });
  let standing = standing.unwrap_or_else(|fault| {
    eprintln!("agency: the machine's standing: {fault}");
    Standing::default()
  });
  let (jobs, browser_jobs) = counted(&at_work(db).await?, &standing);

  for (name, of, count) in [
    ("launch_jobs", &MACHINE_ROOM.launch_jobs, jobs),
    ("launch_browser_jobs", &MACHINE_ROOM.launch_browser_jobs, browser_jobs),
  ] {
    let limit = limit(db, name, of).await?;

    if count >= limit {
      held.push(fill_marks(&of.held, &[("jobs", &count.to_string()), ("limit", &limit.to_string())]));
    }
  }

  Ok(Room {
    held,
    jobs,
    browser_jobs,
    browsers: standing.browsers,
  })
}

/// The launcher's notice kept, or withdrawn where it holds nothing.
pub async fn tell_launch_held(db: &DatabaseConnection, notice: &LaunchHeld) -> Result<(), Refusal> {
  if notice.held.is_empty() {
    return settings::forget(db, LAUNCH_HELD).await;
  }

  settings::set(db, LAUNCH_HELD, &serde_json::json!(notice).to_string()).await
}

/// The launcher's notice of launches held, where one stands.
pub async fn launch_held(db: &DatabaseConnection) -> Result<Option<LaunchHeld>, Refusal> {
  Ok(
    settings::get(db, LAUNCH_HELD)
      .await?
      .and_then(|value| serde_json::from_str(&value).ok()),
  )
}

/// One pass of the held launch's watch: where a notice stands and nothing holds a launch any more, the
/// notice is let go and the room's return published. A notice the launcher replaced meanwhile stands.
/// Answers whether the room returned.
pub async fn watch_held(
  db: &DatabaseConnection,
  nodes: &dyn NodeService,
  events: &Bus,
) -> Result<bool, Refusal> {
  let Some(kept) = settings::get(db, LAUNCH_HELD).await? else {
    return Ok(false);
  };

  if !room(db, nodes).await?.held.is_empty() {
    return Ok(false);
  }

  let gone = store::run(
    db,
    r#"DELETE FROM "settings" WHERE "key" = $1 AND "value" = $2"#,
    vec![LAUNCH_HELD.into(), kept.clone().into()],
  )
  .await?;

  if gone == 0 {
    return Ok(false);
  }

  if let Ok(notice) = serde_json::from_str::<LaunchHeld>(&kept) {
    events.publish(MachineRoomReturned {
      at: notice.at,
      held: notice.held,
    });
  }

  Ok(true)
}
