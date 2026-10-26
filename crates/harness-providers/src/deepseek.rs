//! DeepSeek: present on this machine where the road's key file stands private under the data root, and
//! its models the two its policy declares (docs/provider-presence.md). Its program is bmo-tui standing a
//! dsh session on DeepSeek's own API.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use policy::providers::Declared;
use policy::providers::deepseek::{DEEPSEEK, Model as DeclaredModel};
use serde_json::json;

use crate::balance::{Balance, OffersBalance, figure, key_in_file};
use crate::{Machine, Offered, OfferedModel, Offering, PlaceCache, Provider, Reading, Reason};

pub struct DeepSeek;

impl DeepSeek {
  /// Where the road's key file stands.
  pub fn key_file(machine: &Machine) -> PathBuf {
    machine.data_root().join("env").join(&DEEPSEEK.presence.key_file)
  }

  /// The model one id names, where this provider serves it.
  pub fn model(id: &str) -> Option<&'static DeclaredModel> {
    DEEPSEEK.held(id)
  }
}

/// Whether a secret file may be used: a file of its own, private to its owner, not empty.
fn usable_secret(path: &Path) -> bool {
  use std::os::unix::fs::PermissionsExt;

  std::fs::metadata(path)
    .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o777 == 0o600 && meta.len() > 0)
}

// DeepSeek's balance: asked with the road's key file, the first of the answer's balances the reading.
impl OffersBalance for DeepSeek {
  fn declared(&self) -> &'static policy::providers::Balance {
    &DEEPSEEK.balance
  }

  fn key(&self, machine: &Machine) -> Result<String, String> {
    key_in_file(&Self::key_file(machine))
  }

  fn read(&self, answer: &serde_json::Value) -> Result<Balance, String> {
    let first = answer
      .get("balance_infos")
      .and_then(serde_json::Value::as_array)
      .and_then(|infos| infos.first())
      .ok_or("the answer states no balance")?;
    let amount = figure(first.get("total_balance")).ok_or("the balance states no total")?;
    let currency = first
      .get("currency")
      .and_then(serde_json::Value::as_str)
      .filter(|currency| !currency.is_empty())
      .ok_or("the balance states no currency")?;

    Ok(Balance {
      amount,
      currency: currency.to_owned(),
    })
  }
}

impl Provider for DeepSeek {
  fn name(&self) -> &'static str {
    "deepseek"
  }

  fn balance(&self) -> Option<&'static dyn OffersBalance> {
    Some(&DeepSeek)
  }

  fn declared(&self) -> &'static Declared {
    &DEEPSEEK.declared
  }

  fn missing(&self, machine: &Machine) -> Option<Reason> {
    (!usable_secret(&Self::key_file(machine)))
      .then(|| Reason::new(&DEEPSEEK.presence.missing_code, &DEEPSEEK.presence.missing))
  }

  fn not_registered(&self) -> Option<&'static str> {
    Some(&DEEPSEEK.presence.not_held)
  }

  fn holds_model(&self, model: &str) -> bool {
    Self::model(model).is_some()
  }

  // Its catalogue is the models its policy declares: its API's listing needs the key, and the
  // models are a fact of the deployment rather than of the answer.
  fn catalogue<'a>(&'a self, _machine: &'a Machine, _config_dir: Option<&'a Path>) -> Reading<'a> {
    Box::pin(async move {
      let models: Vec<serde_json::Value> = DEEPSEEK
        .model
        .iter()
        .map(|model| {
          json!({
            "id": model.id,
            "name": model.label,
            "contextLength": model.context_tokens,
            "defaultReasoningEffort": if model.thinking { "high" } else { "none" },
            "supportedReasoningEfforts": ["none", "low", "medium", "high", "max"],
          })
        })
        .collect();

      Ok(json!({ "models": models }))
    })
  }

  fn offered<'a>(
    &'a self,
    _machine: &'a Machine,
    _config_dir: Option<&'a Path>,
    _now_ms: f64,
  ) -> Offering<'a> {
    Box::pin(async move {
      let efforts: Vec<Option<String>> = DEEPSEEK
        .declared
        .effort
        .vocabulary()
        .into_iter()
        .map(|word| word.map(str::to_owned))
        .collect();
      let models = DEEPSEEK.model.iter().map(|model| OfferedModel {
        id: model.id.clone(),
        efforts: efforts.clone(),
        free: None,
        in_service: Some(true),
      });

      Ok(Offered {
        models: models.collect(),
        stale_read_at: None,
      })
    })
  }

  // DeepSeek publishes no cache lifetime, so nothing keeps a place warm.
  fn place_cache(&self, _model: &str, _upstream: Option<&str>) -> Result<PlaceCache, String> {
    Err("DeepSeek publishes no cache lifetime, so there is no clock to keep it warm by.".to_owned())
  }

  // A level the run chose reaches it where its model takes the scale's level; no thinking is DeepSeek's
  // own word `none`, and a mapping's own word stands otherwise.
  fn run_effort(
    &self,
    mapped: Option<&str>,
    thinking: Option<bool>,
    level: Option<&str>,
    levels: &[String],
  ) -> Option<String> {
    if thinking == Some(false) {
      return Some("none".to_owned());
    }

    match level {
      Some(level) if levels.iter().any(|one| one == level) => Some(level.to_owned()),
      _ => mapped.map(str::to_owned),
    }
  }

  // Its program is bmo-tui standing this provider's dsh session.
  fn owns_program(&self, _machine: &Machine, args: &[String], _env: &BTreeMap<String, String>) -> bool {
    crate::stands_dsh_session(self, args)
  }
}
