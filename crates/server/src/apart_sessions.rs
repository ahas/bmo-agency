//! The session a run apart whose role keeps sessions leaves for the next run of the same command
//! (docs/sessions.md, 「A run apart's kept session」; docs/agent-roles.md, 「A role's sessions」): one record
//! an agent's command, taken by a run as it starts so no two runs share it, and written again with the session
//! that answered once the run has answered. A record past its cache lifetime is dropped as it is taken.

use std::path::Path;

use policy::roles::Sessions;
use sea_orm::ConnectionTrait;
use sha2::{Digest, Sha256};
use store::{Fault, one, run};

use crate::agent_roles;
use crate::lines::claude::Command;
use crate::worker_session::warmth::{set_aside_lifetime_ms, warm};

// The flag a command names the session it opens with, and the one its system prompt's file is handed by.
const OPENS: &str = "--session-id";
const SYSTEM_PROMPT_FILE: &str = "--system-prompt-file";

/// The sessions setting a run apart opens its session by: the role its agent's row names, else the role
/// the run names for itself where it stands for no row; a role the store does not hold keeps nothing.
pub async fn setting<C: ConnectionTrait>(db: &C, agent: &str, own: Option<&str>) -> Sessions {
  let role = match agent_roles::role_of_agent(db, agent).await {
    Ok(role) => role.or_else(|| own.map(str::to_owned)),
    Err(why) => {
      eprintln!("{agent}: the role its run holds could not be read, so it opens a new session: {why:?}");

      return Sessions::Fresh;
    }
  };
  let Some(role) = role else {
    return Sessions::Fresh;
  };

  match agent_roles::sessions_of(db, &role).await {
    Ok(sessions) => sessions.unwrap_or(Sessions::Fresh),
    Err(why) => {
      eprintln!(
        "{agent}: the sessions of the role {role} could not be read, so it opens a new one: {why:?}"
      );

      Sessions::Fresh
    }
  }
}

/// The key a command's kept session stands under: the agent, the line, the folder, the login's configuration
/// home and every word of the command but its session's, the system prompt's text standing for its file, so
/// another model, effort, login, tool list or prompt is another command.
pub fn key(agent: &str, line: &str, command: &Command, home: &Path) -> String {
  let mut hash = Sha256::new();
  let mut part = |text: &[u8]| {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text);
  };

  part(agent.as_bytes());
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

/// The session the command's record holds, taken so no other run resumes it meanwhile, where it is still warm
/// for the provider's model at `now`; a record past its lifetime is dropped all the same.
pub async fn take<C: ConnectionTrait>(
  db: &C,
  key: &str,
  provider: &str,
  model: &str,
  now: i64,
) -> Result<Option<String>, Fault> {
  let Some(row) = one(
    db,
    r#"DELETE FROM "apart_sessions" WHERE "key" = $1 RETURNING "session", "answered_at""#,
    vec![key.into()],
  )
  .await?
  else {
    return Ok(None);
  };
  let session: String = row.try_get("", "session")?;
  let answered_at: i64 = row.try_get("", "answered_at")?;

  Ok(warm(answered_at, now, set_aside_lifetime_ms(provider, model)).then_some(session))
}

/// The session a run answered on, kept for the next run of its command, with the moment it answered.
pub async fn keep<C: ConnectionTrait>(
  db: &C,
  key: &str,
  agent: &str,
  line: &str,
  session: &str,
  answered_at: i64,
) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "apart_sessions" ("key", "agent", "line", "session", "answered_at") VALUES ($1, $2, $3, $4, $5)
       ON CONFLICT ("key") DO UPDATE SET "session" = EXCLUDED."session", "answered_at" = EXCLUDED."answered_at""#,
    vec![key.into(), agent.into(), line.into(), session.into(), answered_at.into()],
  )
  .await?;

  Ok(())
}
