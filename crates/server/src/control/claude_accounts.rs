//! The Claude accounts as agency reads them: the order the owner set, which of them would answer now, and
//! the login an automatic run goes out on (docs/cache-fallback.md, 「The Claude Code road is a list of
//! accounts」). A login marked as waiting for a person is asked once more of its own credential, and the
//! mark goes where the owner has signed in again. An account's windows are the cache siren's, asked of it
//! with how long its reading stands (docs/api.md, 「The cache siren」).

use std::path::{Path, PathBuf};

use harness_providers::Machine;
use policy::providers::claude_code::CLAUDE_CODE;
use policy::rest::REST;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use store::{all, one, run};

use super::{Refused, set_setting, setting};
use crate::now_ms;
use crate::peers::cache_siren::CacheSiren;
use crate::places::Ground;

/// What the account judgement reads: the store, and the cache siren, which keeps every account's windows.
#[derive(Clone, Copy)]
pub struct Judging<'a> {
  pub db: &'a DatabaseConnection,
  pub siren: &'a dyn CacheSiren,
}

impl<'a> Judging<'a> {
  /// The judgement on agency's ground.
  pub fn of(ground: &'a Ground) -> Self {
    Self {
      db: &ground.db,
      siren: ground.peers.cache_siren.as_ref(),
    }
  }
}

/// The account a fresh store is seeded with, standing for the login this machine already holds.
pub const SEED_ACCOUNT: &str = "default";

// The settings the walk reads: the account chosen to run on now, and the login in use.
pub(crate) const CHOSEN_KEY: &str = "claude_account_chosen";
pub(crate) const IN_USE_KEY: &str = "claude_account_in_use";

// The file a directory account's login sits in.
pub const CREDENTIAL_FILE: &str = ".credentials.json";

// The provider an account's windows are kept under, and the one window held to the five-hour ceiling.
const CLAUDE_CODE_MODE: &str = "claude-code";
const FIVE_HOUR_WINDOW: &str = "five_hour";

// A token carries no ending of its own, so what stands for it is the year it was issued for, as Claude
// Code declares it.
fn token_life_ms() -> f64 {
  CLAUDE_CODE.login.token_life_days as f64 * 24.0 * 60.0 * 60_000.0
}

/// One account as the store holds it.
#[derive(Clone, Debug)]
pub struct Account {
  pub id: String,
  pub name: String,
  pub place: i64,
  pub enabled: bool,
  pub credential_kind: String,
  pub credential_path: String,
  pub needs_login_at: Option<i64>,
  /// The user whose account it is, where the multi-user model names one.
  pub user_id: Option<i64>,
}

impl Account {
  /// The directory the CLI keeps this account's records under: its configuration home where it has
  /// one, and the machine's for a token account, which has none.
  pub fn records_dir(&self, machine: &Machine) -> PathBuf {
    if self.credential_kind == "directory" {
      PathBuf::from(&self.credential_path)
    } else {
      machine_config_dir(machine)
    }
  }

  /// Where its login sits.
  pub fn credential_file(&self) -> PathBuf {
    if self.credential_kind == "token" {
      PathBuf::from(&self.credential_path)
    } else {
      Path::new(&self.credential_path).join(CREDENTIAL_FILE)
    }
  }

  /// Where this machine keeps its token for the work nodes (docs/work-nodes.md, 「The logins on a
  /// node」): a token account's own file, and a file named by its id under the data root's env
  /// directory for any other.
  pub fn node_token_file(&self, data_root: &Path) -> PathBuf {
    if self.credential_kind == "token" {
      PathBuf::from(&self.credential_path)
    } else {
      data_root.join("env").join(&CLAUDE_CODE.node_login.token_dir).join(&self.id)
    }
  }

  /// Its token for the work nodes, none where this machine holds none.
  pub fn node_token(&self, data_root: &Path) -> Option<String> {
    std::fs::read_to_string(self.node_token_file(data_root))
      .ok()
      .map(|token| token.trim().to_owned())
      .filter(|token| !token.is_empty())
  }

  /// When its token for the work nodes was written, in milliseconds, which tells one token from the
  /// next; none where this machine holds none.
  pub fn node_token_written(&self, data_root: &Path) -> Option<f64> {
    self.node_token(data_root)?;

    written_ms(&self.node_token_file(data_root))
  }

