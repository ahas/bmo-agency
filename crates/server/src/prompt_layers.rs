//! The layers of a supervisor's system prompt that stand between its harness and its role, and the persona's
//! choice (docs/prompt-layers.md): the platform's rules, attached to the platform administrator's supervisors
//! alone; the organisation's instructions and the persona's choice, read off the account service's delegation
//! chain for the person a supervisor serves. Agency declares its settings on the chain as its server stands, and
//! again where a resolution finds one undeclared (`crate::chain`).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use harness_machine::supervisor_prompt::{Layers, organisation};
use policy::prompt_layers::{Kind, PROMPT_LAYERS};
use sea_orm::ConnectionTrait;
use serde_json::Value;
use tool_call::room_gate::AccountService;

/// The body the account service takes a kind's declaration in.
pub fn declaration(kind: &Kind) -> Value {
  crate::chain::declaration(kind)
}

/// Every kind agency declares, declared on the account service (`crate::chain::declare`).
pub async fn declare(service: &AccountService) -> Result<(), String> {
  crate::chain::declare(service).await
}

// The last layers this process read off the chain for each account, which a turn stands on while the account
// service does not answer.
static LAST_READ: LazyLock<Mutex<HashMap<i64, Layers>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

// One kind resolved for a person, the kinds declared again where it stood undeclared.
async fn resolved(service: &AccountService, key: &str, person: i64) -> Result<Option<Value>, String> {
  crate::chain::resolve(service, key, crate::chain::Subject::Person(person)).await
}

/// The organisation's instructions and the persona's choice the chain resolves for `person`, read afresh.
pub async fn read_chain(service: &AccountService, person: i64) -> Result<(Option<String>, bool), String> {
  let words = &PROMPT_LAYERS;
  let (instructions, persona) = futures_util::join!(
    resolved(service, &words.organisation.kind.key, person),
    resolved(service, &words.persona.kind.key, person),
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
