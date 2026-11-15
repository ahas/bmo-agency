//! `deepseek_top_up`: the supervisor tops the DeepSeek balance up by an amount the owner names (docs/tools.md,
//! 「The DeepSeek top-up」). DeepSeek takes no payment by machine, so a call that passes every check answers
//! what a person does on DeepSeek's platform, and pays nothing. A card value is read only to see that it stands,
//! and is never kept, answered or written anywhere.

use std::collections::BTreeSet;
use std::path::Path;

use bmo_agency_client::{AgencyClient, ProviderQuota};
use chrono::{DateTime, SecondsFormat};
use harness_providers::Provider;
use harness_providers::deepseek::DeepSeek;
use policy::fill_marks;
use policy::providers::deepseek::{DEEPSEEK, TopUp};
use serde::Deserialize;
use serde_json::{Value, json};
use tool_call::{Answer, Asked, Control, Handed, Session};

/// The tool's namespace and entry.
pub const NAMESPACE: &str = "bmo";
pub const NAME: &str = "deepseek_top_up";
pub const TITLE: &str = "Top the DeepSeek balance up";
pub const DESCRIPTION: &str = "Top the DeepSeek balance up by the amount the owner names, in the currency DeepSeek keeps the balance in, with the card saved in the console's secret store; call it only when the owner asks for a top-up. One call is held to a cap. DeepSeek takes no payment by machine, so the answer says what a person does on its platform, the amount paid and the balance as its reader last read it.";
pub const CAPABILITIES: &[&str] = &["room.tools"];
/// The administrator's grant a call needs: the card is the platform's own.
pub const GRANT: &str = "platform.payment";

pub fn entry() -> String {
  format!("{NAMESPACE}/{NAME}")
}

/// What a call names: the amount alone. The card is never taken from a call.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopUpInput {
  #[serde(default)]
  pub amount: Option<f64>,
}

impl TopUpInput {
  /// The call's arguments; a call that hands none names no amount.
  pub fn read(arguments: Value) -> Result<Self, serde_json::Error> {
    if arguments.is_null() {
      Ok(Self::default())
    } else {
      serde_json::from_value(arguments)
    }
  }
}

/// Why the card's secret does not serve.
#[derive(Debug, PartialEq, Eq)]
pub enum CardFault {
  Missing,
  /// The field keys it holds no value for.
  Lacks(Vec<String>),
}

/// The fields of `wanted` the card's secret at `path` holds no value for, or that it stands nowhere.
///
/// The console's secret store writes a card as one `KEY=VALUE` line to each field it holds a value for, the value
/// as typed to the line's end, unquoted and unescaped (bmo-web's `docs/accounts.md`, 「Secret kinds」). That is the
/// store's own shape, not a dotenv file: a dotenv reader refuses a value holding a space, as a holder's name does,
/// and reads `$` and `#` as marks. So a line is split at its first `=`. Only the keys of the lines with a value are
/// kept; every value is dropped where it is read.
pub fn check_card(path: &Path, wanted: &[String]) -> Result<(), CardFault> {
  let text = std::fs::read_to_string(path).map_err(|_| CardFault::Missing)?;
  let filled: BTreeSet<&str> = text
    .lines()
    .filter_map(|line| line.split_once('='))
    .filter(|(_, value)| !value.trim().is_empty())
    .map(|(key, _)| key.trim())
    .collect();
  let lacking: Vec<String> = wanted.iter().filter(|key| !filled.contains(key.as_str())).cloned().collect();

  if lacking.is_empty() { Ok(()) } else { Err(CardFault::Lacks(lacking)) }
}

/// The moment a balance was seen, in epoch seconds, as an answer shows it: the UTC time, or the seconds where
/// they stand outside the calendar.
fn seen_at(seconds: i64) -> String {
  DateTime::from_timestamp(seconds, 0)
    .map_or_else(|| seconds.to_string(), |at| at.to_rfc3339_opts(SecondsFormat::Secs, true))
}

/// The amount a call names, above zero.
fn amount_of(input: &TopUpInput, policy: &TopUp) -> Result<f64, String> {
  input
    .amount
    .filter(|amount| amount.is_finite() && *amount > 0.0)
    .ok_or_else(|| policy.words.no_amount.clone())
}

