//! Agency's own settings that a person's organisation may also set (docs/api.md, 「settings」;
//! docs/prompt-layers.md, 「The settings agency declares」): the research supervisor, the minutes between two
//! flushes of the deploy queue, and the two DeepSeek secret names. The platform's word on each is the row of
//! agency's settings table, written through agency's own routes; the research supervisor stands for a person too,
//! where the service in front wrote that person's value into the subject settings. Where neither says one, the
//! policy's default stands. Nothing is asked of another service.

use policy::organisation::ORGANISATION;
use serde_json::Value;

use crate::places::Ground;

/// One setting that stands on the chain: its kind, the settings row it moved off, and whether its value is an
/// amount rather than a text.
pub struct Standing {
  pub key: &'static str,
  pub row: &'static str,
  pub amount: bool,
}

/// The four settings the chain holds, in the order their rows are seeded.
pub fn standing() -> [Standing; 4] {
  let settings = &ORGANISATION.settings;

  [
    Standing {
      key: &settings.research_supervisor.key,
      row: crate::lines::token::RESEARCH_SUPERVISOR,
      amount: false,
    },
    Standing {
      key: &settings.deploy_flush_minutes.key,
      row: crate::watches::deploy_flush::MINUTES_KEY,
      amount: true,
    },
    Standing {
      key: &settings.deepseek_card_secret.key,
      row: "deepseek_card_secret",
      amount: false,
    },
    Standing {
      key: &settings.deepseek_login_secret.key,
      row: "deepseek_login_secret",
      amount: false,
    },
  ]
}

/// The administrator's account, which writes the platform's links; none where the deployment names none.
pub async fn administrator(ground: &Ground) -> Result<Option<i64>, String> {
  crate::account_isolation::administrator(&ground.db)
    .await
    .map(|held| held.and_then(|text| text.trim().parse::<i64>().ok()))
    .map_err(|why| why.to_string())
}

// A link's value that says something: a text with letters in it, a number or a flag.
fn said(value: &Value) -> Option<Value> {
  match value {
    Value::Null => None,
    Value::String(text) if text.trim().is_empty() => None,
    held => Some(held.clone()),
  }
}

/// The platform's word on a standing: the settings row it stands under, where that says one.
pub async fn platform(ground: &Ground, key: &str) -> Result<Option<Value>, String> {
  let Some(standing) = of(key) else { return Ok(None) };
  let held = crate::queue::settings::get(&ground.db, standing.row).await.map_err(|why| why.to_string())?;

  Ok(held.filter(|text| !text.trim().is_empty()).map(|text| match standing.amount {
    true => text.trim().parse::<i64>().map(Value::from).unwrap_or(Value::String(text)),
    false => Value::String(text),
  }))
}

/// The word the chain gives the person for a standing that reaches them: the nearest link of their path where
/// it says one, else the platform's word. None is no answer of the chain, which leaves the row and the default
/// to stand.
pub async fn for_person(ground: &Ground, key: &str, person: i64) -> Result<Option<String>, String> {
  let answer = crate::chain::resolve(&ground.db, key, crate::chain::Subject::Person(person)).await?;

  Ok(answer.as_ref().and_then(|answer| said(&answer["value"])).map(as_text))
}

// A chain value as the text its consumer keeps.
fn as_text(value: Value) -> String {
  match value {
    Value::String(text) => text,
    Value::Number(number) => number.to_string(),
    Value::Bool(on) => on.to_string(),
    held => held.to_string(),
  }
}

/// The platform's text word on a standing, none where its row says none.
pub async fn text(ground: &Ground, standing: &Standing) -> Result<Option<String>, String> {
  Ok(platform(ground, standing.key).await?.map(as_text))
}

/// The platform's word on a standing written into its settings row. Null gives the value back to the kind's
/// default.
pub async fn write(ground: &Ground, standing: &Standing, value: Value) -> Result<(), String> {
  write_row(ground, standing, value).await
}

// The settings row of a standing, written or given back.
async fn write_row(ground: &Ground, standing: &Standing, value: Value) -> Result<(), String> {
  let result = match value {
    Value::Null => crate::queue::settings::forget(&ground.db, standing.row).await,
    Value::String(text) => crate::queue::settings::set(&ground.db, standing.row, &text).await,
    Value::Number(number) => {
      crate::queue::settings::set(&ground.db, standing.row, &number.to_string()).await
    }
    _ => return Err("a standing is written as a text, an amount, or null".to_owned()),
  };

  result.map_err(|why| why.to_string())
}


/// The standing of one kind, none where the kind is not one the chain holds.
pub fn of(kind: &str) -> Option<Standing> {
  standing().into_iter().find(|one| one.key == kind)
}
