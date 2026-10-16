//! What a turn on a worker's session leaves (docs/cache-fallback.md): the session's record the next job on
//! the worker and model resumes, the heal of a session the account's safeguard answered on a lesser model,
//! and the agent's last answer, in the store; and what the turn spent and the windows it read, published
//! for the readers that keep them (docs/api.md, 「Events」).

use std::sync::LazyLock;

use regex::Regex;
use sea_orm::ConnectionTrait;
use store::{all, one, run};

use super::told::WorkerDowngraded;
use crate::control::claude_accounts::SEED_ACCOUNT;
use crate::door::record::{ModelSpend, Reply, UsageSpent, WindowsRead};
use crate::events::Bus;
use crate::places::Ground;
use crate::{Refusal, now_ms};

/// One session record of a worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerSession {
  pub model: String,
  pub account: String,
  pub session_id: String,
  /// When it last answered, in epoch seconds.
  pub last_reply_at: i64,
  /// The model it answers on where that is not its own.
  pub answered_model: Option<String>,
}

/// Every session record the worker holds, one per model and login.
pub async fn sessions(db: &impl ConnectionTrait, worker: &str) -> Result<Vec<WorkerSession>, Refusal> {
  let rows = all(
    db,
    r#"SELECT "model", "account", "claude_session_id", "last_reply_at", "answered_model" FROM "worker_sessions"
       WHERE "worker" = $1 ORDER BY "last_reply_at""#,
    vec![worker.into()],
  )
  .await?;

  Ok(
    rows
      .iter()
      .map(|row| -> Result<WorkerSession, sea_orm::DbErr> {
        Ok(WorkerSession {
          model: row.try_get("", "model")?,
          account: row.try_get("", "account")?,
          session_id: row.try_get("", "claude_session_id")?,
          last_reply_at: row.try_get("", "last_reply_at")?,
          answered_model: row.try_get("", "answered_model")?,
        })
      })
      .collect::<Result<_, _>>()?,
  )
}

/// The worker's session on a model and login a turn may resume: none where there is none, or where one
/// idle past the life the configuration gives a session is retired.
pub async fn resumable(
  db: &impl ConnectionTrait,
  worker: &str,
  model: &str,
  account: &str,
  ttl_ms: Option<i64>,
) -> Result<Option<String>, Refusal> {
  let row = one(
    db,
    r#"SELECT "claude_session_id", "last_reply_at" FROM "worker_sessions" WHERE "worker" = $1 AND "model" = $2 AND "account" = $3"#,
    vec![worker.into(), model.into(), account.into()],
  )
  .await?;
  let Some(row) = row else { return Ok(None) };
  let last: i64 = row.try_get("", "last_reply_at")?;

  if let Some(ttl) = ttl_ms
    && now_ms() - last * 1000 > ttl
  {
    forget(db, worker, model, Some(account)).await?;

    return Ok(None);
  }

  Ok(Some(row.try_get("", "claude_session_id")?))
}

/// The session's record gone: one login's, or every login's on the model.
pub async fn forget(
  db: &impl ConnectionTrait,
  worker: &str,
  model: &str,
  account: Option<&str>,
) -> Result<(), Refusal> {
  match account {
    None => {
      run(
        db,
        r#"DELETE FROM "worker_sessions" WHERE "worker" = $1 AND "model" = $2"#,
        vec![worker.into(), model.into()],
      )
      .await?
    }
    Some(account) => {
      run(
        db,
        r#"DELETE FROM "worker_sessions" WHERE "worker" = $1 AND "model" = $2 AND "account" = $3"#,
        vec![worker.into(), model.into(), account.into()],
      )
      .await?
    }
  };

  Ok(())
}

/// The session that answered, or the one a fold left, recorded as the one the next job on the worker and
/// model resumes, with the prefix it stands on and the model it fell to, its clock moved to now.
pub async fn record(
  db: &impl ConnectionTrait,
  worker: &str,
  model: &str,
  account: &str,
  session_id: &str,
  prefix_digest: Option<&str>,
  answered_model: Option<&str>,
) -> Result<(), Refusal> {
  run(
    db,
    r#"INSERT INTO "worker_sessions" ("worker", "model", "account", "claude_session_id", "last_reply_at", "prefix_digest", "answered_model")
       VALUES ($1, $2, $3, $4, $5, $6, $7)
       ON CONFLICT ("worker", "model", "account") DO UPDATE SET "claude_session_id" = excluded."claude_session_id",
         "last_reply_at" = excluded."last_reply_at", "prefix_digest" = excluded."prefix_digest", "answered_model" = excluded."answered_model""#,
    vec![
      worker.into(),
      model.into(),
      account.into(),
      session_id.into(),
      (now_ms() / 1000).into(),
      prefix_digest.into(),
      answered_model.into(),
    ],
  )
  .await?;

  Ok(())
}

