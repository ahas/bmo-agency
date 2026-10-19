//! The Claude logins as a job's runner and the job unit ask after them (docs/api.md, 「Accounts」): the rest a
//! done job's session usage calls for, whether a user holds a login that is on, and where the logins keep
//! their session records.

use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use store::one;

use super::claude_accounts::{Judging, accounts, machine_config_dir};
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
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/accounts/claude/{account}/usage-rest";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let account = call.param("account").unwrap_or_default().to_owned();
    let used: Used = serde_json::from_value(call.body().await?)
      .map_err(|error| bad(format!("a session's usage: {error}")))?;
    let now = now_ms() / 1000;

    if account.is_empty() {
      return Err(bad("an account is required"));
    }

    if let Some(until) = rest_until_after(&used.usage, now) {
      let ground = call.ground();

      if write_rest(Judging::of(ground), until, RestKind::Limit, &account, now).await? {
        ground.events.publish(DeploymentResting::of(RestKind::Limit, &account, until as i64));
      }
    }

    Ok(StatusCode::NO_CONTENT.into_response())
  }
}

crate::moved!(UsageRest);

/// Whether a user holds a login that is on.
pub struct AccountHeld;

impl Route for AccountHeld {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/accounts/claude/held";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let user = call.query("user").ok_or_else(|| bad("user is required"))?;
    let held = match user.trim().parse::<i64>() {
      Err(_) => false,
      Ok(user) => one(
        call.store(),
        r#"SELECT 1 AS "held" FROM "claude_accounts" WHERE "enabled" = 1 AND "user_id" = $1 LIMIT 1"#,
        vec![user.into()],
      )
      .await?
      .is_some(),
    };

    Ok(answer(StatusCode::OK, json!({ "held": held })))
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

crate::moved!(RecordHomes);
