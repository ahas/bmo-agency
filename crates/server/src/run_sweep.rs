//! The ledger's runs whose turn ended without closing them (docs/run-records.md, 「The run ledger」): a job's run
//! whose job has ended with no turn of it going, and a chat turn's run with no turn kept in flight under its unit,
//! each closed as cut with a line in its log saying why.

use store::all;

use crate::places::Ground;
use crate::run_ledger::{self, Closing, NotClosed};
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::{Refusal, now_ms};

/// How long a chat turn's run stands before a pass may read it as left behind: past the moment between its
/// opening and its turn being kept in flight.
pub const CHAT_GRACE_MS: i64 = 2 * 60 * 1000;

/// The outcome a run left behind is closed with.
pub const CUT: &str = "cut";

/// Why a job's run is closed by the pass.
pub const JOB_ENDED: &str =
  "Closed as cut: its job had ended and no turn of it was going, so nothing else would close this run.";

/// Why a chat turn's run is closed by the pass.
pub const CHAT_LOST: &str =
  "Closed as cut: no turn was going under this run any more, so nothing else would close it.";

/// One pass over the open runs: each left behind closed as cut, its log told why. Answers how many it closed.
pub async fn pass(ground: &Ground) -> Result<usize, Refusal> {
  let now = now_ms();
  let open = all(
    &ground.db,
    r#"SELECT "id", "agent", "unit", "chat_id", "started_at" FROM "runs"
       WHERE "finished_at" IS NULL AND ("unit" LIKE 'job-%' OR "unit" LIKE 'chat:%')
       ORDER BY "id""#,
    vec![],
  )
  .await?;
  let mut closed = 0;

  for row in open {
    let (Ok(id), Ok(agent), Ok(unit)) = (
      row.try_get::<i64>("", "id"),
      row.try_get::<String>("", "agent"),
      row.try_get::<String>("", "unit"),
    ) else {
      continue;
    };
    let chat_id = row.try_get::<Option<i64>>("", "chat_id").ok().flatten();
    let started_at = row.try_get::<i64>("", "started_at").unwrap_or(now);
    let job = unit.strip_prefix("job-").and_then(|id| id.parse::<i64>().ok());
    let why = match job {
      Some(job) if job_left(ground, job, &unit).await? => JOB_ENDED,
      None
        if unit.starts_with("chat:")
          && now - started_at > CHAT_GRACE_MS
          && !crate::turn_pickup::kept(&ground.db, &unit).await =>
      {
        CHAT_LOST
      }
      _ => continue,
    };
    let closing = Closing {
      finished_at: now,
      outcome: CUT.to_owned(),
      context_tokens: None,
      spent: None,
    };

    match run_ledger::close(&ground.db, id, &closing).await {
      Ok(()) => {}
      Err(NotClosed::Store(why)) => return Err(why),
      Err(_) => continue,
    }

    RunLog::new(Target {
      agent,
      run: unit,
      chat_id,
      job_id: job,
    })
    .write(&ground.db, Line::note(why))
    .await;
    closed += 1;
  }

  Ok(closed)
}

// Whether a job's run is left behind: the job has ended, this process carries no turn of it, and no turn of it is
// kept in flight for a server's pickup.
async fn job_left(ground: &Ground, job: i64, unit: &str) -> Result<bool, Refusal> {
  let ended = store::one(&ground.db, r#"SELECT "state" FROM "jobs" WHERE "id" = $1"#, vec![job.into()])
    .await?
    .and_then(|row| row.try_get::<String>("", "state").ok())
    .is_some_and(|state| !matches!(state.as_str(), "starting" | "running"));

  Ok(ended && !crate::control::work::is_carrying(job) && !crate::turn_pickup::kept(&ground.db, unit).await)
}
