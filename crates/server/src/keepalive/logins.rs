//! The Claude logins' watch on the keepalive pass (docs/cache-fallback.md, 「A login that is about to
//! end」): a login that has ended marked so every walk passes it over, a login signed in again losing
//! that mark, and a login near its end, or past it, published once for that ending as
//! `account-login-notice`, which the consumer owning the owner's chat says there; beside each, the
//! account's token for the work nodes (docs/work-nodes.md, 「The logins on a node」), marked apart from its
//! login here. Nothing here launches anything, and nothing here keeps a credential.

use std::path::Path;

use policy::providers::claude_code::CLAUDE_CODE;
use serde::Serialize;
use store::run;

use crate::Refusal;
use crate::control::claude_accounts::{
  Account, Judging, account_open, accounts, login_end, login_standing,
};
use crate::door::place_run::read_run;
use crate::door::turn::{account_of_login, program_login};
use crate::events::{Bus, Event};
use crate::lines::fault;
use crate::lines::place::{Place, places_dir};
use crate::queue::settings;
use crate::watches::links::Roots;

/// Which login of an account a notice is about: the one on this machine, or its token for the work nodes.
const LOGIN_HERE: &str = "here";
const NODE_TOKEN: &str = "node-token";

/// A login's end found by the watch, published once for that ending (docs/api.md, 「Events」): the account,
/// which login it is, how it stands, how long is left where that is read, and the words the policy composes
/// of it with the one command the owner runs. Saying it to the owner is the consumer's.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginNotice {
  pub account: String,
  /// `here` or `node-token`.
  pub login: &'static str,
  /// `gone`, `ending` or `missing`.
  pub stands: &'static str,
  pub left_ms: Option<f64>,
  pub notice: String,
}

impl Event for AccountLoginNotice {
  const NAME: &'static str = "account.changed";
}

// The variables a program is handed its login by.
const LOGIN_VARIABLES: [&str; 2] = ["CLAUDE_CONFIG_DIR", "CLAUDE_CODE_OAUTH_TOKEN"];

// What a warning about one ending is recorded under, so the same ending is said once.
fn notice_key(account: &str) -> String {
  format!("claude_login_notice:{account}")
}

async fn every(db: &sea_orm::DatabaseConnection) -> Result<Vec<Account>, Refusal> {
  accounts(db).await.map_err(|refused| fault(refused.error))
}

async fn named(db: &sea_orm::DatabaseConnection, id: &str) -> Result<Option<Account>, Refusal> {
  Ok(every(db).await?.into_iter().find(|account| account.id == id))
}

/// Every login read once on the pass, as it stands at `now` in milliseconds, each account's token for the
/// work nodes with it.
pub async fn watch(judging: Judging<'_>, events: &Bus, roots: &Roots, now: i64) -> Result<(), Refusal> {
  let db = judging.db;

  // An endpoint account holds no login for the watch to read: the judgement lifts its mark off its key.
  for account in every(db).await?.into_iter().filter(|account| account.credential_file().is_some()) {
    clear_if_back(db, &account.id, now).await?;

    let held = named(db, &account.id).await?.unwrap_or(account);

    if login_end(&held).is_some_and(|end| end <= now as f64) {
      mark_gone(db, &held.id, now).await?;
    }

    let held = named(db, &held.id).await?.unwrap_or(held);

    tell(db, events, &held, now).await?;

    // A token account's own file is its login here too, whose year the watch above already reads.
    if held.credential.node_token_apart()
      && held.node_token_end(&roots.data).is_some_and(|end| end <= now as f64)
    {
      mark_node_token_gone(db, &held, &roots.data).await?;
    }

    tell_node_token(db, events, &held, roots, now).await?;
  }

  retire_closed(judging, &roots.data, now).await
}

/// Every standing program whose login cannot answer now — off, its window full, waiting for a person —
/// taken down where no turn is in it, so the loop in its pane does not start it again on that login and
/// resume its session there: the place is handed back to the host, and the next turn of that agent stands
/// its program again on the login the walk offers then, a fresh session where the login moved. A program
/// that names no login of its own is the machine's and is left alone.
pub async fn retire_closed(judging: Judging<'_>, data_root: &Path, now: i64) -> Result<(), Refusal> {
  let accounts = every(judging.db).await?;
  let Ok(places) = std::fs::read_dir(places_dir(data_root)) else { return Ok(()) };

  for entry in places.flatten() {
    let Some(agent) = entry.file_name().to_str().map(str::to_owned) else { continue };
    let place = Place::of(data_root, &agent);
    let Some(program) = place.program() else { continue };
    let env = &program.command.env;

    if !LOGIN_VARIABLES
      .iter()
      .any(|variable| env.get(*variable).is_some_and(|value| !value.is_empty()))
    {
      continue;
    }

    if read_run(&place.path).is_some_and(|run| run.ended_at.is_none()) {
      continue;
    }

    let Some(account) = account_of_login(&accounts, &program_login(Path::new(""), env), data_root) else {
      continue;
    };

    if account_open(judging, &account, now).await.map_err(|refused| fault(refused.error))? {
      continue;
    }

    if place.release().await.is_some() {
      println!("{agent}: its program stood on {account}, which does not answer now, and was taken down");
    }
  }

  Ok(())
}

