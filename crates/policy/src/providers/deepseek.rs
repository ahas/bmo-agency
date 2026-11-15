//! What DeepSeek declares (docs/provider-presence.md): its own API, reached by Claude Code through agency's
//! gateway member.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::Declared;

#[derive(Debug, Deserialize)]
pub struct DeepSeek {
  #[serde(flatten)]
  pub declared: Declared,
  pub presence: Presence,
  pub balance: super::Balance,
  pub top_up: TopUp,
  /// The Claude Code effort word a run of each DeepSeek word goes out with.
  #[serde(default)]
  pub claude: Option<super::Claude>,
}

/// The road's key file, and what the provider says where it cannot serve a run.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
  pub key_file: String,
  pub missing_code: String,
  pub missing: String,
  pub not_held: String,
}

/// The top-up tool's reading (docs/tools.md, 「The DeepSeek top-up」): the card's kind and fields it checks, the
/// cap on one call by currency, and what it answers. Which secret holds the card is agency's setting, not policy.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopUp {
  /// The console's kind the card is saved in.
  pub card_kind: String,
  /// The field keys a saved card holds, each one line.
  pub card_fields: Vec<String>,
  pub cap_reason: String,
  /// The largest amount one call names, by currency.
  pub cap: BTreeMap<String, f64>,
  pub words: TopUpWords,
}

/// What the top-up tool answers, each `{name}` filled in with what it names.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopUpWords {
  pub no_amount: String,
  pub settings_unasked: String,
  pub card_unnamed: String,
  pub card_missing: String,
  pub card_lacks: String,
  pub balance_unasked: String,
  pub balance_unread: String,
  pub no_cap: String,
  pub over_cap: String,
  pub by_person: String,
  pub outcome: String,
}

crate::part!(pub DEEPSEEK: DeepSeek = "providers/deepseek.toml");
