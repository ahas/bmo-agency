#![allow(
  dead_code,
  unused_imports,
  unused_variables,
  clippy::needless_lifetimes,
  clippy::let_and_return,
  clippy::unnecessary_to_owned
)]
//! The Claude logins as a job's runner, the job unit and the console ask after them (docs/api.md,
//! 「Accounts」): the rest a done job's session usage calls for, whether a user holds a login that is on with
//! each login's own two headline windows, and where the logins keep their session records.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use store::one;

use super::claude_accounts::{Judging, accounts, machine_config_dir};
use super::fallback::{account_balance, account_headlines};
use super::{Call, Refused, Route, answer};
use crate::door::record::SessionUsage;
use crate::now_ms;
use crate::rest::{DeploymentResting, RestKind, rest_until_after, write_rest};

fn bad(error: impl Into<String>) -> Refused {
  Refused::new(StatusCode::BAD_REQUEST, error)
}

/// A done job's session usage, as its answer reported it on the login.
#[derive(Deserialize)]
struct Used {
  usage: SessionUsage,
}

/// The rest a done job's session usage calls for, begun on the login it was read of.
pub struct UsageRest;

impl Route for UsageRest {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/accounts/{accountId}";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let account = call.param("accountId").unwrap_or_default().to_owned();
    let body = call.body().await?;
    let usage = body
      .get("usage")
      .cloned()
      .or_else(|| body.get("window").cloned())
      .unwrap_or(body.clone());
    let used: Used = serde_json::from_value(json!({ "usage": usage }))
      .map_err(|error| bad(format!("a session's usage: {error}")))?;
    let now = now_ms() / 1000;

    if account.is_empty() {
      return Err(bad("an account is required"));
    }

    let mut rest = None;

    if let Some(until) = rest_until_after(&used.usage, now) {
      let ground = call.ground();

      if write_rest(Judging::of(ground), until, RestKind::Limit, &account, now).await? {
        ground.events.publish(DeploymentResting::of(RestKind::Limit, &account, until as i64));
      }
      rest = Some(until as i64);
    }

    Ok(answer(
      StatusCode::OK,
      json!({
        "id": account,
        "provider": "claude-code",
        "userName": Value::Null,
        "held": true,
        "home": Value::Null,
        "window": body.get("window").cloned().unwrap_or(Value::Null),
        "restUntil": rest,
      }),
    ))
  }
}

crate::moved!(UsageRest);

/// Whether a user holds a login that is on.
pub struct AccountHeld;

impl Route for AccountHeld {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/accounts";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let machine = call.machine();
    let user = call.query("userName").or_else(|| call.query("user"));
    let mut items = Vec::new();
    let wanted = match user.as_deref() {
      Some(user) => match user.trim().parse::<i64>() {
        Ok(user) => Some(user),
        Err(_) => return Ok(answer(StatusCode::OK, json!({ "items": [], "more": false }))),
      },
      None => None,
    };

    let judging = Judging::of(call.ground());

    for account in accounts(call.store()).await? {
      let (session, week) = account_headlines(judging.windows, &account).await?;
      let balance = account_balance(judging.windows, &account).await?;
      let held = match wanted {
        Some(user) => account.user_id == Some(user) && account.enabled,
        None => account.enabled,
      };
      let (endpoint, key_file, models) = account.credential.endpoint_parts();

      items.push(json!({
        "id": account.id,
        "provider": "claude-code",
        "userName": account.user_id,
        "held": held,
        "home": account.records_dir(machine).to_string_lossy(),
        "window": Value::Null,
        "session": session,
        "week": week,
        "balance": balance,
        "restUntil": Value::Null,
        "credentialKind": account.credential.kind().word(),
        "endpointUrl": endpoint,
        "keyFile": key_file.map(|file| file.to_string_lossy()),
        "models": models,
        "basePrompt": account.base_prompt,
      }));
    }

    Ok(answer(StatusCode::OK, json!({ "items": items, "more": false })))
  }
}

crate::moved!(AccountHeld);

/// The directory each login keeps its session records under: this machine's own first, each once.
pub struct RecordHomes;

impl Route for RecordHomes {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/accounts/claude/homes";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let machine = call.machine();
    let mut homes = vec![machine_config_dir(machine).to_string_lossy().into_owned()];

    for account in accounts(call.store()).await? {
      let home = account.records_dir(machine).to_string_lossy().into_owned();

      if !homes.contains(&home) {
        homes.push(home);
      }
    }

    Ok(answer(StatusCode::OK, json!({ "homes": homes })))
  }
}
