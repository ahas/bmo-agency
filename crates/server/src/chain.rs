//! The account service's delegation chain as agency asks it (the web's `docs/accounts.md`, 「The delegation
//! chain」): every setting agency declares, declared as the server stands and again where a resolution finds one
//! undeclared; a kind resolved for a person or a unit; and a link's entry read or a value written in the name of
//! the person a console call is made for, so the chain itself judges who may write it. Nothing is kept: every
//! consumer asks afresh as it acts.

use std::time::Duration;

pub use bmo_chain::{Answered, Subject, flag, path_of, units_of};
use bmo_chain::{Chain, Resolution};
use policy::organisation::ORGANISATION;
use policy::prompt_layers::{Kind, PROMPT_LAYERS};
use serde_json::{Value, json};
use tool_call::account_service::AccountService;

// The chain as agency asks it: the account service's address and token, agency named as the unit declaring its
// kinds (bmo-chat's `docs/chain.md`).
fn chain(service: &AccountService) -> Chain {
  Chain::new(
    &service.address,
    &service.token,
    &PROMPT_LAYERS.declared_by,
    Duration::from_secs(PROMPT_LAYERS.ask_seconds),
  )
}

/// Every kind agency declares: the prompt layers' and the organisation's.
pub fn kinds() -> Vec<Kind> {
  let organisation = &ORGANISATION;
  let own = |kind: &Kind| Kind {
    key: kind.key.clone(),
    description: kind.description.clone(),
    shape: kind.shape.clone(),
    choices: kind.choices.clone(),
    mode: kind.mode.clone(),
    rules: kind.rules,
    levels: kind.levels.clone(),
    unit_permission: kind.unit_permission.clone(),
    person_writer: kind.person_writer.clone(),
    default: kind.default.clone(),
  };

  let mut kinds = vec![
    own(&PROMPT_LAYERS.organisation.kind),
    own(&PROMPT_LAYERS.persona.kind),
    own(&PROMPT_LAYERS.personal.allowed),
    own(&PROMPT_LAYERS.personal.text),
    organisation.composition.kind(),
    own(&organisation.repositories),
    own(&organisation.personal_apps.make),
    own(&organisation.personal_apps.network),
    own(&organisation.personal_apps.secrets),
    own(&organisation.personal_apps.share_scope),
    own(&organisation.personal_apps.share_review),
    own(&organisation.personal_apps.install),
    own(&organisation.personal_local_ai),
    own(&organisation.own_agents.add),
    own(&organisation.own_agents.road),
    own(&organisation.resources),
    own(&organisation.settings.research_supervisor),
    own(&organisation.settings.deploy_flush_minutes),
    own(&organisation.settings.deepseek_card_secret),
    own(&organisation.settings.deepseek_login_secret),
  ];

  // One kind per provider this build carries, saying whether its sources may be gone out on under a link
  // (docs/supply.md, 「The sources a run walks」): a link turns a provider off below it, none turns it back on.
  kinds.extend(harness_providers::ARCHIVE_PROVIDERS.map(|code| Kind {
    key: format!("provider.{code}.offered"),
    description: format!(
      "Whether {code} may be gone out on under this link: a link turns the provider's sources, the platform's \
       and its units' and its people's, off for everyone below it; none turns it back on."
    ),
    shape: "flag".to_owned(),
    choices: Vec::new(),
    mode: "narrowing".to_owned(),
    rules: true,
    levels: vec!["platform".to_owned(), "unit".to_owned()],
    unit_permission: "manage".to_owned(),
    person_writer: "heads".to_owned(),
    default: Some(toml::Value::Boolean(true)),
  }));

  kinds
}

