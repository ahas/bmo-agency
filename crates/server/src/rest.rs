//! The deployment's rest as agency's store holds it (docs/api.md, 「Accounts」, the rest): it rests while the
//! service's own outage rest stands, or while every enabled Claude account has reached its own ceiling, until
//! the first of them comes back. Agency reads it; whether a stand-in answers a room meanwhile, and what anyone
//! is told of it, are its consumers'.

use sea_orm::DatabaseConnection;
use store::{all, one};

use crate::Refusal;

// A rest that names no account: the service's outage, or a limit rest of this machine's own login.
const REST_UNTIL: &str = "rest_until";
// Why the rest began: `limit` or `outage`.
const REST_KIND: &str = "rest_kind";

async fn setting(db: &DatabaseConnection, key: &str) -> Result<Option<String>, Refusal> {
  let row = one(db, r#"SELECT "value" FROM "settings" WHERE "key" = $1"#, vec![key.into()]).await?;

  Ok(row.map(|row| row.try_get("", "value")).transpose()?)
}

/// A rest's moment as its value is written: the leading whole number, a sign before it taken with it.
pub fn leading_number(value: &str) -> Option<i64> {
  let value = value.trim_start();
  let digits = value.strip_prefix(['-', '+']).unwrap_or(value);
  let length = value.len() - digits.len() + digits.chars().take_while(char::is_ascii_digit).count();

  value[..length].parse().ok()
}

// A moment a rest is kept until, in epoch seconds.
async fn moment(db: &DatabaseConnection, key: &str) -> Result<Option<i64>, Refusal> {
  Ok(setting(db, key).await?.and_then(|value| leading_number(&value)))
}

/// Whether the deployment rests now, `now` in epoch seconds.
pub async fn resting(db: &DatabaseConnection, now: i64) -> Result<bool, Refusal> {
  Ok(rest_until(db, now).await?.is_some())
}

// The service's own rest, which belongs to no account, where it stands past `now`.
async fn outage_rest(db: &DatabaseConnection, now: i64) -> Result<Option<i64>, Refusal> {
  if setting(db, REST_KIND).await?.as_deref() != Some("outage") {
    return Ok(None);
  }

  Ok(moment(db, REST_UNTIL).await?.filter(|until| *until > now))
}

/// Until when the deployment rests, in epoch seconds: the service's own rest where one stands, or else the
/// soonest end of the enabled accounts' rests where every one of them rests; none where it does not rest.
pub async fn rest_until(db: &DatabaseConnection, now: i64) -> Result<Option<i64>, Refusal> {
  if let Some(until) = outage_rest(db, now).await? {
    return Ok(Some(until));
  }

  let accounts = all(
    db,
    r#"SELECT "id" FROM "claude_accounts" WHERE "enabled" <> 0 ORDER BY "place", "rowid""#,
    vec![],
  )
  .await?;
  let mut soonest: Option<i64> = None;

  for account in &accounts {
    let id: String = account.try_get("", "id")?;

    match account_rest_until(db, &id).await?.filter(|until| *until > now) {
      Some(until) => soonest = Some(soonest.map_or(until, |held| held.min(until))),
      None => return Ok(None),
    }
  }

  Ok(soonest)
}

/// Why the deployment rests: the service's outage where its own rest stands, a usage limit otherwise.
pub async fn rest_kind(db: &DatabaseConnection, now: i64) -> Result<&'static str, Refusal> {
  Ok(if outage_rest(db, now).await?.is_some() { "outage" } else { "limit" })
}

// How long one account's own limit rest stands, as the one account judgement reads it
// (`control::claude_accounts`): its own rest, or a window of its own read full.
async fn account_rest_until(db: &DatabaseConnection, account: &str) -> Result<Option<i64>, Refusal> {
  crate::control::claude_accounts::rest_until(db, account)
    .await
    .map_err(|refused| Refusal::new(refused.status, "account-rest-unread", refused.error))
}
