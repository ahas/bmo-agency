//! The sessions roles keep (docs/sessions.md, 「A role's session」): one row a session a role's run opened,
//! kept by the role, the path, the provider and the model together and the command's key beside them, taken by
//! a run as it starts so no two runs share it, and written again with the session that answered once the run
//! has answered. A session past its cache lifetime, or whose last turn filled the model's window, is dropped
//! as it is taken, and the run opens a new one.

use std::path::Path;

use policy::roles::Sessions;
use sea_orm::{ConnectionTrait, QueryResult};
use sha2::{Digest, Sha256};
use store::{Fault, one, run};

use crate::agent_roles;
use crate::lines::claude::Command;
use crate::worker_session::warmth::{set_aside_lifetime_ms, warm};

// The flag a command names the session it opens with, and the one its system prompt's file is handed by.
const OPENS: &str = "--session-id";
const SYSTEM_PROMPT_FILE: &str = "--system-prompt-file";

/// The sessions setting a role's run opens its session by; a role the store does not hold keeps nothing.
pub async fn setting<C: ConnectionTrait>(db: &C, role: &str) -> Sessions {
  match agent_roles::sessions_of(db, role).await {
    Ok(sessions) => sessions.unwrap_or(Sessions::Fresh),
    Err(why) => {
      eprintln!(
        "{role}: the role's sessions setting could not be read, so its run opens a new one: {why:?}"
      );

      Sessions::Fresh
    }
  }
}

/// The key a command stands under beside its role, path, provider and model: the line, the login's
/// configuration home and every word of the command but its session's, the system prompt's text standing for
/// its file, so another effort, login, tool list or prompt is another command.
pub fn key(line: &str, command: &Command, home: &Path) -> String {
  let mut hash = Sha256::new();
  let mut part = |text: &[u8]| {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text);
  };

  part(line.as_bytes());
  part(command.cwd.as_bytes());
  part(home.as_os_str().as_encoded_bytes());

  let mut words = command.args.iter();

  while let Some(word) = words.next() {
    if word == OPENS {
      words.next();
      continue;
    }

    part(word.as_bytes());

    if word == SYSTEM_PROMPT_FILE
      && let Some(file) = words.next()
    {
      part(&std::fs::read(file).unwrap_or_else(|_| file.as_bytes().to_vec()));
    }
  }

  format!("{:x}", hash.finalize())
}

/// What a role's session is kept by: the role, the path its run works at, the provider and model it went out
/// on, the effort and login, and the command's key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionKey {
  pub role: String,
  pub path: String,
  pub provider: String,
  pub model: String,
  pub effort: String,
  pub account: Option<String>,
  pub command: String,
}

/// One kept session as its row holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
  pub id: i64,
  pub role: String,
  pub path: String,
  pub provider: String,
  pub model: String,
  pub effort: String,
  pub account: Option<String>,
  pub session: String,
  pub opened_at: i64,
  pub answered_at: Option<i64>,
  pub runs: i64,
  pub context_tokens: Option<i64>,
  pub context_share: Option<f64>,
  pub held: bool,
  pub dropped: bool,
}

impl Kept {
  pub fn of(row: &QueryResult) -> Result<Self, Fault> {
    Ok(Self {
      id: row.try_get("", "id")?,
      role: row.try_get("", "role")?,
      path: row.try_get("", "path")?,
      provider: row.try_get("", "provider")?,
      model: row.try_get("", "model")?,
      effort: row.try_get("", "effort")?,
      account: row.try_get("", "account")?,
      session: row.try_get("", "session")?,
      opened_at: row.try_get("", "opened_at")?,
      answered_at: row.try_get("", "answered_at")?,
      runs: row.try_get("", "runs")?,
      context_tokens: row.try_get("", "context_tokens")?,
      context_share: row.try_get("", "context_share")?,
      held: row.try_get::<i64>("", "held")? != 0,
      dropped: row.try_get::<i64>("", "dropped")? != 0,
    })
  }
}

/// Whether the next run of a kept session's key would resume it at `now`: not dropped, held by no run, its
/// cache still warm for its provider's model and its last turn under the model's window.
pub fn valid(kept: &Kept, now: i64) -> bool {
  !kept.dropped
    && !kept.held
    && kept.context_share.is_none_or(|share| share < 1.0)
    && kept
      .answered_at
      .is_some_and(|answered| warm(answered, now, set_aside_lifetime_ms(&kept.provider, &kept.model)))
}

