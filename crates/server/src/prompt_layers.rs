//! The layers of a supervisor's system prompt that stand between its harness and its role, and the persona's
//! choice (docs/prompt-layers.md): the platform's rules, attached to the platform administrator's supervisors
//! alone; the organisation's instructions and the persona's choice, read off the account service's delegation
//! chain for the person a supervisor serves. Agency declares both settings on the chain as its server stands,
//! and again where a resolution finds one undeclared.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use harness_machine::supervisor_prompt::{Layers, organisation};
use policy::prompt_layers::{Kind, PROMPT_LAYERS};
use reqwest::StatusCode;
use sea_orm::ConnectionTrait;
use serde_json::{Value, json};
use tool_call::room_gate::AccountService;

/// What the account service answers a kind's resolution for a person: what its links said, or that it said
/// nothing — the kind undeclared or the person not standing there.
enum Said {
  Resolved(Value),
  Undeclared,
  Nothing,
}

// The last layers this process read off the chain for each account, which a turn stands on while the account
// service does not answer.
static LAST_READ: LazyLock<Mutex<HashMap<i64, Layers>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn client() -> reqwest::Client {
  reqwest::Client::builder()
    .timeout(Duration::from_secs(PROMPT_LAYERS.ask_seconds))
    .build()
    .unwrap_or_default()
}

/// The body the account service takes a kind's declaration in.
pub fn declaration(kind: &Kind) -> Value {
  let mut body = json!({
    "key": kind.key,
    "declaredBy": PROMPT_LAYERS.declared_by,
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
  if service.address.is_empty() {
    return Ok(());
  }

  let client = client();

  for kind in PROMPT_LAYERS.kinds() {
    let answer = client
      .put(format!("{}/chain/kinds/{}", service.address, kind.key))
      .bearer_auth(&service.token)
      .json(&declaration(kind))
      .send()
      .await
      .map_err(|why| format!("the account service was not reached: {why}"))?;
    let status = answer.status();

    if !status.is_success() {
      let said = answer.text().await.unwrap_or_default();

      return Err(format!(
        "the account service refused the kind {} with {}: {said}",
        kind.key,
        status.as_u16()
      ));
    }
  }

  Ok(())
}

// One kind resolved for a person.
async fn said(
  client: &reqwest::Client,
  service: &AccountService,
  key: &str,
  person: i64,
) -> Result<Said, String> {
  let answer = client
    .get(format!("{}/chain/kinds/{key}/resolve", service.address))
    .bearer_auth(&service.token)
    .query(&[("person", person.to_string())])
    .send()
    .await
    .map_err(|why| format!("the account service was not reached: {why}"))?;
  let status = answer.status();

  if status == StatusCode::NOT_FOUND {
    let body: Value = answer.json().await.unwrap_or(Value::Null);

    return Ok(if body["code"] == "no-such-kind" { Said::Undeclared } else { Said::Nothing });
  }

  if !status.is_success() {
    return Err(format!("the account service answered {}", status.as_u16()));
  }

  answer
    .json()
    .await
    .map(Said::Resolved)
    .map_err(|why| format!("the account service's answer does not read: {why}"))
}

// One kind resolved for a person, the kinds declared again and the kind asked once more where it stood
// undeclared; a kind that stays undeclared says nothing.
async fn resolved(
  client: &reqwest::Client,
  service: &AccountService,
  key: &str,
  person: i64,
) -> Result<Option<Value>, String> {
  match said(client, service, key, person).await? {
    Said::Resolved(answer) => Ok(Some(answer)),
    Said::Nothing => Ok(None),
    Said::Undeclared => {
      if let Err(why) = declare(service).await {
        eprintln!("agency: the prompt layers' kinds were not declared: {why}");
        return Ok(None);
      }

      match said(client, service, key, person).await? {
        Said::Resolved(answer) => Ok(Some(answer)),
        Said::Nothing | Said::Undeclared => Ok(None),
      }
    }
  }
}

/// The organisation's instructions and the persona's choice the chain resolves for `person`, read afresh.
pub async fn read_chain(service: &AccountService, person: i64) -> Result<(Option<String>, bool), String> {
  let client = client();
  let words = &PROMPT_LAYERS;
  let (instructions, persona) = futures_util::join!(
    resolved(&client, service, &words.organisation.kind.key, person),
    resolved(&client, service, &words.persona.kind.key, person),
  );
  let texts: Vec<String> = instructions?
    .as_ref()
    .and_then(|answer| answer["layers"].as_array())
    .into_iter()
    .flatten()
    .filter_map(|layer| layer["value"].as_str().map(str::to_owned))
    .collect();
  let persona_none = persona?
    .as_ref()
    .and_then(|answer| answer["value"].as_array())
    .is_some_and(|keys| keys.iter().any(|key| key == words.persona.none.as_str()));

  Ok((organisation(&texts), persona_none))
}

/// The layers a supervisor owned by `owner` is composed with, or none for one of no account. The platform's
/// rules are attached to a supervisor of no account or of the platform administrator's account; a supervisor
/// of an account has its organisation's instructions and its persona's choice read off the chain for that
/// person. Where the account service is named and does not answer, the layers this process last read for the
/// account stand; with none read yet, the sentence of why is the error, so no turn goes out without its
/// organisation's instructions.
pub async fn of<C: ConnectionTrait>(
  db: &C,
  service: &AccountService,
  owner: Option<i64>,
) -> Result<Layers, String> {
  let Some(account) = owner else {
    return Ok(Layers {
      platform: true,
      ..Layers::default()
    });
  };
  let platform = crate::account_isolation::owners_own(db, &account.to_string())
    .await
    .map_err(|refusal| refusal.to_string())?;

  if service.address.is_empty() {
    return Ok(Layers {
      platform,
      ..Layers::default()
    });
  }

  match read_chain(service, account).await {
    Ok((organisation, persona_none)) => {
      let layers = Layers {
        platform,
        organisation,
        persona_none,
      };

      if let Ok(mut kept) = LAST_READ.lock() {
        kept.insert(account, layers.clone());
      }

      Ok(layers)
    }
    Err(why) => {
      let kept = LAST_READ.lock().ok().and_then(|kept| kept.get(&account).cloned());

      kept.ok_or_else(|| {
        format!("the organisation's instructions for account {account} could not be read: {why}")
      })
    }
  }
}