/// The body the account service takes a kind's declaration in.
pub fn declaration(kind: &Kind) -> Value {
  let mut body = json!({
    "key": kind.key,
    "description": kind.description,
    "shape": kind.shape,
    "choices": kind.choices,
    "mode": kind.mode,
    "rules": kind.rules,
    "levels": kind.levels,
    "unitPermission": kind.unit_permission,
    "personWriter": kind.person_writer,
  });

  if let Some(default) = &kind.default {
    body["default"] = serde_json::to_value(default).unwrap_or(Value::Null);
  }

  body
}

/// Every kind agency declares, declared on the account service, `PUT /chain/kinds/{key}`; the first refusal or
/// failure is the error. Nothing is asked where no account service is named.
pub async fn declare(service: &AccountService) -> Result<(), String> {
  let chain = chain(service);

  if !chain.named() {
    return Ok(());
  }

  chain.declare(&kinds().iter().map(declaration).collect::<Vec<_>>()).await
}

/// One kind resolved for a subject, every kind declared again and the kind asked once more where it stood
/// undeclared; a kind that stays undeclared, or a subject the service does not hold, says nothing. Where no
/// account service is named, nothing is said either.
pub async fn resolve(
  service: &AccountService,
  key: &str,
  subject: Subject,
) -> Result<Option<Value>, String> {
  let chain = chain(service);

  if !chain.named() {
    return Ok(None);
  }

  match chain.resolve(key, subject).await? {
    Resolution::Resolved(answer) => Ok(Some(answer)),
    Resolution::Nothing => Ok(None),
    Resolution::Undeclared => {
      if let Err(why) = declare(service).await {
        eprintln!("agency: its delegation chain kinds were not declared: {why}");
        return Ok(None);
      }

      match chain.resolve(key, subject).await? {
        Resolution::Resolved(answer) => Ok(Some(answer)),
        Resolution::Nothing | Resolution::Undeclared => Ok(None),
      }
    }
  }
}

/// A link's entry of a kind, read in the caller's name: the chain answers it to whoever writes the link and to a
/// person at their own, and refuses anyone else 403.
pub async fn entry(
  service: &AccountService,
  key: &str,
  link: &str,
  caller: i64,
) -> Result<Answered, String> {
  chain(service).entry(key, link, caller).await
}

/// A link's value written in the caller's name, null to follow the link above; the chain judges whether the caller
/// writes it, and whether a lock above withholds it. The call names agency as the unit that declared the kind, so
/// the chain takes a `json` value agency judged on its own route.
pub async fn write_value(
  service: &AccountService,
  key: &str,
  link: &str,
  caller: i64,
  value: Value,
) -> Result<Answered, String> {
  chain(service).write_value(key, link, caller, value).await
}

/// Whether the caller may write the unit's link of a kind now: the chain answers the entry to its writers alone and
/// says whether a lock above withholds it.
pub async fn writes_unit(
  service: &AccountService,
  key: &str,
  unit: i64,
  caller: i64,
) -> Result<bool, String> {
  let read = entry(service, key, &format!("unit:{unit}"), caller).await?;

  Ok(read.ok() && read.body["writable"].as_bool().unwrap_or(false))
}

/// A link's delegation written in the caller's name, `allow`, `lock` or `inherit`; the chain judges whether the caller
/// writes it.
pub async fn write_delegation(
  service: &AccountService,
  key: &str,
  link: &str,
  caller: i64,
  delegation: &str,
) -> Result<Answered, String> {
  chain(service).write_delegation(key, link, caller, delegation).await
}

/// A read of the account service at `path`, in the caller's name where one is named: a unit, its subtree or a group
/// of the chain.
pub async fn read(service: &AccountService, path: &str, caller: Option<i64>) -> Result<Answered, String> {
  chain(service).read(path, caller).await
}

/// A link's rules written whole in the caller's name, each `who` and `value`.
pub async fn write_rules(
  service: &AccountService,
  key: &str,
  link: &str,
  caller: i64,
  rules: Value,
) -> Result<Answered, String> {
  chain(service).write_rules(key, link, caller, rules).await
}
