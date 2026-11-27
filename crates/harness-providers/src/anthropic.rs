//! The Anthropic API: present where the gateway's key file and its members' key files stand private under the
//! data root, and its models the members of its road (docs/provider-presence.md). Its program is Claude Code on
//! agency's gateway, so a run spends the member's API key rather than a subscription's windows.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use policy::providers::Declared;
use policy::providers::anthropic::ANTHROPIC;
use serde_json::{Value, json};

use crate::claude_code::ClaudeCode;
use crate::member;
use crate::{Machine, Offered, OfferedModel, Offering, PlaceCache, Provider, Reading, Reason};

pub struct Anthropic;

impl Provider for Anthropic {
  fn name(&self) -> &'static str {
    "anthropic"
  }

  fn key_file(&self) -> Option<&'static str> {
    Some(ANTHROPIC.presence.key_file.as_str())
  }

  fn declared(&self) -> &'static Declared {
    &ANTHROPIC.declared
  }

  // The gateway publishes no cache lifetime of its members, so nothing keeps a place warm.
  fn place_cache(&self, _model: &str, _upstream: Option<&str>) -> Result<PlaceCache, String> {
    Err("The Anthropic API's members publish no cache lifetime here, so there is no clock to keep one warm by.".to_owned())
  }

  // Its program is Claude Code under one of its road's homes.
  fn owns_program(&self, machine: &Machine, args: &[String], env: &BTreeMap<String, String>) -> bool {
    member::owns(machine, args, env, self.name())
  }

  fn missing(&self, machine: &Machine) -> Option<Reason> {
    member::missing(machine, self.name(), &ANTHROPIC.presence.missing_code, &ANTHROPIC.presence.missing)
  }

  fn not_registered(&self) -> Option<&'static str> {
    Some(&ANTHROPIC.presence.not_held)
  }

  fn models_on_gateway(&self) -> bool {
    true
  }

  fn holds_model(&self, machine: &Machine, model: &str) -> bool {
    member::member_named(machine, self.name(), model).ok().flatten().is_some()
  }

  fn catalogue<'a>(&'a self, machine: &'a Machine, _config_dir: Option<&'a Path>) -> Reading<'a> {
    Box::pin(async move {
      let held = member::road_members(machine, self.name())?;
      let models: Vec<Value> = held
        .iter()
        .map(|one| {
          json!({
            "id": one.member.name,
            "name": one.member.label.clone().unwrap_or_else(|| one.member.name.clone()),
            "contextLength": one.context_tokens,
            "defaultReasoningEffort": if one.thinking { "medium" } else { "none" },
            "supportedReasoningEfforts": &ANTHROPIC.declared.effort.words,
          })
        })
        .collect();

      Ok(json!({ "models": models }))
    })
  }

  fn offered<'a>(
    &'a self,
    machine: &'a Machine,
    _config_dir: Option<&'a Path>,
    _now_ms: f64,
  ) -> Offering<'a> {
    Box::pin(async move {
      let efforts: Vec<Option<String>> = ANTHROPIC
        .declared
        .effort
        .vocabulary()
        .into_iter()
        .map(|word| word.map(str::to_owned))
        .collect();
      let models = member::road_members(machine, self.name())?.into_iter().map(|one| OfferedModel {
        id: one.member.name,
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

  fn claude_effort(&self, word: &str) -> Option<&'static str> {
    ANTHROPIC.claude.as_ref().and_then(|claude| claude.word(word))
  }

  fn session_of_event(&self, event: &serde_json::Value) -> Option<String> {
    ClaudeCode.session_of_event(event)
  }

  fn progress_of(&self, line: &str) -> Option<String> {
    ClaudeCode.progress_of(line)
  }

  fn session_records(
    &self,
    machine: &Machine,
    _logins: &[PathBuf],
    cwd: &Path,
    session: &str,
    model: &str,
  ) -> Vec<PathBuf> {
    member::session_records(machine, self.name(), cwd, session, model)
  }

  fn home_records(&self, machine: &Machine, _logins: &[PathBuf], cwd: &Path) -> Vec<PathBuf> {
    member::home_records(machine, self.name(), cwd)
  }

  fn forget_prompts(
    &self,
    machine: &Machine,
    _logins: &[PathBuf],
    cwd: &Path,
    session: Option<&str>,
  ) -> Result<usize, String> {
    member::forget_prompts(machine, self.name(), cwd, session)
  }
}