// A login standing again, read off the credential rather than anybody's word: a moment in the future is
// a login somebody has signed in to since, as a key written after the mark is an endpoint account's, and
// the mark goes.
async fn clear_if_back(db: &sea_orm::DatabaseConnection, id: &str, now: i64) -> Result<(), Refusal> {
  let Some(held) = named(db, id).await? else { return Ok(()) };

  if held.needs_login_at.is_none() || !held.login_back(now) {
    return Ok(());
  }

  run(
    db,
    r#"UPDATE "claude_accounts" SET "needs_login_at" = NULL WHERE "id" = $1"#,
    vec![id.into()],
  )
  .await?;
  println!("claude account {id}: its login is standing again");

  Ok(())
}

/// A login found gone, which the walk passes over until a person signs in again.
pub async fn mark_gone(db: &sea_orm::DatabaseConnection, id: &str, now: i64) -> Result<(), Refusal> {
  let Some(held) = named(db, id).await? else { return Ok(()) };

  if held.needs_login_at.is_some() {
    return Ok(());
  }

  run(
    db,
    r#"UPDATE "claude_accounts" SET "needs_login_at" = $1 WHERE "id" = $2"#,
    vec![(now / 1000).into(), id.into()],
  )
  .await?;
  println!("claude account {id}: its login is gone and waits for a person");

  Ok(())
}

/// A moment as JavaScript writes a number into text, so a warning recorded here or by the TypeScript side
/// (`ts/agent/claude-login.ts`) is read as the same ending by the other.
pub fn moment_text(moment: f64) -> String {
  if moment.fract() == 0.0 && moment.abs() < 1e15 {
    format!("{}", moment as i64)
  } else {
    format!("{moment}")
  }
}

// How long is left, as the login's message says it.
fn span(left: f64) -> String {
  let wording = &CLAUDE_CODE.login;

  if left < 60.0 * 60_000.0 {
    wording.minutes.replace("{n}", &((left / 60_000.0).round().max(1.0) as i64).to_string())
  } else {
    wording.hours.replace("{n}", &((left / (60.0 * 60_000.0)).round() as i64).to_string())
  }
}

/// The warning about one login's ending, as the owner reads it: the account, when it ends, and the one
/// command the owner runs, never anything out of its credential.
pub fn notice(account: &Account, gone: bool, left_ms: Option<f64>) -> String {
  let wording = &CLAUDE_CODE.login;
  let span = left_ms.map(span);
  let when = match (gone, span) {
    (true, _) => wording.gone.clone(),
    (false, None) => wording.ending.clone(),
    (false, Some(span)) => wording.ending_in.replace("{left}", &span),
  };
  let token = account.credential.token_file().is_some();
  let command = if token { &wording.token_command } else { &wording.directory_command };

  [
    wording.head.replace("{name}", &account.name).replace("{when}", &when),
    wording.run.clone(),
    command.replace("{path}", &account.credential.path().to_string_lossy()),
    if token { wording.token_after.clone() } else { wording.directory_after.clone() },
    if gone { wording.gone_after.clone() } else { wording.ending_after.clone() },
  ]
  .join("\n")
}

/// The ending published once: recorded under the moment it is about as it is published, so a later pass
/// over the same ending publishes nothing. Delivering it is the consumer's. An endpoint account has no
/// login to end and is never told about.
pub async fn tell(
  db: &sea_orm::DatabaseConnection,
  events: &Bus,
  account: &Account,
  now: i64,
) -> Result<(), Refusal> {
  let standing = login_standing(account, now);

  if standing == "standing" || account.credential_file().is_none() {
    return Ok(());
  }

  let end = login_end(account);
  let about = end.map_or_else(|| account.needs_login_at.unwrap_or(0).to_string(), moment_text);
  let key = notice_key(&account.id);

  if settings::get(db, &key).await?.as_deref() == Some(about.as_str()) {
    return Ok(());
  }

  let left = end.map(|end| (end - now as f64).max(0.0));
  let gone = standing == "gone";

  events.publish(AccountLoginNotice {
    account: account.id.clone(),
    login: LOGIN_HERE,
    stands: if gone { "gone" } else { "ending" },
    left_ms: left,
    notice: notice(account, gone, left),
  });

  settings::set(db, &key, &about).await
}

