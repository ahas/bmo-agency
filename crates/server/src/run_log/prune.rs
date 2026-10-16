//! The run log held to its bounds (docs/run-records.md, 「Agent log」): an agent's newest lines, the text
//! they may hold together, and the age past which no line stays. The writer prunes its own agent's lines as
//! it goes; agency's own clock sweeps the whole table as the server starts and once a day after, which is
//! what reaches an agent whose writer is long gone.

use std::time::Duration;

use sea_orm::DatabaseConnection;
use store::run;

use crate::clock::every;
use crate::{Refusal, now_ms};

/// The run log's bounds: the newest lines an agent keeps, the text they may hold together, and the age
/// past which no line stays.
pub const LOG_LINES_PER_AGENT: i64 = 4000;
pub const LOG_BYTES_PER_AGENT: i64 = 64 * 1024 * 1024;
pub const LOG_MOST_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;

/// How often the whole table is swept.
pub const PRUNE_PASS: Duration = Duration::from_secs(24 * 60 * 60);

// An agent's newest system prompt line is never taken: it is the version that agent's session runs on,
// written again only when that version changes. `{agent}` narrows it to one agent where it is asked of one.
fn kept_prompts(agent: &str) -> String {
  format!(
    r#""id" NOT IN (
      SELECT MAX("id") FROM "agent_log"
      WHERE "job_id" IS NULL AND "kind" = 'prompt' AND "name" = 'system'{agent}
      GROUP BY "agent"
    )"#
  )
}

// The lines past an agent's newest or past its share of text, oldest first. Neither the count nor the text
// reaches a run that has not written its end, since every room's turns share one agent's budget; the age
// bounds such a run. `{agent}` narrows it to one agent, and `{lines}` and `{bytes}` name the bounds.
fn weighed(agent: &str, kept: &str, lines: &str, bytes: &str) -> String {
  format!(
    r#"WITH "open" AS (
         SELECT "agent", "run" FROM "agent_log"
         WHERE "job_id" IS NULL{agent}
         GROUP BY "agent", "run" HAVING SUM(CASE WHEN "kind" = 'end' THEN 1 ELSE 0 END) = 0
       )
       DELETE FROM "agent_log" WHERE {kept} AND "id" IN (
         SELECT "id" FROM (
           SELECT
             "id",
             ROW_NUMBER() OVER (PARTITION BY "agent" ORDER BY "id" DESC) AS "place",
             SUM(length("text") + COALESCE(length("detail"), 0)) OVER (
               PARTITION BY "agent" ORDER BY "id" DESC ROWS UNBOUNDED PRECEDING
             ) AS "held"
           FROM "agent_log" AS "line"
           WHERE "job_id" IS NULL{agent} AND NOT EXISTS (
             SELECT 1 FROM "open" WHERE "open"."agent" = "line"."agent" AND "open"."run" = "line"."run"
           )
         ) AS "weighed" WHERE "place" > {lines} OR "held" > {bytes}
       )"#
  )
}

/// One agent's own run log held to the bounds, as its writer prunes it every so many lines: its lines past
/// its newest or past its share of text, and every line anywhere past the age but that agent's newest
/// system prompt. How many lines went.
pub async fn agent_run_log(db: &DatabaseConnection, agent: &str, now: i64) -> Result<u64, Refusal> {
  let mine = r#" AND "agent" = $1"#;
  let kept = kept_prompts(mine);
  let weighed = run(
    db,
    &weighed(mine, &kept, "$2", "$3"),
    vec![agent.into(), LOG_LINES_PER_AGENT.into(), LOG_BYTES_PER_AGENT.into()],
  )
  .await?;
  let aged = run(
    db,
    &format!(r#"DELETE FROM "agent_log" WHERE "at" < $2 AND "job_id" IS NULL AND {kept}"#),
    vec![agent.into(), (now - LOG_MOST_AGE_SECONDS).into()],
  )
  .await?;

  Ok(weighed + aged)
}

/// The run log held to its bounds over every agent at `now`, in epoch seconds. How many lines went.
pub async fn run_log(db: &DatabaseConnection, now: i64) -> Result<u64, Refusal> {
  let kept = kept_prompts("");
  let weighed = run(
    db,
    &weighed("", &kept, "$1", "$2"),
    vec![LOG_LINES_PER_AGENT.into(), LOG_BYTES_PER_AGENT.into()],
  )
  .await?;
  let aged = run(
    db,
    &format!(r#"DELETE FROM "agent_log" WHERE "at" < $1 AND "job_id" IS NULL AND {kept}"#),
    vec![(now - LOG_MOST_AGE_SECONDS).into()],
  )
  .await?;

  Ok(weighed + aged)
}

/// The whole run log swept on agency's own clock as the server starts and once a day after, for as long as
/// the process stands; what each pass took said on its output.
pub async fn keep(db: DatabaseConnection) {
  every("run log prune", Duration::ZERO, PRUNE_PASS, move || {
    let db = db.clone();

    async move {
      let gone = run_log(&db, now_ms() / 1000).await.map_err(|refusal| refusal.detail)?;

      if gone > 0 {
        println!("run log prune: {gone} line(s) past the log's bounds taken out");
      }

      Ok(())
    }
  })
  .await;
}
