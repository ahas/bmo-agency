//! Who may read or change the agents a console manages (docs/api.md, 「The control port」): a control route
//! of the supervisors, the roles, the repositories, the registered models or the model files, asked by a
//! signed-in person the console names, goes on only where the console's permission for that person's
//! `agents` section lets it.
//!
//! A caller carrying no `Owner-Key` is a unit, a tool, a job or the shell acting on the deployment's own
//! token, and keeps the routes' present behaviour. The platform administrator, `Owner-Role: administrator`,
//! is the owner and keeps every access. A member's permission is the console's, kept on the account service:
//! agency asks it, with that person's headers, and refuses where the answer gives no read or no write.

use std::path::PathBuf;
use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::Value;
use tool_call::room_gate::AccountService;

use crate::control::{Call, Refused};

/// The console's section the five kinds of agent management stand under (the web's `docs/accounts.md`,
/// 「Console sections」).
pub const SECTION: &str = "agents";

/// The account service is waited on for its answer: the local service answers at once, and a call that cannot
/// be made is no access, never a guessed one.
const WAIT: Duration = Duration::from_secs(10);

/// One managed route a member asks: whose permission it needs, which way, and the account service to ask.
pub struct Check {
  key: String,
  data_root: PathBuf,
  write: bool,
}

/// The check a call needs, or none where it needs none: a route of another kind, a caller carrying no key, or
/// the owner.
pub fn asked(call: &Call, method: &Method, path: &str) -> Option<Check> {
  if !managed(path) {
    return None;
  }

  let key = call
    .header("owner-key")
    .map(|key| key.trim().to_owned())
    .filter(|key| !key.is_empty())?;

  if call
    .header("owner-role")
    .is_some_and(|role| role.trim().eq_ignore_ascii_case("administrator"))
  {
    return None;
  }

  Some(Check {
    key,
    data_root: call.data_root(),
    write: !matches!(method, &Method::GET | &Method::HEAD),
  })
}

/// Whether a route is one of the five kinds agent management is read and changed through.
fn managed(path: &str) -> bool {
  let under =
    |root: &str| path == root || path.starts_with(root) && path.as_bytes().get(root.len()) == Some(&b'/');

  under("/supervisors")
    || under("/roles")
    || under("/repositories")
    || under("/models")
    || under("/model-catalogue")
}

/// Whether the caller may make this call, or the refusal saying why it may not.
pub async fn check(check: Check) -> Result<(), Refused> {
  if access_of(&check).await? {
    return Ok(());
  }

  let what = if check.write { "change" } else { "read" };
  let access = if check.write { "write" } else { "read" };

  Err(Refused::new(
    StatusCode::FORBIDDEN,
    format!(
      "The signed-in person may not {what} agent management: the console holds no {access} access to the {SECTION} section for them"
    ),
  ))
}

/// Whether the account service says the person may use the agents section for a read and for a write; the
/// write is what a change asks, and a read is what every call asks.
async fn access_of(check: &Check) -> Result<bool, Refused> {
  let accounts = AccountService::read(&check.data_root, |name| std::env::var(name).ok());

  if accounts.address.is_empty() {
    return Err(unchecked("the account service is not named"));
  }

  let asked = reqwest::Client::new()
    .get(format!("{}/users/{}/sections", accounts.address, check.key))
    .bearer_auth(&accounts.token)
    .header("Owner-Key", &check.key)
    .header("Owner-Role", "member")
    .timeout(WAIT)
    .send()
    .await
    .map_err(|why| unchecked(&format!("the account service was not reached: {why}")))?;
  let status = asked.status();
  let read = asked.json::<Value>().await.unwrap_or(Value::Null);

  if status == StatusCode::FORBIDDEN || status == StatusCode::UNAUTHORIZED {
    return Ok(false);
  }

  if !status.is_success() {
    let said = read.get("error").and_then(Value::as_str).unwrap_or("no reason given");

    return Err(unchecked(&format!("the account service answered {}: {said}", status.as_u16())));
  }

  Ok(section_allows(&read, check.write))
}

/// Whether one section of the answer reads and, where a change asks, writes.
fn section_allows(body: &Value, write: bool) -> bool {
  body
    .get("sections")
    .and_then(Value::as_array)
    .and_then(|sections| {
      sections.iter().find(|row| row.get("section").and_then(Value::as_str) == Some(SECTION))
    })
    .is_some_and(|row| {
      let allows = |name: &str| row.get(name).and_then(Value::as_bool).unwrap_or(false);

      if write { allows("write") } else { allows("read") }
    })
}

/// The refusal of a check that could not be made: no access is guessed.
fn unchecked(why: &str) -> Refused {
  Refused::new(
    StatusCode::SERVICE_UNAVAILABLE,
    format!("Agent management's access could not be checked: {why}"),
  )
  .with_code("access-unchecked")
}
