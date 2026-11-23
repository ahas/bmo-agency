//! The supply sources a person's delegation chain leaves open (docs/supply.md, 「The sources a person's chain
//! leaves open」): their own login or key, their organisation's own supply and the platform's, each a flag that
//! narrows down the chain, read afresh for every run. The order a run walks keeps that fixed shape — personal
//! supply, then the organisation's, then the platform's — and a source the chain closes is left out of it.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use policy::organisation::ORGANISATION;
use serde_json::Value;

use super::{PERSONAL, organisation_word};
use crate::chain::{self, Subject};
use crate::places::Ground;

/// What a person's chain leaves open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
  /// Their own login or key.
  pub personal: bool,
  /// The platform's supply, every platform source the hook names.
  pub platform: bool,
  /// The units on their path whose own supply they may take, the nearest first; none where organisation
  /// supply stands closed for them.
  pub units: Vec<i64>,
}

impl Default for Open {
  /// What stands where the chain says nothing: personal and platform supply open, and no organisation's.
  fn default() -> Self {
    Self {
      personal: true,
      platform: true,
      units: Vec::new(),
    }
  }
}

// The last sources this process read off the chain for each account, which a run stands on while the account
// service does not answer.
static LAST_READ: LazyLock<Mutex<HashMap<i64, Open>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether organisation supply stands open on a resolved answer: the platform's own word says it is open for the
/// subject — its value, or a rule naming them, their group or a unit on their path — and no link below closes it.
/// A unit's word alone never opens it, so only the platform opens it for a company.
pub fn organisation_open(answer: Option<&Value>) -> bool {
  let Some(answer) = answer else { return false };
  let platform_says = answer["layers"]
    .as_array()
    .into_iter()
    .flatten()
    .any(|layer| layer["link"] == "platform" && layer["value"] == true);

  platform_says && chain::flag(Some(answer), false)
}

/// The units of a resolved subject's path, the nearest first.
pub fn path_units(answer: Option<&Value>) -> Vec<i64> {
  let mut units: Vec<i64> = answer
    .and_then(|answer| answer["path"].as_array())
    .into_iter()
    .flatten()
    .filter_map(|link| link.as_str()?.strip_prefix("unit:")?.parse().ok())
    .collect();

  units.reverse();
  units
}

/// What the chain leaves open for a subject, read afresh; an error where the account service did not answer.
/// Where it says nothing — no account service named, a subject it does not hold — the default stands.
pub async fn read(
  service: &tool_call::room_gate::AccountService,
  subject: Subject,
) -> Result<Open, String> {
  let kinds = &ORGANISATION.sources;
  let (personal, platform, organisation) = futures_util::join!(
    chain::resolve(service, &kinds.personal.key, subject),
    chain::resolve(service, &kinds.platform.key, subject),
    chain::resolve(service, &kinds.organisation.key, subject),
  );
  let (personal, platform, organisation) = (personal?, platform?, organisation?);
  let units = if organisation_open(organisation.as_ref()) {
    path_units(organisation.as_ref())
  } else {
    Vec::new()
  };

  Ok(Open {
    personal: chain::flag(personal.as_ref(), true),
    platform: chain::flag(platform.as_ref(), true),
    units,
  })
}

/// What a person's chain leaves open, read afresh; where the account service does not answer, what this process
/// last read for them, else the default.
pub async fn open_for(ground: &Ground, account: i64) -> Open {
  match read(&crate::apps::service(ground), Subject::Person(account)).await {
    Ok(open) => {
      if let Ok(mut kept) = LAST_READ.lock() {
        kept.insert(account, open.clone());
      }

      open
    }
    Err(why) => {
      eprintln!("agency: the supply sources of account {account} were not read: {why}");
      LAST_READ.lock().ok().and_then(|kept| kept.get(&account).cloned()).unwrap_or_default()
    }
  }
}

/// The order a run walks, shaped by what the chain leaves open: personal supply left out where it is closed;
/// every platform source left out where the platform's is closed; and the organisation's own supply, where it
/// stands open on some unit, before every platform source, so after personal supply wherever the order puts that
/// first. Nothing else of the hook's order moves.
pub fn shaped(order: &[String], open: &Open) -> Vec<String> {
  let organisation = organisation_word();
  let mut shaped: Vec<String> = order
    .iter()
    .filter(|source| source.as_str() != organisation)
    .filter(|source| if source.as_str() == PERSONAL { open.personal } else { open.platform })
    .cloned()
    .collect();

  if !open.units.is_empty() {
    let at = shaped.iter().position(|source| source != PERSONAL).unwrap_or(shaped.len());

    shaped.insert(at, organisation.to_owned());
  }

  shaped
}
