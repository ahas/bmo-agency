//! The seeds agency's server settles its store with (docs/store.md, 「Its seeds」): the values agency's
//! policy declares — the account, the catalogue's entries, the base prompts and the roles — with the
//! account's credential the machine's own Claude Code login directory, each base prompt's text its seed file
//! in the code tree, and each role's prompt and description its prompt file or its co-worker definition
//! there.

use std::path::Path;

use harness_providers::Machine;
use harness_providers::claude_code::ClaudeCode;
use policy::base_prompts::BASE_PROMPTS;
use policy::roles::{ROLES, Role};
use policy::seeds::SEEDS;
use serde::Deserialize;
use store::seed::{AccountSeed, BasePromptSeed, CatalogueSeed, RoleSeed, Seeds};

use crate::control::claude_accounts::Credential;
use crate::order::{Entry, Order};
use crate::places::machine_code_root;

/// The seeds the deployment's store is settled with on this machine.
pub fn deployment(machine: &Machine) -> Seeds {
  // The seeded account's login is the machine's own Claude Code login directory.
  let login = Credential::Directory {
    home: ClaudeCode::machine_config_dir(machine),
  };

  Seeds {
    account: Some(AccountSeed {
      id: SEEDS.account.id.clone(),
      name: SEEDS.account.name.clone(),
      credential_kind: login.kind().word().to_owned(),
      credential_path: login.path().to_string_lossy().into_owned(),
    }),
    catalogue: SEEDS
      .catalogue
      .iter()
      .map(|entry| CatalogueSeed {
        name: entry.name.clone(),
        label: entry.label.clone(),
        file: entry.file.clone(),
        bytes: entry.bytes,
        sha256: entry.sha256.clone(),
        source: entry.source.clone(),
        engine: entry.engine.clone(),
        registration: entry.registration.clone(),
        thinking: entry.thinking,
        temperature: entry.temperature,
        top_p: entry.top_p,
        presence_penalty: entry.presence_penalty,
        max_tokens: entry.max_tokens,
        note: entry.note.clone(),
      })
      .collect(),
    base_prompts: base_prompt_seeds(&machine_code_root(machine)),
    roles: role_seeds(&machine_code_root(machine)),
  }
}

/// The roles the policy declares, each with the text of the prompt file or the co-worker definition it
/// names under the code tree. One whose file cannot be read is said and left out, so it is given by the first
/// settling that can read it.
pub fn role_seeds(code: &Path) -> Vec<RoleSeed> {
  ROLES
    .roles
    .iter()
    .filter_map(|role| match role_texts(code, role) {
      Ok((prompt, description)) => Some(role_seed(role, prompt, description)),
      Err(why) => {
        eprintln!("agency: the role seed {} is not given: {why}", role.name);

        None
      }
    })
    .collect()
}

// A seeded role's row: its order with the policy's share, and its compaction threshold its first provider's
// declared default.
fn role_seed(role: &Role, prompt: Option<String>, description: Option<String>) -> RoleSeed {
  let order = Order {
    entries: role
      .order
      .iter()
      .map(|entry| Entry {
        provider: entry.provider.clone(),
        model: entry.model.clone(),
        effort: entry.effort.clone(),
        thinking: entry.thinking.map(|on| if on { "on" } else { "off" }.to_owned()),
        answer_tokens: None,
      })
      .collect(),
    quota_share: ROLES.quota_share,
  };

  RoleSeed {
    name: role.name.clone(),
    label: role.label.clone(),
    bundles: role.bundles.clone(),
    line: role.line.clone(),
    order: order.entries_text(),
    quota_share: order.quota_share,
    max_concurrent: role.max_concurrent,
    prompt,
    description,
    sessions: role.sessions.word().to_owned(),
    fold_ratio: order.default_fold_ratio(),
    callable: role.callable,
  }
}

// What a co-worker definition's front matter says of it.
#[derive(Deserialize)]
struct Definition {
  description: Option<String>,
}

// A role's prompt and description: the prompt file's text, or a co-worker definition's body and its front
// matter's description.
fn role_texts(code: &Path, role: &Role) -> Result<(Option<String>, Option<String>), String> {
  let read = |file: &Path| std::fs::read_to_string(file).map_err(|why| format!("{}: {why}", file.display()));

  if let Some(file) = &role.prompt {
    return Ok((Some(read(&code.join("prompts").join("roles").join(file))?), None));
  }

  let Some(file) = &role.definition else {
    return Ok((None, None));
  };
  let path = code.join("prompts").join("agents").join("worker").join("coworkers").join(file);
  let text = read(&path)?;
  // The front matter stands between the definition's two marker lines; its fields are YAML.
  let text = text.replace("\r\n", "\n");
  let (fields, body) = text
    .strip_prefix("---\n")
    .and_then(|rest| rest.split_once("\n---\n"))
    .ok_or_else(|| format!("{}: no front matter", path.display()))?;
  let definition: Definition =
    serde_norway::from_str(fields).map_err(|why| format!("{}: its front matter: {why}", path.display()))?;
  let body = body.trim_start().to_owned();

  Ok((Some(body), definition.description))
}

/// The base prompts the policy declares, each with its seed file's text under the code tree. One whose
/// file cannot be read is said and left out, so it is given by the first settling that can read it.
pub fn base_prompt_seeds(code: &Path) -> Vec<BasePromptSeed> {
  BASE_PROMPTS
    .seed
    .iter()
    .filter_map(|seed| {
      let file = code.join("prompts").join("base").join(&seed.file);

      match std::fs::read_to_string(&file) {
        Ok(text) => Some(BasePromptSeed {
          name: seed.name.clone(),
          label: seed.label.clone(),
          text,
          agents: seed.agents.clone(),
        }),
        Err(why) => {
          eprintln!("agency: the base prompt seed {} is not given: {}: {why}", seed.name, file.display());

          None
        }
      }
    })
    .collect()
}
