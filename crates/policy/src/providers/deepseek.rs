//! What DeepSeek declares (docs/provider-presence.md): its own API, reached by the dsh harness on its
//! OpenAI-compatible route.

use serde::Deserialize;

use super::Declared;

#[derive(Debug, Deserialize)]
pub struct DeepSeek {
  #[serde(flatten)]
  pub declared: Declared,
  pub presence: Presence,
  pub model: Vec<Model>,
  pub balance: super::Balance,
}

impl DeepSeek {
  /// The model one id names, where this provider serves it.
  pub fn held(&self, id: &str) -> Option<&Model> {
    self.model.iter().find(|one| one.id == id)
  }
}

/// The road's key file, and what the provider says where it cannot serve a run.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
  pub key_file: String,
  pub missing: String,
  pub not_held: String,
}

/// One model this provider serves: its name on the wire, the name a screen shows, and the run's sizes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
  pub id: String,
  pub label: String,
  pub context_tokens: i64,
  pub answer_tokens: i64,
  pub thinking: bool,
}

crate::part!(pub DEEPSEEK: DeepSeek = "providers/deepseek.toml");