/// The session a key's last answered run left, taken so no other run resumes it meanwhile, where it is still
/// valid at `now`; one no longer valid is dropped all the same, and the run opens a new one.
pub async fn take<C: ConnectionTrait>(db: &C, key: &SessionKey, now: i64) -> Result<Option<String>, Fault> {
  let Some(row) = one(
    db,
    r#"SELECT * FROM "role_sessions"
       WHERE "role" = $1 AND "path" = $2 AND "provider" = $3 AND "model" = $4 AND "command_key" = $5
         AND "dropped" = 0 AND "held" = 0 AND "answered_at" IS NOT NULL
       ORDER BY "answered_at" DESC LIMIT 1"#,
    vec![
      key.role.clone().into(),
      key.path.clone().into(),
      key.provider.clone().into(),
      key.model.clone().into(),
      key.command.clone().into(),
    ],
  )
  .await?
  else {
    return Ok(None);
  };
  let kept = Kept::of(&row)?;

  if !valid(&kept, now) {
    drop_session(db, kept.id).await?;

    return Ok(None);
  }

  let taken = run(
    db,
    r#"UPDATE "role_sessions" SET "held" = 1 WHERE "id" = $1 AND "held" = 0"#,
    vec![kept.id.into()],
  )
  .await?;

  Ok((taken == 1).then_some(kept.session))
}

/// A kept session dropped with its record: no later run resumes it.
pub async fn drop_session<C: ConnectionTrait>(db: &C, id: i64) -> Result<(), Fault> {
  run(
    db,
    r#"UPDATE "role_sessions" SET "dropped" = 1, "held" = 0 WHERE "id" = $1"#,
    vec![id.into()],
  )
  .await?;

  Ok(())
}

/// The session a key's run went out on handed over: closed with its record, so the next run of its key opens a
/// new one (docs/sessions.md, 「A role's session」).
pub async fn hand_over<C: ConnectionTrait>(db: &C, session: &str) -> Result<u64, Fault> {
  run(
    db,
    r#"UPDATE "role_sessions" SET "dropped" = 1, "held" = 0 WHERE "session" = $1 AND "dropped" = 0"#,
    vec![session.into()],
  )
  .await
}

/// What a run that answered leaves of its session: the context its last turn carried, where it is read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Answered {
  pub at: i64,
  pub opened_at: i64,
  pub context_tokens: Option<i64>,
  pub context_share: Option<f64>,
}

/// The session a run answered on, kept for the next run of its key: the row it resumed given back with one
/// more run, or a row of its own where it opened a new session.
pub async fn keep<C: ConnectionTrait>(
  db: &C,
  key: &SessionKey,
  session: &str,
  answered: Answered,
) -> Result<(), Fault> {
  let resumed = run(
    db,
    r#"UPDATE "role_sessions" SET "held" = 0, "answered_at" = $2, "runs" = "runs" + 1, "context_tokens" = $3,
         "context_share" = $4
       WHERE "session" = $1 AND "role" = $5 AND "dropped" = 0"#,
    vec![
      session.into(),
      answered.at.into(),
      answered.context_tokens.into(),
      answered.context_share.into(),
      key.role.clone().into(),
    ],
  )
  .await?;

  if resumed > 0 {
    return Ok(());
  }

  run(
    db,
    r#"INSERT INTO "role_sessions" ("role", "path", "provider", "model", "effort", "account", "command_key",
         "session", "opened_at", "answered_at", "runs", "context_tokens", "context_share")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1, $11, $12)"#,
    vec![
      key.role.clone().into(),
      key.path.clone().into(),
      key.provider.clone().into(),
      key.model.clone().into(),
      key.effort.clone().into(),
      key.account.clone().into(),
      key.command.clone().into(),
      session.into(),
      answered.opened_at.into(),
      answered.at.into(),
      answered.context_tokens.into(),
      answered.context_share.into(),
    ],
  )
  .await?;

  Ok(())
}

/// A session a run took and did not answer on given back unchanged, so the next run of its key may resume
/// it while it is still valid.
pub async fn give_back<C: ConnectionTrait>(db: &C, session: &str) -> Result<(), Fault> {
  run(
    db,
    r#"UPDATE "role_sessions" SET "held" = 0 WHERE "session" = $1"#,
    vec![session.into()],
  )
  .await?;

  Ok(())
}

/// Every session kept at a path handed over, whatever its role, provider and model: a reset of the checkout's
/// sessions leaves the next run there a new one.
pub async fn hand_over_path<C: ConnectionTrait>(db: &C, path: &str) -> Result<u64, Fault> {
  run(
    db,
    r#"UPDATE "role_sessions" SET "dropped" = 1, "held" = 0 WHERE "path" = $1 AND "dropped" = 0"#,
    vec![path.into()],
  )
  .await
}