// The settings an account's token for the work nodes is kept apart under (docs/work-nodes.md, 「The
// logins on a node」): the mark of one found gone, and the notice said about it.
fn node_gone_key(account: &str) -> String {
  format!("claude_node_token_gone:{account}")
}

fn node_notice_key(account: &str) -> String {
  format!("claude_node_token_notice:{account}")
}

// Which token for the nodes this machine holds now: the moment its file was written, or none held.
fn node_stamp(account: &Account, data_root: &Path) -> String {
  account.node_token_written(data_root).map_or_else(|| "none".to_owned(), moment_text)
}

/// Whether the account's token for the work nodes is marked gone. The mark names the token held when it
/// was made, so a token made since stands clear of it with nobody's word.
pub async fn node_token_gone(
  db: &sea_orm::DatabaseConnection,
  account: &Account,
  data_root: &Path,
) -> Result<bool, Refusal> {
  Ok(
    settings::get(db, &node_gone_key(&account.id)).await?.as_deref()
      == Some(node_stamp(account, data_root).as_str()),
  )
}

/// The account's token for the work nodes found gone, as it is held now, which a run on a node passes
/// over until a person makes another; nothing for an endpoint account, which has none.
pub async fn mark_node_token_gone(
  db: &sea_orm::DatabaseConnection,
  account: &Account,
  data_root: &Path,
) -> Result<(), Refusal> {
  if account.node_token_file(data_root).is_none() || node_token_gone(db, account, data_root).await? {
    return Ok(());
  }

  settings::set(db, &node_gone_key(&account.id), &node_stamp(account, data_root)).await?;
  println!(
    "claude account {}: its token for the work nodes is gone and waits for a person",
    account.id
  );

  Ok(())
}

/// How an account's token for the work nodes stands where it is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeToken {
  Gone,
  Missing,
  Ending,
}

/// The warning about an account's token for the work nodes, as the owner reads it: the account, how its
/// token stands, and the one command the owner runs on this machine, never anything out of the token.
pub fn node_notice(account: &Account, deploy: &Path, standing: NodeToken, left_ms: Option<f64>) -> String {
  let wording = &CLAUDE_CODE.node_login;
  let when = match standing {
    NodeToken::Gone => wording.gone.clone(),
    NodeToken::Missing => wording.missing.clone(),
    NodeToken::Ending => wording.ending_in.replace("{left}", &span(left_ms.unwrap_or(0.0))),
  };
  let name = account.name.replace('\'', r"'\''");

  [
    wording.head.replace("{name}", &account.name).replace("{when}", &when),
    CLAUDE_CODE.login.run.clone(),
    wording.command.replace("{deploy}", &deploy.to_string_lossy()).replace("{name}", &name),
    wording.after.clone(),
    if standing == NodeToken::Ending {
      CLAUDE_CODE.login.ending_after.clone()
    } else {
      wording.gone_after.clone()
    },
  ]
  .join("\n")
}

/// An account's token for the work nodes published once: gone or never made where it is marked so, ending
/// inside the notice window otherwise. A token account's own file is its login here too, and its ending is
/// told as the login's.
pub async fn tell_node_token(
  db: &sea_orm::DatabaseConnection,
  events: &Bus,
  account: &Account,
  roots: &Roots,
  now: i64,
) -> Result<(), Refusal> {
  if account.node_token_file(&roots.data).is_none() {
    return Ok(());
  }

  let stamp = node_stamp(account, &roots.data);
  let end = account.node_token_end(&roots.data).filter(|_| account.credential.node_token_apart());
  let (standing, about) = if node_token_gone(db, account, &roots.data).await? {
    let standing = if stamp == "none" { NodeToken::Missing } else { NodeToken::Gone };

    (standing, format!("gone:{stamp}"))
  } else {
    match end {
      Some(end) if end - now as f64 <= notice_ahead_ms() => (NodeToken::Ending, moment_text(end)),
      _ => return Ok(()),
    }
  };
  let key = node_notice_key(&account.id);

  if settings::get(db, &key).await?.as_deref() == Some(about.as_str()) {
    return Ok(());
  }

  let left = end.map(|end| (end - now as f64).max(0.0));

  events.publish(AccountLoginNotice {
    account: account.id.clone(),
    login: NODE_TOKEN,
    stands: match standing {
      NodeToken::Gone => "gone",
      NodeToken::Missing => "missing",
      NodeToken::Ending => "ending",
    },
    left_ms: left,
    notice: node_notice(account, &roots.deploy, standing, left),
  });

  settings::set(db, &key, &about).await
}

// How long before its end a token for the nodes is told as ending: the login's own notice window.
fn notice_ahead_ms() -> f64 {
  CLAUDE_CODE.login.notice_ahead_hours as f64 * 60.0 * 60_000.0
}
