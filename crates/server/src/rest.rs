//! The deployment's rest as agency's store holds it (docs/api.md, 「Accounts」, the rest): it rests while the
//! service's own outage rest stands, or while every enabled Claude account has reached its own ceiling,
//! until the first of them comes back. Agency reads it and writes it; the rest that closes the last login
//! is published once, and whether a stand-in answers a room meanwhile, and what anyone is told of it, are
//! its consumers'.

use policy::rest::REST;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use store::{all, one};

use crate::Refusal;
use crate::control::claude_accounts::{Judging, SEED_ACCOUNT};
use crate::door::record::SessionUsage;
use crate::events::Event;
use crate::queue::settings;

// A rest that names no account: the service's outage, or a limit rest of this machine's own login.
const REST_UNTIL: &str = "rest_until";
// Why the rest began: `limit` or `outage`.
const REST_KIND: &str = "rest_kind";

async fn setting(db: &DatabaseConnection, key: &str) -> Result<Option<String>, Refusal> {
  let row = one(db, r#"SELECT "value" FROM "settings" WHERE "key" = $1"#, vec![key.into()]).await?;

  Ok(row.map(|row| row.try_get("", "value")).transpose()?)
}

/// The moment a login's own session usage rests it until, in epoch seconds: its session window's reset where
/// that window stands at the threshold, its week's where the week does — the policy's fallback while from
/// now where the week names none — and none otherwise.
pub fn rest_until_after(usage: &SessionUsage, now: i64) -> Option<f64> {
  if usage.utilization >= REST.threshold {
    return Some(usage.resets_at);
  }

  if usage.weekly_utilization.is_some_and(|weekly| weekly >= REST.weekly_threshold) {
    return Some(usage.weekly_resets_at.unwrap_or((now + REST.fallback_seconds) as f64));
  }

  None
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
pub async fn resting(judging: Judging<'_>, now: i64) -> Result<bool, Refusal> {
  Ok(rest_until(judging, now).await?.is_some())
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
pub async fn rest_until(judging: Judging<'_>, now: i64) -> Result<Option<i64>, Refusal> {
  let db = judging.db;

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

    match account_rest_until(judging, &id).await?.filter(|until| *until > now) {
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
// (`control::claude_accounts`): its own rest, or a window of its own agency keeps read full.
async fn account_rest_until(judging: Judging<'_>, account: &str) -> Result<Option<i64>, Refusal> {
  crate::control::claude_accounts::rest_until(judging, account)
    .await
    .map_err(|refused| Refusal::new(refused.status, "account-rest-unread", refused.error))
}

/// Why the deployment rested, where its rest has just passed: every rest whose moment is behind `now` cleared,
/// and the kind of the silence that ended — the service's outage or a limit — where the deployment was silent
/// and rests no longer; none otherwise.
pub async fn clear_passed_rests(judging: Judging<'_>, now: i64) -> Result<Option<RestKind>, Refusal> {
  let db = judging.db;
  let unnamed = moment(db, REST_UNTIL).await?;
  let outage_held = setting(db, REST_KIND).await?.as_deref() == Some("outage") && unnamed.is_some();
  let accounts = all(
    db,
    r#"SELECT "id" FROM "claude_accounts" WHERE "enabled" <> 0 ORDER BY "place", "rowid""#,
    vec![],
  )
  .await?;
  let mut held = vec![];
  let mut passed = vec![];

  if unnamed.is_some_and(|until| until <= now) {
    passed.push(REST_UNTIL.to_owned());
    passed.push(REST_KIND.to_owned());
  }

  for account in &accounts {
    let id: String = account.try_get("", "id")?;
    let own_key = format!("{REST_UNTIL}:{id}");

    held.push(account_rest_until(judging, &id).await?);

    if moment(db, &own_key).await?.is_some_and(|until| until <= now) {
      passed.push(own_key);
    }
  }

  let was_silent = outage_held || (!held.is_empty() && held.iter().all(Option::is_some));

  for key in &passed {
    settings::forget(db, key).await?;
  }

  if passed.is_empty() || !was_silent || resting(judging, now).await? {
    return Ok(None);
  }

  Ok(Some(if outage_held { RestKind::Outage } else { RestKind::Limit }))
}

/// What a rest is: one login's limit, one login's refusal of a run, or the service's own outage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestKind {
  Limit,
  /// A login that refused a run, a request for `/login` or a 403, rests under its own name like a limit; the
  /// owner hears of it as the login notice tells it, never as a limit.
  Refused,
  Outage,
}

impl RestKind {
  /// The word the rest's kind is kept and published under.
  pub fn word(self) -> &'static str {
    match self {
      Self::Limit => "limit",
      Self::Refused => "refused",
      Self::Outage => "outage",
    }
  }

  // Whether the rest is one login's own, written under its name.
  fn of_a_login(self) -> bool {
    self != Self::Outage
  }
}

/// The rest that closed the last login that would answer, published once as it is written (docs/api.md,
/// 「Events」): the account it was written under, none for the service's outage, its kind, until when, in
/// epoch seconds, and the words the owner is told it in.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentResting {
  pub account: Option<String>,
  pub kind: &'static str,
  pub until: i64,
  pub notice: String,
}

impl Event for DeploymentResting {
  const NAME: &'static str = "rest.changed";
}

impl DeploymentResting {
  /// The rest as it is published, in the policy's words.
  pub fn of(kind: RestKind, account: &str, until: i64) -> Self {
    Self {
      account: kind.of_a_login().then(|| account.to_owned()),
      kind: kind.word(),
      until,
      notice: REST.resting.clone(),
    }
  }
}

/// A rest written down, answering whether it is the one that silenced the deployment: nothing was resting
/// before it and nothing answers now. A limit's rest and a refusal's are their login's, and a login already
/// resting is left as it stands; an outage is the service's, and a second one while the first goes on writes nothing. `until`
/// is in epoch seconds, written in plain decimal, with no fraction where it is whole.
pub async fn write_rest(
  judging: Judging<'_>,
  until: f64,
  kind: RestKind,
  account: &str,
  now: i64,
) -> Result<bool, Refusal> {
  let db = judging.db;
  let was_resting = resting(judging, now).await?;

  match kind {
    RestKind::Outage => {
      if outage_rest(db, now).await?.is_some() {
        return Ok(false);
      }

      // A limit rest written in the unnamed pair is this machine's login's, put under its own name before
      // the service's rest takes the pair.
      let unnamed = if setting(db, REST_KIND).await?.as_deref() == Some("outage") {
        None
      } else {
        moment(db, REST_UNTIL).await?
      };
      let seed = format!("{REST_UNTIL}:{SEED_ACCOUNT}");

      if let Some(unnamed) = unnamed
        && moment(db, &seed).await?.is_none()
      {
        settings::set(db, &seed, &unnamed.to_string()).await?;
      }

      settings::set(db, REST_UNTIL, &until.to_string()).await?;
      settings::set(db, REST_KIND, "outage").await?;
    }
    RestKind::Limit | RestKind::Refused => {
      if account_rest_until(judging, account).await?.is_some_and(|standing| standing > now) {
        return Ok(false);
      }

      settings::set(db, &format!("{REST_UNTIL}:{account}"), &until.to_string()).await?;
    }
  }

  let silenced = !was_resting && resting(judging, now).await?;
  let whose = match kind {
    RestKind::Outage => "the service".to_owned(),
    RestKind::Limit | RestKind::Refused => format!("account {account}"),
  };

  println!(
    "{whose} rests until {}{}",
    chrono::DateTime::from_timestamp(until as i64, 0)
      .map(|moment| moment.to_rfc3339())
      .unwrap_or_default(),
    if silenced { ": nothing answers now" } else { "" }
  );

  Ok(silenced)
}
