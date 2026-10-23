//! Each registered Claude account's own windows, asked of the account-usage endpoint with that account's
//! own login (docs/usage-windows.md, 「The Claude account reader」), so an account nothing has run on lately
//! still has its own figures and never another's.
//!
//! The token is only ever in the request: read out of the account's own credential file, put in the one
//! header that asks, and dropped — never kept, returned, said or renewed. Renewing a login is the CLI's; a
//! token found expired is left for the CLI, and the account is marked unread so its last reading stops
//! standing until a reading lifts the mark.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use policy::providers::claude_code::CLAUDE_CODE;
use policy::rest::REST;
use serde_json::Value;

use crate::reader::{Passing, Reader, Said};
use crate::windows::{Keeping, Slot, Window};

/// The provider the windows are kept under.
pub const MODE: &str = "claude-code";

/// The window held against the rest's own ceiling; every longer one is held against the weekly one.
pub const FIVE_HOUR_WINDOW: &str = "five_hour";

// The failures that say the login's token cannot be read with until a run renews it: one found expired in
// its file, and one the endpoint refused as unauthorised.
const TOKEN_EXPIRED: &str = "token expired";
const TOKEN_REFUSED: &str = "HTTP 401";

/// Where a login's token is read from: a directory account's credential file, which holds it beside its
/// expiry, or a token account's file, which is the token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Credential {
  Directory(PathBuf),
  Token(PathBuf),
}

/// One registered account: the id its windows are kept under and where its token is read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Login {
  pub account: String,
  pub credential: Credential,
}

/// The registered accounts, on or off, as the caller that keeps them answers on each pass.
pub trait Logins: Send + Sync {
  fn logins(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Login>, String>> + Send + '_>>;
}

/// The share past which a window of this name is full: the five-hour window against the rest's own
/// ceiling and every longer one against the weekly one.
pub fn window_ceiling(name: &str) -> f64 {
  if name == FIVE_HOUR_WINDOW { REST.threshold } else { REST.weekly_threshold }
}

/// The windows an answer states, each as the fraction used and the moment it resets. A window with no
/// figure says nothing and is left out rather than written as nought.
pub fn windows_of(answer: &Value) -> Vec<Window> {
  CLAUDE_CODE
    .usage
    .windows
    .iter()
    .filter_map(|name| {
      let window = answer.get(name).filter(|window| window.is_object())?;
      let used = window.get("utilization").and_then(Value::as_f64)?;
      let resets_at = window
        .get("resets_at")
        .and_then(Value::as_str)
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .map(|at| at.timestamp());

      Some(Window {
        name: name.clone(),
        utilization: used / 100.0,
        resets_at,
        duration_minutes: None,
        limit_name: None,
      })
    })
    .collect()
}

// The access token a login holds now, or why there is none to ask with.
fn access_token(credential: &Credential, now_ms: i64) -> Result<String, &'static str> {
  let (Credential::Directory(file) | Credential::Token(file)) = credential;
  let text = std::fs::read_to_string(file).map_err(|_| "credential file unreadable")?;

  if let Credential::Token(_) = credential {
    let token = text.trim();

    return if token.is_empty() { Err("no token") } else { Ok(token.to_owned()) };
  }

  let held: Value = serde_json::from_str(&text).map_err(|_| "credential file unreadable")?;
  let login = held.get("claudeAiOauth");
  let token = login
    .and_then(|login| login.get("accessToken"))
    .and_then(Value::as_str)
    .filter(|token| !token.is_empty())
    .ok_or("no token")?;

  if login
    .and_then(|login| login.get("expiresAt"))
    .and_then(Value::as_f64)
    .is_some_and(|expires| expires <= now_ms as f64)
  {
    return Err(TOKEN_EXPIRED);
  }

  Ok(token.to_owned())
}

// The client every ask goes out on: a request that has not answered within `wait` fails.
fn asking_within(wait: Duration) -> reqwest::Client {
  reqwest::Client::builder().timeout(wait).build().unwrap_or_default()
}

/// The reader on the clock: where it asks, who it asks for, and each account's failure as last said.
pub struct ClaudeAccounts {
  url: String,
  http: reqwest::Client,
  logins: Arc<dyn Logins>,
  said: Said,
}

impl ClaudeAccounts {
  /// The reader asking `url` for every login `logins` answers.
  pub fn new(url: impl Into<String>, logins: Arc<dyn Logins>) -> Self {
    Self {
      url: url.into(),
      http: asking_within(Duration::from_secs(CLAUDE_CODE.usage.ask_seconds)),
      logins,
      said: Said::default(),
    }
  }