/// The model a turn on a kept session goes out on: the one it fell to, where the worker's heal is off.
pub async fn launch_model(
  db: &impl ConnectionTrait,
  worker: &str,
  fallen: Option<&str>,
  model: String,
) -> Result<String, Refusal> {
  let Some(fallen) = fallen else { return Ok(model) };

  Ok(if heals(db, worker).await? { model } else { fallen.to_owned() })
}

// Whether a session the safeguard refused is dropped, which the worker's own row says: on unless the
// owner turned it off.
async fn heals(db: &impl ConnectionTrait, worker: &str) -> Result<bool, Refusal> {
  Ok(
    one(db, r#"SELECT "downgrade_heal" FROM "agents" WHERE "name" = $1"#, vec![worker.into()])
      .await?
      .map(|row| row.try_get::<Option<i64>>("", "downgrade_heal"))
      .transpose()?
      .flatten()
      .is_none_or(|heal| heal != 0),
  )
}

static FAMILY: LazyLock<Regex> = LazyLock::new(|| Regex::new("^claude-([a-z]+)").expect("a pattern"));

fn family(model: &str) -> &str {
  FAMILY
    .captures(model)
    .and_then(|found| found.get(1))
    .map_or(model, |found| found.as_str())
}

/// The models a run answered on other than the one it was launched on, within its family: the account's
/// safeguard refusing the launched model on the session's carried context. A launch that named its model
/// in short is read as no downgrade, since which full name the CLI made of it is not known here.
pub fn downgrades(spend: &[ModelSpend], launched: &str) -> Vec<String> {
  if !launched.starts_with("claude-") {
    return vec![];
  }

  spend
    .iter()
    .filter(|one| one.tokens > 0 && one.model != launched && family(&one.model) == family(launched))
    .map(|one| one.model.clone())
    .collect()
}

/// What a downgrade came to: the lesser models, and whether the session's record was dropped.
pub struct Downgrade {
  pub models: Vec<String>,
  pub dropped: bool,
}

/// The heal of a run that came back on a lesser model: the refused session's record dropped, unless the
/// owner turned the heal off for this worker, and `worker-downgraded` published with which worker fell to
/// which model, and on which job where a job's run fell. Telling the owner is the consumer's.
#[allow(clippy::too_many_arguments)]
pub async fn heal(
  ground: &Ground,
  worker: &str,
  model: &str,
  launched: &str,
  account: &str,
  spend: &[ModelSpend],
  job: Option<i64>,
) -> Result<Downgrade, Refusal> {
  let models = downgrades(spend, launched);

  if models.is_empty() {
    return Ok(Downgrade {
      models,
      dropped: false,
    });
  }

  let dropped = heals(&ground.db, worker).await?;

  if dropped {
    forget(&ground.db, worker, model, Some(account)).await?;
  }

  println!(
    "downgrade {worker} {launched}: answered on {}, session {}",
    models.join(", "),
    if dropped { "dropped" } else { "kept, the heal is off for this worker" }
  );

  ground.events.publish(WorkerDowngraded {
    worker: worker.to_owned(),
    launched: launched.to_owned(),
    answered_on: models.clone(),
    dropped,
    job,
  });

  Ok(Downgrade { models, dropped })
}

/// What a kept session is marked with once a turn on it answered: the model it fell to now, or the
/// fallen model it was launched on, which it still answers on.
pub fn fallen_model(model: &str, launched: &str, fell_to: &[String]) -> Option<String> {
  fell_to.first().cloned().or_else(|| (launched != model).then(|| launched.to_owned()))
}

/// The agent's last answer: the provider, model and effort it answered on.
pub async fn note_answer(
  db: &impl ConnectionTrait,
  agent: &str,
  mode: &str,
  model: Option<&str>,
  effort: Option<&str>,
) -> Result<(), Refusal> {
  run(
    db,
    r#"UPDATE "agents" SET "last_mode" = $1, "last_model" = $2, "last_effort" = $3 WHERE "name" = $4"#,
    vec![mode.into(), model.into(), effort.into(), agent.into()],
  )
  .await?;

  Ok(())
}

/// A turn of upkeep or of words on the agent's session counted as its keepalive, failure included.
pub async fn note_keepalive(db: &impl ConnectionTrait, agent: &str) -> Result<(), Refusal> {
  let now = now_ms() / 1000;

  run(
    db,
    r#"UPDATE "agents" SET "last_run_at" = $1, "last_keepalive_at" = $1, "runs" = "runs" + 1 WHERE "name" = $2"#,
    vec![now.into(), agent.into()],
  )
  .await?;

  Ok(())
}

/// What one run spent published under a name, on the login it went out on: every model it spent on with
/// tokens, and nothing for a run that reported nothing.
pub fn note_spend(events: &Bus, agent: &str, spend: &[ModelSpend], account: &str) {
  if let Some(spent) = UsageSpent::of(agent, None, now_ms() / 1000, spend, |_| account.to_owned()) {
    events.publish(spent);
  }
}

/// What a turn's answer says about usage: the agent's last answer written, and each window the answer
/// read and what it spent published; an answer carrying no window publishes none.
pub async fn note_usage(
  db: &impl ConnectionTrait,
  events: &Bus,
  agent: &str,
  reply: &Reply,
) -> Result<(), Refusal> {
  let at = now_ms() / 1000;
  let account = reply.account.clone().unwrap_or_else(|| SEED_ACCOUNT.to_owned());

  note_answer(db, agent, &reply.mode, reply.model.as_deref(), reply.effort.as_deref()).await?;

  if let Some(read) = WindowsRead::of_reply(reply, at) {
    events.publish(read);
  }

  note_spend(events, agent, &reply.spend, &account);

  Ok(())
}

/// A run of the agent counted, as a job's turn counts it on either side of it.
pub async fn note_run(db: &impl ConnectionTrait, agent: &str) -> Result<(), Refusal> {
  run(
    db,
    r#"UPDATE "agents" SET "last_run_at" = $1, "runs" = "runs" + 1 WHERE "name" = $2"#,
    vec![(now_ms() / 1000).into(), agent.into()],
  )
  .await?;

  Ok(())
}

/// A worker whose keepalive the owner turned off until its next job woken by that job.
pub async fn wake_keepalive(db: &impl ConnectionTrait, worker: &str) -> Result<(), Refusal> {
  run(
    db,
    r#"UPDATE "agents" SET "keepalive_paused_at" = NULL WHERE "name" = $1"#,
    vec![worker.into()],
  )
  .await?;

  Ok(())
}

/// The login a provider's reading is filed under: the one that answered, where the provider holds logins
/// of agency's, and the empty account on any other road.
pub fn slot_account(mode: &str, account: Option<&str>) -> String {
  if harness_providers::provider(mode).is_some_and(|provider| provider.holds_logins()) {
    account.filter(|account| !account.is_empty()).unwrap_or(SEED_ACCOUNT).to_owned()
  } else {
    String::new()
  }
}

/// What one run apart reported it spent, published under the agent's own name and the run log it kept,
/// each row on its provider's slot: the login that carried it where the provider holds logins of agency's.
pub fn note_run_spend(
  events: &Bus,
  agent: &str,
  spend: &[ModelSpend],
  run_log: Option<&str>,
  account: Option<&str>,
) {
  if let Some(spent) =
    UsageSpent::of(agent, run_log, now_ms() / 1000, spend, |one| slot_account(&one.mode, account))
  {
    events.publish(spent);
  }
}

/// What a job's turn says about usage under the job's run: the agent's last answer written, and each
/// window the answer read and what it spent published, each on its provider's slot; an answer carrying no
/// window publishes none.
pub async fn note_job_usage(
  db: &impl ConnectionTrait,
  events: &Bus,
  agent: &str,
  reply: &Reply,
  logged_as: Option<&str>,
) -> Result<(), Refusal> {
  let at = now_ms() / 1000;
  let account = reply.account.as_deref();

  note_answer(db, agent, &reply.mode, reply.model.as_deref(), reply.effort.as_deref()).await?;

  if let Some(read) = WindowsRead::of_reply(reply, at) {
    events.publish(WindowsRead {
      account: Some(slot_account(&reply.mode, account)),
      ..read
    });
  }

  // A row its program named no provider for is the turn's own provider's.
  let spend: Vec<ModelSpend> = reply
    .spend
    .iter()
    .map(|one| ModelSpend {
      mode: if one.mode.is_empty() { reply.mode.clone() } else { one.mode.clone() },
      ..one.clone()
    })
    .collect();

  if let Some(spent) = UsageSpent::of(agent, logged_as, at, &spend, |one| slot_account(&one.mode, account))
  {
    events.publish(spent);
  }

  Ok(())
}
