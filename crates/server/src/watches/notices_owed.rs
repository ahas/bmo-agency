//! The outbox's sender as agency's clock runs it (docs/agent-control.md, 「An item's change is told to its
//! orderer」): every few seconds the standing notices whose condition ended are let go first — a review's check
//! no longer due — so none is told again once it is no longer owed, and then every row due is sent. A commit
//! waiting to go live is let go by the job service's events, once its item no longer reads so.

use std::collections::BTreeMap;

use sea_orm::DatabaseConnection;

/// One pass of the sender. A condition that could not be read is said and lets nothing go; answers how many
/// notices the receiver took.
pub async fn pass(db: &DatabaseConnection, vars: &BTreeMap<String, String>) -> Result<usize, String> {
  if let Err(why) = crate::job_review::checks_owed(db, vars).await {
    eprintln!("notices owed: the checks owed were not brought up to date: {why}");
  }

  crate::outbox::send_due(db, vars).await
}
