//! What a word said to a worker is in the store and on the worker's screen: its row, waiting until a turn has
//! taken it, and the run of the worker's own log it stands in with whatever stood under it.

use policy::deployment::DEPLOYMENT;
use sea_orm::DatabaseConnection;
use store::{all, one, run};

use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::{Refusal, now_ms};

/// The name a word's own line carries in the worker's log, which is how a screen tells the owner's words
/// from the prompt a run was launched with.
pub const LINE_NAME: &str = "word";

/// One word as it waits for a worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
  pub id: i64,
  pub text: String,
  /// When it was said, in epoch seconds.
  pub added_at: i64,
}

/// The run a word and the answer to it are kept under in the worker's own log: named for the word, which is
/// written the moment it is said, rather than for the turn that answers it later.
pub fn run_of(worker: &str, word: i64) -> String {
  format!("{}-word-{worker}-{word}", DEPLOYMENT.agent_key)
}

/// A word written down to be handed over, with its line in the worker's log at once, so it stands on the
/// worker's screen whether a session has been handed it yet or not. Its id.
pub async fn say(db: &DatabaseConnection, worker: &str, text: &str) -> Result<i64, Refusal> {
  let id: i64 = one(
    db,
    r#"INSERT INTO "worker_words" ("worker", "text", "added_at") VALUES ($1, $2, $3) RETURNING "id""#,
    vec![worker.into(), text.into(), (now_ms() / 1000).into()],
  )
  .await?
  .ok_or_else(|| Refusal::failed("the word was not written down"))?
  .try_get("", "id")?;

  write(db, worker, id, "prompt", Some(LINE_NAME), text).await?;

  Ok(id)
}

/// The words no turn has taken yet, oldest first.
pub async fn waiting(db: &DatabaseConnection, worker: &str) -> Result<Vec<Word>, Refusal> {
  let rows = all(
    db,
    r#"SELECT "id", "text", "added_at" FROM "worker_words"
       WHERE "worker" = $1 AND "delivered_at" IS NULL ORDER BY "id""#,
    vec![worker.into()],
  )
  .await?;

  Ok(
    rows
      .iter()
      .map(|row| -> Result<Word, sea_orm::DbErr> {
        Ok(Word {
          id: row.try_get("", "id")?,
          text: row.try_get("", "text")?,
          added_at: row.try_get("", "added_at")?,
        })
      })
      .collect::<Result<_, _>>()?,
  )
}

/// The words marked as taken, so none is said twice.
pub async fn delivered(db: &DatabaseConnection, words: &[Word]) -> Result<(), Refusal> {
  let now = now_ms() / 1000;

  for word in words {
    run(
      db,
      r#"UPDATE "worker_words" SET "delivered_at" = $1 WHERE "id" = $2 AND "delivered_at" IS NULL"#,
      vec![now.into(), word.id.into()],
    )
    .await?;
  }

  Ok(())
}

/// One line of the word's run in the worker's log, kept whole: what the screen shows was said, and what was
/// said back.
pub async fn write(
  db: &DatabaseConnection,
  worker: &str,
  word: i64,
  kind: &'static str,
  name: Option<&str>,
  text: &str,
) -> Result<(), Refusal> {
  let mut log = RunLog::new(Target {
    agent: worker.to_owned(),
    run: run_of(worker, word),
    chat_id: None,
    job_id: None,
  })
  .keeping_launch();
  let line = Line {
    kind,
    text: text.to_owned(),
    name: name.map(str::to_owned),
    ..Line::default()
  };

  match log.write(db, line).await {
    Some(_) => Ok(()),
    None => Err(Refusal::failed(format!("a line of {worker}'s word was not written"))),
  }
}
