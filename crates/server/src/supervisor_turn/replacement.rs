//! The replacement an agent's one session across rooms owes (docs/sessions.md, 「Session replacement」): written
//! down where a turn found the session's prompt moved, or a document reload named the agent, and read off by
//! the pass that makes it while the agent is quiet. The word kept is why it is owed, for the journal.

use sea_orm::ConnectionTrait;

use crate::Refusal;
use crate::queue::settings;

fn key(agent: &str) -> String {
  format!("handoff_owed:{agent}")
}

/// Why the agent's session owes a replacement, or none where it owes none.
pub async fn owed(db: &impl ConnectionTrait, agent: &str) -> Result<Option<String>, Refusal> {
  Ok(settings::get(db, &key(agent)).await?.filter(|why| !why.is_empty()))
}

/// The replacement written down as owed, with why; one owed already keeps its first reason.
pub async fn owe(db: &impl ConnectionTrait, agent: &str, why: &str) -> Result<(), Refusal> {
  if owed(db, agent).await?.is_some() {
    return Ok(());
  }

  settings::set(db, &key(agent), why).await
}

/// The debt settled: the replacement was made, or nothing was left to replace.
pub async fn settle(db: &impl ConnectionTrait, agent: &str) -> Result<(), Refusal> {
  settings::forget(db, &key(agent)).await
}
