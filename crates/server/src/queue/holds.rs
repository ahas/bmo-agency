//! What holds a checkout now (docs/api.md, 「assignments and jobs」): an assignment `held` or `running`, or a
//! job at work in it. `POST /assignments`, a job's borrowing and the queue's notices read a checkout as free by
//! this one reading, and the assignments whose hold has ended are released before any of them reads it.

use sea_orm::DatabaseConnection;
use store::{all, one, run};

use crate::Refusal;
use crate::now_ms;

/// What opens the holder's name of an assignment a job took for itself, the job's number after it: a checkout it
/// borrowed from another repository.
pub const JOB_HOLDER: &str = "job:";

// The checkouts held now, one row a hold: every assignment held or running, and every job at work.
const HOLDING: &str = r#"SELECT "worker_name" FROM "assignments" WHERE "state" IN ('held', 'running')
  UNION ALL SELECT "worker_name" FROM "jobs"
   WHERE "state" IN ('starting', 'running') OR ("state" = 'reviewing' AND NOT "review_parked")"#;

/// The assignments whose hold has ended, released: a running one whose job has ended, one a job borrowed that no
/// longer runs, and a held one a newer assignment of the same item follows, since an item is held on one checkout
/// at a time. No clock lets any other hold go.
pub async fn release_ended(db: &DatabaseConnection) -> Result<(), Refusal> {
  let now = now_ms();
  run(
    db,
    r#"UPDATE "assignments" a SET "state" = 'released', "changed_at" = $1
       WHERE a."state" = 'running'
         AND EXISTS (SELECT 1 FROM "jobs" j WHERE j."assignment_id" = a."id")
         AND NOT EXISTS (SELECT 1 FROM "jobs" j
               WHERE j."assignment_id" = a."id"
                 AND (j."state" IN ('starting', 'running') OR (j."state" = 'reviewing' AND NOT j."review_parked")))"#,
    vec![now.into()],
  )
  .await?;
  // A checkout a job borrowed is lent for as long as that job runs (docs/worker-slots.md, 「A job writes only in
  // its own checkout」).
  run(
    db,
    r#"UPDATE "assignments" a SET "state" = 'released', "changed_at" = $1
       WHERE a."state" = 'held'
         AND a."holder_name" LIKE $2
         AND NOT EXISTS (SELECT 1 FROM "jobs" j
               WHERE $3 || j."id"::text = a."holder_name"
                 AND (j."state" IN ('starting', 'running') OR (j."state" = 'reviewing' AND NOT j."review_parked")))"#,
    vec![now.into(), format!("{JOB_HOLDER}%").into(), JOB_HOLDER.into()],
  )
  .await?;
  // A hold its item was assigned again after, under whichever holder, is one no job will start on: a hold left
  // under a name no session ends with goes here. A job's loan is no assignment of its item on either side.
  run(
    db,
    r#"UPDATE "assignments" a SET "state" = 'released', "changed_at" = $1
       WHERE a."state" = 'held'
         AND a."holder_name" NOT LIKE $2
         AND EXISTS (SELECT 1 FROM "assignments" b
               WHERE b."item_id" = a."item_id" AND (b."created_at", b."id") > (a."created_at", a."id")
                 AND b."holder_name" NOT LIKE $2)"#,
    vec![now.into(), format!("{JOB_HOLDER}%").into()],
  )
  .await?;

  Ok(())
}

/// The held assignments of an item that holders other than `holder` left, released as `holder` asks for the
/// item, since an item is held on one checkout at a time. A job's loan is no assignment of its item.
pub async fn release_others(db: &DatabaseConnection, item_id: i64, holder: &str) -> Result<(), Refusal> {
  run(
    db,
    r#"UPDATE "assignments" SET "state" = 'released', "changed_at" = $1
       WHERE "state" = 'held' AND "item_id" = $2 AND "holder_name" <> $3 AND "holder_name" NOT LIKE $4"#,
    vec![now_ms().into(), item_id.into(), holder.into(), format!("{JOB_HOLDER}%").into()],
  )
  .await?;

  Ok(())
}

/// Whether nothing holds a checkout now, read in whatever connection a caller holds.
pub async fn is_free<C: sea_orm::ConnectionTrait>(db: &C, checkout: &str) -> Result<bool, Refusal> {
  let held = one(
    db,
    &format!(r#"SELECT 1 AS "held" FROM ({HOLDING}) h WHERE h."worker_name" = $1 LIMIT 1"#),
    vec![checkout.into()],
  )
  .await?;

  Ok(held.is_none())
}

/// The checkouts something holds now, by name.
pub async fn held_checkouts(db: &DatabaseConnection) -> Result<Vec<String>, Refusal> {
  Ok(
    all(db, &format!(r#"SELECT DISTINCT "worker_name" FROM ({HOLDING}) h"#), vec![])
      .await?
      .iter()
      .filter_map(|row| row.try_get::<String>("", "worker_name").ok())
      .collect(),
  )
}
