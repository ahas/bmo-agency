//! Who may read or change the agents a console manages (docs/api.md, 「The control port」): a control route
//! of the supervisors, the roles, the repositories, the registered models or the model files, asked by a
//! signed-in person the console names, goes on only where the console's permission for that person lets it:
//! each kind stands under the console section of the agent management tab that holds it, and the agent list,
//! which the console's screen reads under any one of its parts, is read under any of the five.
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

/// One of the five parts of agent management: a kind of route, and the console section of the tab that holds
/// it (the web's `docs/accounts.md`, 「Console sections」).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
  Supervisors,
  Roles,
  Repositories,
  Models,
  ModelFiles,
}

impl Part {
  const ALL: [Self; 5] =
    [Self::Supervisors, Self::Roles, Self::Repositories, Self::Models, Self::ModelFiles];

  /// The path the part's routes stand under.
  fn root(self) -> &'static str {
    match self {
      Self::Supervisors => "/supervisors",
      Self::Roles => "/roles",
      Self::Repositories => "/repositories",
      Self::Models => "/models",
      Self::ModelFiles => "/model-catalogue",
    }
  }

  /// The console section the part stands under.
  fn section(self) -> &'static str {
    match self {
      Self::Supervisors => "supervisors",
      Self::Roles => "roles",
      Self::Repositories => "repositories",
      Self::Models => "models",
      Self::ModelFiles => "model-files",
    }
  }

  /// The part alone, as the parts a change of it is allowed under.
  fn alone(self) -> &'static [Self] {
    match self {
      Self::Supervisors => &[Self::Supervisors],
      Self::Roles => &[Self::Roles],
      Self::Repositories => &[Self::Repositories],
      Self::Models => &[Self::Models],
      Self::ModelFiles => &[Self::ModelFiles],
    }
  }

  /// The parts whose read lets a person read this one: the agent list is read under any part, since the
  /// console's screen opens on any one of them and an agent's page stands under the three it is made of.
  fn read_under(self) -> &'static [Self] {
    match self {
      Self::Supervisors | Self::Roles | Self::Repositories => &Self::ALL,
      Self::Models | Self::ModelFiles => self.alone(),
    }
  }

  /// The part a path stands under, none for a route of another kind.
  fn of(path: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|part| {
      let root = part.root();

      path == root || path.starts_with(root) && path.as_bytes().get(root.len()) == Some(&b'/')
    })
  }
}

/// The account service is waited on for its answer: the local service answers at once, and a call that cannot
/// be made is no access, never a guessed one.
const WAIT: Duration = Duration::from_secs(10);

/// One managed route a member asks: whose permission it needs, which way, and the account service to ask.
pub struct Check {
  key: String,
  data_root: PathBuf,
  part: Part,
  write: bool,
}

/// The check a call needs, or none where it needs none: a route of another kind, a caller carrying no key, or
/// the owner.
pub fn asked(call: &Call, method: &Method, path: &str) -> Option<Check> {
  let part = Part::of(path)?;
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
    part,
    write: !matches!(method, &Method::GET | &Method::HEAD),
  })
}

/// Whom a call on a person's own supply acts for (docs/api.md, 「Who a caller acts for」): a person the console
/// names as a member, on their own accounts and keys alone, or the platform — the administrator, or the
/// deployment's own caller carrying no key — on the platform's accounts and of each person's supply only the
/// roads it serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acting {
  Person(i64),
  Platform,
}

impl Acting {
  /// Whom the call acts for, or the refusal of a member's key that names no user id.
  pub fn of(call: &Call) -> Result<Self, Refused> {
    let Some(key) = call.header("owner-key").map(|key| key.trim().to_owned()).filter(|key| !key.is_empty())
    else {
      return Ok(Self::Platform);
    };

    if call
      .header("owner-role")
      .is_some_and(|role| role.trim().eq_ignore_ascii_case("administrator"))
    {
      return Ok(Self::Platform);
    }

    key.parse::<i64>().map(Self::Person).map_err(|_| {
      Refused::new(StatusCode::FORBIDDEN, "The signed-in person is named by no user id agency reads")
        .with_code("forbidden")
    })
  }

  /// The person the call acts for, none for the platform.
  pub fn person(self) -> Option<i64> {
    match self {
      Self::Person(user) => Some(user),
      Self::Platform => None,
    }
  }
}

/// Whether the caller may make this call, or the refusal saying why it may not.
pub async fn check(check: Check) -> Result<(), Refused> {
  if access_of(&check).await? {
    return Ok(());
  }

  let what = if check.write { "change" } else { "read" };
  let access = if check.write { "write" } else { "read" };
  let sections = match check.sections() {
    [one] => format!("the {} section", one.section()),
    _ => "any section of agent management".to_owned(),
  };

  Err(Refused::new(
    StatusCode::FORBIDDEN,
    format!(
      "The signed-in person may not {what} {}: the console holds no {access} access to {sections} for them",
      check.part.root()
    ),
  ))
}

impl Check {
  /// The parts any one of which lets the call go on: a change needs its own part's write, a read the read of a
  /// part it stands under.
  fn sections(&self) -> &'static [Part] {
    if self.write { self.part.alone() } else { self.part.read_under() }
  }
}

/// Whether the account service says the person may use a section the call stands under for what it asks: the
/// write for a change, the read for a read.
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

  Ok(sections_allow(&read, check.sections(), check.write))
}

/// Whether a section of the answer among the parts named reads and, where a change asks, writes.
fn sections_allow(body: &Value, parts: &[Part], write: bool) -> bool {
  let access = if write { "write" } else { "read" };

  body.get("sections").and_then(Value::as_array).is_some_and(|rows| {
    rows.iter().any(|row| {
      let named = row.get("section").and_then(Value::as_str);

      parts.iter().any(|part| named == Some(part.section()))
        && row.get(access).and_then(Value::as_bool).unwrap_or(false)
    })
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