  /// When its token for the work nodes ends: the year it was issued for, from the day it was written.
  pub fn node_token_end(&self, data_root: &Path) -> Option<f64> {
    self.node_token_written(data_root).map(|written| written + token_life_ms())
  }
}

// When a file was written, in milliseconds with the fraction kept, to the last digit: a moment a notice
// is recorded under is read off the same file again next, and the two readings must agree.
fn written_ms(path: &Path) -> Option<f64> {
  let written = std::fs::metadata(path).ok()?.modified().ok()?;
  let since = written.duration_since(std::time::UNIX_EPOCH).ok()?;

  Some(since.as_secs() as f64 * 1000.0 + f64::from(since.subsec_nanos()) / 1e6)
}

/// The configuration directory the CLI itself would use: `CLAUDE_CONFIG_DIR` where it is set, and
/// `.claude` under the home otherwise.
pub fn machine_config_dir(machine: &Machine) -> PathBuf {
  machine
    .var("CLAUDE_CONFIG_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(machine.var("HOME").unwrap_or_default()).join(".claude"))
}

/// Every account in the order the owner set, on or off.
pub async fn accounts(db: &DatabaseConnection) -> Result<Vec<Account>, Refused> {
  all(
    db,
    "SELECT id, name, place, enabled, credential_kind, credential_path, needs_login_at, user_id
     FROM claude_accounts ORDER BY place, rowid",
    vec![],
  )
  .await?
  .iter()
  .map(|row| {
    Ok(Account {
      id: row.try_get("", "id")?,
      name: row.try_get("", "name")?,
      place: row.try_get("", "place")?,
      enabled: row.try_get::<i64>("", "enabled")? == 1,
      credential_kind: row.try_get("", "credential_kind")?,
      credential_path: row.try_get("", "credential_path")?,
      needs_login_at: row.try_get("", "needs_login_at")?,
      user_id: row.try_get("", "user_id")?,
    })
  })
  .collect()
}

/// A moment a setting holds, read as the whole number it was written as.
async fn moment(db: &DatabaseConnection, key: &str) -> Result<Option<i64>, Refused> {
  Ok(setting(db, key).await?.and_then(|value| leading_integer(&value)))
}

/// A whole number at the head of a text, as `parseInt` reads one.
pub fn leading_integer(text: &str) -> Option<i64> {
  let text = text.trim_start();
  let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
  let length = digits.bytes().take_while(u8::is_ascii_digit).count();

  if length == 0 {
    return None;
  }

  text[..text.len() - digits.len() + length].parse().ok()
}

/// How long one account answers nothing for its limit, in epoch seconds, or nothing: the later of the
/// rest written under it — a rest written with no login named is the seeded account's, unless it is the
/// service's outage — and a window of its own read at its ceiling, not yet reset, by a reading still fresh.
/// A window is read on the siren's clock and off every answer, so a login whose window filled is closed
/// whether or not a run has met the refusal, and whatever became of the rest written for it; a reading
/// nothing renewed closes nothing, so the next run renews the login's token or meets the refusal that rests
/// it.
pub async fn rest_until(judging: Judging<'_>, account: &str) -> Result<Option<i64>, Refused> {
  let db = judging.db;
  let own = match moment(db, &format!("rest_until:{account}")).await? {
    Some(own) => Some(own),
    None if account == SEED_ACCOUNT && setting(db, "rest_kind").await?.as_deref() != Some("outage") => {
      moment(db, "rest_until").await?
    }
    None => None,
  };

  Ok(own.max(full_window_until(judging.siren, account, now_ms().div_euclid(1000)).await))
}

/// The share past which a window of this name is full: the five-hour window against its own ceiling and
/// every longer one against the weekly one.
pub fn window_ceiling(name: &str) -> f64 {
  if name == FIVE_HOUR_WINDOW { REST.threshold } else { REST.weekly_threshold }
}

// The moment the latest of an account's windows read at its ceiling resets, where one is, has not reset by
// `now` and was read freshly: within the span the siren says its reading stands. A full window that named
// no reset is kept for the policy's fallback after it was read. A siren that cannot be asked closes
// nothing: the next run meets the refusal that rests the login.
async fn full_window_until(siren: &dyn CacheSiren, account: &str, now: i64) -> Option<i64> {
  let reading = match siren.usage_windows(CLAUDE_CODE_MODE, account).await {
    Ok(reading) => reading,
    Err(fault) => {
      eprintln!("claude account {account}: its windows could not be read: {fault}");
      return None;
    }
  };
  let fresh_after = now - reading.fresh().as_secs() as i64;
  let mut until = None;

  for window in &reading.windows {
    if window.utilization < window_ceiling(&window.name) || window.seen_at < fresh_after {
      continue;
    }

    let reset = window.resets_at.unwrap_or(window.seen_at + REST.fallback_seconds);

    if reset > now {
      until = until.max(Some(reset));
    }
  }

  until
}

/// Whether an account answers nothing until its own window resets.
pub async fn resting(judging: Judging<'_>, account: &str, now: i64) -> Result<bool, Refused> {
  Ok(rest_until(judging, account).await?.is_some_and(|until| until > now.div_euclid(1000)))
}

// Every moment a credential file states under a name that speaks of expiring, read as seconds or
// milliseconds by its own size.
fn expiries(value: &Value, found: &mut Vec<f64>) {
  if let Value::Object(fields) = value {
    for (name, held) in fields {
      if let Some(number) = held.as_f64()
        && name.to_lowercase().contains("expir")
        && number > 1_000_000_000.0
      {
        found.push(if number > 1_000_000_000_000.0 { number } else { number * 1000.0 });
      }

      expiries(held, found);
    }
  } else if let Value::Array(items) = value {
    for held in items {
      expiries(held, found);
    }
  }
}

/// The moment a credential states: the earliest expiry in its file, or for a token that states none,
/// a year from when its file was written. Nothing where the file cannot be read or states nothing.
pub fn credential_expiry(account: &Account) -> Option<f64> {
  let path = account.credential_file();
  let text = std::fs::read_to_string(&path).ok()?;
  let said = serde_json::from_str::<Value>(&text).ok().and_then(|held| {
    let mut found = Vec::new();

    expiries(&held, &mut found);
    found.into_iter().reduce(f64::min)
  });

  if account.credential_kind != "token" || said.is_some() {
    return said;
  }

  written_ms(&path).map(|written| written + token_life_ms())
}

/// Whether a login is waiting for a person. One marked so is asked once more of its own credential:
/// signing in again writes a new moment there, and the mark goes.
async fn needs_login(db: &DatabaseConnection, account: &Account, now: i64) -> Result<bool, Refused> {
  if account.needs_login_at.is_none() {
    return Ok(false);
  }

  if credential_expiry(account).is_none_or(|expiry| expiry <= now as f64) {
    return Ok(true);
  }

  run(
    db,
    "UPDATE claude_accounts SET needs_login_at = NULL WHERE id = $1",
    vec![account.id.clone().into()],
  )
  .await?;
  eprintln!("claude account {}: its login is standing again", account.id);

  Ok(false)
}

// The logins that are on, in the order the road takes them: the login in use first, then the account the
// owner chose where one was chosen, then the owner's order.
async fn road_order(db: &DatabaseConnection) -> Result<Vec<Account>, Refused> {
  let enabled: Vec<Account> = accounts(db).await?.into_iter().filter(|one| one.enabled).collect();
  let mut heads = Vec::new();

  for key in [IN_USE_KEY, CHOSEN_KEY] {
    if let Some(id) = setting(db, key).await?
      && enabled.iter().any(|one| one.id == id)
      && !heads.contains(&id)
    {
      heads.push(id);
    }
  }

  let (mut first, rest): (Vec<Account>, Vec<Account>) =
    enabled.into_iter().partition(|one| heads.contains(&one.id));

  first.sort_by_key(|one| heads.iter().position(|head| *head == one.id));

  Ok(first.into_iter().chain(rest).collect())
}

/// Whether a login answers now: it is on, no window of its own is full, and its login has not gone. This is
/// the one judgement every run of every line is offered a login by.
async fn open_now(judging: Judging<'_>, account: &Account, now: i64) -> Result<bool, Refused> {
  Ok(
    account.enabled
      && !resting(judging, &account.id, now).await?
      && !needs_login(judging.db, account, now).await?,
  )
}

/// The logins the road offers a run pinned to none, in order: the login in use first while it can
/// answer, then the open ones in the owner's order, started on the account the owner chose where one
/// was chosen. A login that cannot answer is never offered, so where none is open none is.
pub async fn open_accounts(judging: Judging<'_>) -> Result<Vec<Account>, Refused> {
  let now = now_ms();
  let mut open = Vec::new();

  for account in road_order(judging.db).await? {
    if open_now(judging, &account, now).await? {
      open.push(account);
    }
  }

  Ok(open)
}

/// The account the owner chose to run on now, where the one chosen is on.
pub async fn chosen_account(db: &DatabaseConnection) -> Result<Option<Account>, Refused> {
  let Some(id) = setting(db, CHOSEN_KEY).await? else {
    return Ok(None);
  };

  Ok(accounts(db).await?.into_iter().find(|one| one.enabled && one.id == id))
}

/// The user who administers the platform, where the multi-user model names one.
async fn administrator(db: &DatabaseConnection) -> Result<Option<i64>, Refused> {
  Ok(setting(db, "users_administrator").await?.and_then(|value| leading_integer(&value)))
}

/// The logins a work key's run made for the platform's own work is offered, in order: the one login
/// the key is pinned to where it is pinned to one that is on, the open walk otherwise; neither offers
/// a login another user owns.
pub async fn walk_for(judging: Judging<'_>, key: &str) -> Result<Vec<Account>, Refused> {
  walk_for_user(judging, key, None).await
}

/// The login a work key is pinned to, where it is pinned to one that is on.
pub async fn pinned_account(db: &DatabaseConnection, key: &str) -> Result<Option<Account>, Refused> {
  let pinned = one(db, "SELECT account FROM fallback_routes WHERE key = $1", vec![key.into()])
    .await?
    .map(|row| row.try_get::<Option<String>>("", "account"))
    .transpose()?
    .flatten();

  let Some(pinned) = pinned else { return Ok(None) };

  Ok(accounts(db).await?.into_iter().find(|one| one.enabled && one.id == pinned))
}

/// The logins a work key's run may go out on for the user it is made for, whether or not they answer now,
/// in the order the road takes them: that user's own logins alone, and for no user the platform's own
/// work's. A key pinned to one login that is on may go out on
/// that one and no other.
pub async fn carried_for_user(
  db: &DatabaseConnection,
  key: &str,
  user: Option<i64>,
) -> Result<Vec<Account>, Refused> {
  let administrator = if user.is_none() { administrator(db).await? } else { None };
  let carries = |account: &Account| match user {
    Some(user) => account.user_id == Some(user),
    None => account.user_id.is_none() || administrator.is_none() || account.user_id == administrator,
  };

  if let Some(pinned) = pinned_account(db, key).await? {
    return Ok(if carries(&pinned) { vec![pinned] } else { vec![] });
  }

  Ok(road_order(db).await?.into_iter().filter(|one| carries(one)).collect())
}

/// The logins a work key's run is offered for the user it is made for: of the logins it may go out on,
/// those that answer now. A key pinned to a login that cannot answer is offered none.
pub async fn walk_for_user(
  judging: Judging<'_>,
  key: &str,
  user: Option<i64>,
) -> Result<Vec<Account>, Refused> {
  let now = now_ms();
  let mut open = Vec::new();

  for account in carried_for_user(judging.db, key, user).await? {
    if open_now(judging, &account, now).await? {
      open.push(account);
    }
  }

  Ok(open)
}

/// The moment, in epoch seconds, the first of these logins that rests on its limit answers again; none
/// where none of them rests, every one of them waiting for a person instead.
pub async fn reopens_at(judging: Judging<'_>, closed: &[Account]) -> Result<Option<i64>, Refused> {
  let now = now_ms().div_euclid(1000);
  let mut first = None;

  for account in closed {
    if let Some(until) = rest_until(judging, &account.id).await?.filter(|until| *until > now) {
      first = Some(first.map_or(until, |first: i64| first.min(until)));
    }
  }

  Ok(first)
}

/// Whether a login answers now: it is on, its own window is not full, and its login has not gone.
pub async fn account_open(judging: Judging<'_>, id: &str, now: i64) -> Result<bool, Refused> {
  let Some(account) = accounts(judging.db).await?.into_iter().find(|one| one.id == id) else {
    return Ok(false);
  };

  open_now(judging, &account, now).await
}

/// The login the road's runs go out on now, where one is held and is still on.
pub async fn in_use(db: &DatabaseConnection) -> Result<Option<String>, Refused> {
  let enabled = accounts(db).await?;

  Ok(
    setting(db, IN_USE_KEY)
      .await?
      .filter(|id| enabled.iter().any(|one| one.enabled && &one.id == id)),
  )
}

/// That the road's runs go out on this login now, written only where it moved.
pub async fn hold_in_use(db: &DatabaseConnection, id: &str) -> Result<(), Refused> {
  if setting(db, IN_USE_KEY).await?.as_deref() != Some(id) {
    set_setting(db, IN_USE_KEY, id).await?;
  }

  Ok(())
}

// Whether a credential file holds something to renew itself with: a name that speaks of refreshing
// with a value under it, which is never read.
fn renews(value: &Value) -> bool {
  match value {
    Value::Object(fields) => fields.iter().any(|(name, held)| {
      (name.to_lowercase().contains("refresh") && held.as_str().is_some_and(|text| !text.is_empty()))
        || renews(held)
    }),
    Value::Array(items) => items.iter().any(renews),
    _ => false,
  }
}

/// When a login ends in the only sense worth telling the owner: nothing where its file holds a refresh,
/// since the CLI renews that login for itself; the moment its credential states otherwise.
pub fn login_end(account: &Account) -> Option<f64> {
  let renewing = account.credential_kind != "token"
    && std::fs::read_to_string(account.credential_file())
      .ok()
      .and_then(|text| serde_json::from_str::<Value>(&text).ok())
      .is_some_and(|held| renews(&held));

  if renewing { None } else { credential_expiry(account) }
}

// How long before its end a login is told as ending.
fn notice_ahead_ms() -> f64 {
  CLAUDE_CODE.login.notice_ahead_hours as f64 * 60.0 * 60_000.0
}

/// How a login stands: gone where it waits for a person or is past its end, ending inside the notice
/// window, standing otherwise, a login whose end nothing can read included.
pub fn login_standing(account: &Account, now: i64) -> &'static str {
  if account.needs_login_at.is_some() {
    return "gone";
  }

  match login_end(account) {
    None => "standing",
    Some(end) if end <= now as f64 => "gone",
    Some(end) if end - now as f64 <= notice_ahead_ms() => "ending",
    Some(_) => "standing",
  }
}