  /// The same reader taking a request that has not answered within `wait` as a failure, where a caller
  /// stands in an endpoint of its own.
  pub fn waiting(mut self, wait: Duration) -> Self {
    self.http = asking_within(wait);
    self
  }

  /// The reader asking the provider's own account-usage endpoint.
  pub fn of_provider(logins: Arc<dyn Logins>) -> Self {
    Self::new(CLAUDE_CODE.usage.url.as_str(), logins)
  }

  async fn ask(&self, token: &str) -> Result<Vec<Window>, String> {
    let answer = self.http.get(&self.url).bearer_auth(token).send().await.map_err(|why| {
      if why.is_timeout() {
        "the endpoint did not answer in time".to_owned()
      } else {
        "the request failed".to_owned()
      }
    })?;

    if !answer.status().is_success() {
      return Err(format!("HTTP {}", answer.status().as_u16()));
    }

    let read: Value = answer.json().await.map_err(|_| "the answer is not JSON".to_owned())?;

    Ok(windows_of(&read))
  }

  // The windows the account last read full, not yet reset at `now`, that this reading finds below their
  // ceiling: a limit lifted early, by a quota reset or the provider itself.
  async fn came_back_early(
    keeping: &Keeping,
    account: &str,
    windows: &[Window],
    now: i64,
  ) -> Result<Vec<String>, String> {
    let held = keeping
      .read(Slot {
        mode: MODE,
        account,
      })
      .await
      .map_err(|fault| fault.detail)?;

    Ok(
      held
        .windows
        .iter()
        .filter(|kept| {
          let ceiling = window_ceiling(&kept.window.name);

          kept.window.utilization >= ceiling
            && kept.window.resets_at.is_some_and(|reset| reset > now)
            && windows
              .iter()
              .any(|window| window.name == kept.window.name && window.utilization < ceiling)
        })
        .map(|kept| kept.window.name.clone())
        .collect(),
    )
  }

  // One account read and its windows written; why not where they were not. A token that cannot be read
  // with marks the account so its last reading stops standing, and a reading taken lifts the mark.
  async fn read(&self, keeping: &Keeping, login: &Login, now_ms: i64) -> Result<Vec<Window>, String> {
    let now = now_ms.div_euclid(1000);
    let account = login.account.as_str();
    let asked = match access_token(&login.credential, now_ms) {
      Ok(token) => self.ask(&token).await,
      Err(why) => Err(why.to_owned()),
    };
    let windows = match asked {
      Ok(windows) if windows.is_empty() => return Err("no usable windows".to_owned()),
      Ok(windows) => windows,
      Err(failure) => {
        if failure == TOKEN_EXPIRED || failure == TOKEN_REFUSED {
          keeping.mark_unread(account, now).await.map_err(|fault| fault.detail)?;
        }

        return Err(failure);
      }
    };
    let early = Self::came_back_early(keeping, account, &windows, now).await?;

    keeping
      .replace(
        Slot {
          mode: MODE,
          account,
        },
        &windows,
        now,
      )
      .await
      .map_err(|fault| fault.detail)?;
    keeping.lift_unread(account).await.map_err(|fault| fault.detail)?;

    for name in early {
      println!(
        "claude usage for {account}: its {name} window came back before the reset its last reading named"
      );
    }

    Ok(windows)
  }

  /// Every registered account asked once, on or off, each reading replacing that account's previous
  /// windows and nobody else's, and each account answered with the windows it was written with or why
  /// nothing was written; an account that could not be asked keeps what it had.
  pub async fn read_each(
    &self,
    keeping: &Keeping,
    now_ms: i64,
  ) -> Result<Vec<(String, Result<Vec<Window>, String>)>, String> {
    let mut outcomes = Vec::new();

    for login in self.logins.logins().await? {
      let outcome = self.read(keeping, &login, now_ms).await;

      outcomes.push((login.account, outcome));
    }

    Ok(outcomes)
  }
}

impl Reader for ClaudeAccounts {
  fn mode(&self) -> &str {
    MODE
  }

  fn cadence(&self) -> Duration {
    Duration::from_secs(CLAUDE_CODE.usage.pass_seconds)
  }

  fn fresh(&self) -> Duration {
    self.cadence() * CLAUDE_CODE.usage.fresh_passes
  }

  fn pass<'a>(&'a self, keeping: &'a Keeping, now_ms: i64) -> Passing<'a> {
    Box::pin(async move {
      match self.read_each(keeping, now_ms).await {
        Ok(outcomes) => {
          self.said.outcome("the claude accounts", &Ok(()));

          for (account, outcome) in outcomes {
            self.said.outcome(&format!("claude usage for {account}"), &outcome.map(drop));
          }
        }
        Err(why) => {
          self.said.outcome("the claude accounts", &Err(why));
        }
      }
    })
  }
}