/// The card's secret checked, or the refusal saying what a person saves.
fn card_checked(data_root: &Path, policy: &TopUp) -> Result<(), String> {
  let (line, fields) =
    match check_card(&data_root.join("env").join(&policy.card_secret), &policy.card_fields) {
      Ok(()) => return Ok(()),
      Err(CardFault::Missing) => (&policy.words.card_missing, String::new()),
      Err(CardFault::Lacks(lacking)) => (&policy.words.card_lacks, lacking.join(", ")),
    };

  Err(fill_marks(
    line,
    &[("secret", &policy.card_secret), ("kind", &policy.card_kind), ("fields", &fields)],
  ))
}

/// What the balance reader last read of DeepSeek's own road, as agency answers it.
async fn quota_of(agency: &AgencyClient, policy: &TopUp) -> Result<ProviderQuota, String> {
  agency
    .get_provider(DeepSeek.name())
    .await
    .map(|provider| provider.quota)
    .map_err(|why| fill_marks(&policy.words.balance_unasked, &[("why", &why.to_string())]))
}

/// One call checked, and answered with what a person does: DeepSeek takes no payment by machine, so nothing is
/// paid and the balance stands as its reader last read it.
pub async fn top_up_with(
  agency: &AgencyClient,
  data_root: &Path,
  input: &TopUpInput,
) -> Result<Answer, String> {
  let policy = &DEEPSEEK.top_up;
  let amount = amount_of(input, policy)?;

  card_checked(data_root, policy)?;

  let quota = quota_of(agency, policy).await?;
  let (Some(balance), Some(currency), Some(seen)) =
    (quota.balance, quota.currency.as_deref(), quota.balance_seen_at)
  else {
    return Err(policy.words.balance_unread.clone());
  };
  let cap = policy
    .cap
    .get(currency)
    .ok_or_else(|| fill_marks(&policy.words.no_cap, &[("currency", currency)]))?;

  if amount > *cap {
    return Err(fill_marks(
      &policy.words.over_cap,
      &[
        ("amount", &amount.to_string()),
        ("cap", &cap.to_string()),
        ("currency", currency),
        ("reason", &policy.cap_reason),
      ],
    ));
  }

  let by_person = fill_marks(
    &policy.words.by_person,
    &[
      ("top_up", &DEEPSEEK.balance.top_up),
      ("amount", &amount.to_string()),
      ("currency", currency),
      ("secret", &policy.card_secret),
    ],
  );
  let outcome = fill_marks(
    &policy.words.outcome,
    &[
      ("paid", "0"),
      ("balance", &balance.to_string()),
      ("currency", currency),
      ("seen", &seen_at(seen)),
    ],
  );

  Ok(Answer::text(format!("{by_person}\n{outcome}")))
}

async fn top_up(session: &Session, input: &TopUpInput) -> Result<Answer, String> {
  let data_root = session.data_root();
  let control = Control::read(&data_root, |name| std::env::var(name).ok());
  let agency = AgencyClient::new(&control.base(), control.token.unwrap_or_default())?;

  top_up_with(&agency, &data_root, input).await
}

/// The tool's entry point: one call, answered. A call that does not read is refused by a fixed sentence, never
/// by the reader's own words, which may quote what the call held.
pub async fn answer(handed: Handed) -> Result<Value, String> {
  let answered = match handed.asked {
    Asked::Call {
      entry: asked,
      arguments,
    } if asked == entry() => match TopUpInput::read(arguments) {
      Ok(input) => top_up(&handed.session, &input).await.unwrap_or_else(Answer::failure),
      Err(_) => Answer::failure("the call names amount, a number, and nothing else"),
    },
    Asked::Call { entry: asked, .. } => Answer::failure(format!("this executable holds no entry {asked}")),
    Asked::ResourcesList => return Ok(json!({ "resources": [] })),
    Asked::ResourcesRead { .. } => return Ok(json!({ "error": "this executable serves no resources" })),
    Asked::Other => return Err("this executable answers calls alone".to_owned()),
  };

  serde_json::to_value(answered).map_err(|why| format!("the answer could not be written: {why}"))
}