/// The accounts as the Claude road walks them, each with how its login stands, whether the owner
/// chose it, whether automatic runs go out on it now and how long a limit rest keeps it out of the
/// walk. The path is the whole of what is shown of a credential.
pub async fn views(judging: Judging<'_>) -> Result<Vec<Value>, Refused> {
  let now = now_ms();
  let chosen = chosen_account(judging.db).await?.map(|one| one.id);
  let serving = open_accounts(judging).await?.first().map(|one| one.id.clone());
  let mut views = Vec::new();

  for account in accounts(judging.db).await? {
    let rest = rest_until(judging, &account.id).await?.filter(|until| until * 1000 > now);

    views.push(json!({
      "id": account.id,
      "name": account.name,
      "place": account.place,
      "enabled": account.enabled,
      "credentialKind": account.credential_kind,
      "credentialPath": account.credential_path,
      "login": login_standing(&account, now),
      "loginEndsAt": login_end(&account).map(|end| (end / 1000.0).floor() as i64),
      "chosen": chosen.as_deref() == Some(account.id.as_str()),
      "serving": serving.as_deref() == Some(account.id.as_str()),
      "restUntil": rest,
    }));
  }

  Ok(views)
}

/// The directory whose catalogue answers: the records directory of the login the walk would take
/// first, or of the first login that is on where none answers now, or the machine's where no account is
/// registered (docs/model-tiers.md, 「The Claude catalogue」).
pub async fn catalogue_dir(judging: Judging<'_>, machine: &Machine) -> Result<PathBuf, Refused> {
  let open = open_accounts(judging).await?;
  let first = match open.into_iter().next() {
    Some(account) => Some(account),
    None => road_order(judging.db).await?.into_iter().next(),
  };

  Ok(first.map_or_else(|| machine_config_dir(machine), |account| account.records_dir(machine)))
}
