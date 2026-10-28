//! The gateway's table, written from the registry (docs/own-llm.md, 「The gateway」): what LiteLLM is
//! started on. Every registration is served under its name with one entry for each of its addresses,
//! and a registration of several addresses once more under `<name>@<node>` for each, one address
//! alone, so a session placed on a node asks that node only. The table names each key by a variable
//! and never a key; the launcher sets the variables from the key files in its child's environment.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use store::registry::Registration;

/// The hooks LiteLLM loads, each from a file of its name beside the table: the one that carries a
/// Messages request's thinking to the chat template, the one that writes a run's sampling settings
/// from its header into the request, the one that folds a system turn inside the conversation into
/// the person's turn beside it, the one that copies a run's long tools from their header into the
/// field the engines read, and the one that starts a request from its role's saved prefix on an instance
/// that keeps role files.
pub const HOOKS: [&str; 5] = ["thinking", "sampling", "reminders", "long_tools", "roles"];

/// The gateway's own key, as the variable the table names it by.
pub const GATEWAY_KEY_VARIABLE: &str = "LLM_GATEWAY_KEY";

/// The table as LiteLLM reads it, with the key files its variables are read from.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
  /// What LiteLLM reads: JSON, which its YAML reader takes as it stands.
  pub text: String,
  /// Each variable the table names, with the key file under the data root's `env/` it is read from.
  pub key_files: BTreeMap<String, String>,
}

/// The variable one key file is handed under.
pub fn key_variable(key_file: &str) -> String {
  let named: String = key_file
    .chars()
    .map(|one| if one.is_ascii_alphanumeric() { one.to_ascii_uppercase() } else { '_' })
    .collect();
  format!("BMO_KEY_{named}")
}

/// The chat completions address without its path's last two parts, which the OpenAI client adds.
fn api_base(address: &str) -> String {
  let Ok(mut parsed) = url::Url::parse(address) else {
    return address.trim_end_matches('/').to_owned();
  };
  let path = parsed.path().strip_suffix("/chat/completions").unwrap_or(parsed.path()).to_owned();
  parsed.set_path(&path);
  parsed.as_str().trim_end_matches('/').to_owned()
}

#[derive(Serialize)]
struct Config {
  model_list: Vec<Entry>,
  litellm_settings: Settings,
  general_settings: General,
}

#[derive(Serialize)]
struct Entry {
  model_name: String,
  litellm_params: Params,
  /// What the hooks read of the address behind the entry, which LiteLLM hands no backend; none where the
  /// address says nothing.
  #[serde(skip_serializing_if = "Option::is_none")]
  model_info: Option<Info>,
}

#[derive(Clone, Serialize)]
struct Info {
  /// The instance behind the address keeps role files (docs/own-llm.md, 「Role files」).
  role_files: bool,
}

#[derive(Clone, Serialize)]
struct Params {
  model: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  api_base: Option<String>,
  /// An address that takes no key is still handed one, since the OpenAI client will not go without;
  /// the backend reads none.
  api_key: String,
  timeout: i64,
  stream_timeout: i64,
  /// Thinking off where a request names nothing: the template's own default thinks for minutes on a
  /// slow card, even on a turn that asked for none.
  chat_template_kwargs: TemplateSettings,
}

#[derive(Clone, Serialize)]
struct TemplateSettings {
  enable_thinking: bool,
}

#[derive(Serialize)]
struct Settings {
  /// A parameter a backend does not take is dropped rather than refused.
  drop_params: bool,
  /// An `openai/` backend is asked at its chat completions path; left to itself the gateway asks the
  /// Responses API, which the backends do not serve.
  use_chat_completions_url_for_anthropic_messages: bool,
  callbacks: Vec<String>,
  /// A failed request is answered once and not retried here: the run that asked decides. Each entry
  /// holds its own wait; the gateway-wide `request_timeout` is not read on the Messages path, so none
  /// is set.
  num_retries: u32,
}

#[derive(Serialize)]
struct General {
  master_key: String,
}

/// One member a deployment serves besides the registry: a provider's own API behind a model name the
/// selector shows. Its key is named by its file and never written here.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Member {
  /// The model name a caller asks for.
  pub name: String,
  /// The LiteLLM provider: `deepseek`, `openrouter`, `anthropic` or `openai`.
  pub provider: String,
  /// The model id the provider's API knows.
  pub model: String,
  /// The provider's API base, where it has one.
  #[serde(default)]
  pub api_base: Option<String>,
  /// The key file under the data root's `env/`.
  pub key_file: String,
  /// The request wait in seconds.
  #[serde(default = "default_wait")]
  pub wait_seconds: i64,
}

fn default_wait() -> i64 {
  300
}

pub fn gateway_table(registrations: &[Registration]) -> Table {
  gateway_table_with_members(registrations, &[])
}

pub fn gateway_table_with_members(registrations: &[Registration], members: &[Member]) -> Table {
  let mut key_files = BTreeMap::new();
  let mut models = Vec::new();

  for registration in registrations {
    for address in &registration.addresses {
      let variable = address.key_file.as_deref().map(key_variable);
      if let (Some(variable), Some(key_file)) = (&variable, &address.key_file) {
        key_files.insert(variable.clone(), key_file.clone());
      }
      let params = Params {
        model: format!("openai/{}", registration.model),
        api_base: Some(api_base(&address.url)),
        api_key: variable.map_or_else(|| "no-key".to_owned(), |variable| format!("os.environ/{variable}")),
        timeout: registration.wait_seconds,
        stream_timeout: registration.wait_seconds,
        chat_template_kwargs: TemplateSettings {
          enable_thinking: false,
        },
      };
      let info = address.role_files.then_some(Info { role_files: true });
      if registration.addresses.len() > 1 {
        models.push(Entry {
          model_name: registration.name.clone(),
          litellm_params: params.clone(),
          model_info: info.clone(),
        });
        models.push(Entry {
          model_name: format!("{}@{}", registration.name, address.node),
          litellm_params: params,
          model_info: info,
        });
      } else {
        models.push(Entry {
          model_name: registration.name.clone(),
          litellm_params: params,
          model_info: info,
        });
      }
    }
  }

  for member in members {
    let variable = key_variable(&member.key_file);
    key_files.insert(variable.clone(), member.key_file.clone());
    let provider = match member.provider.as_str() {
      "anthropic" | "deepseek" | "openrouter" | "openai" => member.provider.as_str(),
      _ => continue,
    };
    models.push(Entry {
      model_name: member.name.clone(),
      litellm_params: Params {
        model: format!("{provider}/{}", member.model),
        api_base: member.api_base.clone(),
        api_key: format!("os.environ/{variable}"),
        timeout: member.wait_seconds,
        stream_timeout: member.wait_seconds,
        chat_template_kwargs: TemplateSettings {
          enable_thinking: false,
        },
      },
      model_info: None,
    });
  }

  let config = Config {
    model_list: models,
    litellm_settings: Settings {
      drop_params: true,
      use_chat_completions_url_for_anthropic_messages: true,
      callbacks: HOOKS.iter().map(|hook| format!("{hook}.handler")).collect(),
      num_retries: 0,
    },
    general_settings: General {
      master_key: format!("os.environ/{GATEWAY_KEY_VARIABLE}"),
    },
  };
  let text = serde_json::to_string_pretty(&config).expect("the table is plain data");
  Table {
    text: format!("{text}\n"),
    key_files,
  }
}

fn hex(bytes: &[u8]) -> String {
  format!("{:x}", Sha256::digest(bytes))
}

/// What decides whether a running child serves the table as it stands: the table itself, what each key
/// file it names holds, and the hooks it loads, so a key given again or a hook changed reaches the
/// gateway as a new table would. The keys are digested and never kept.
pub fn table_digest(
  table: &Table,
  read_key: impl Fn(&str) -> Option<String>,
  hooks: &[(String, Vec<u8>)],
) -> String {
  let mut hash = Sha256::new();
  hash.update(table.text.as_bytes());
  for (variable, key_file) in &table.key_files {
    hash.update(format!("\n{variable}=").as_bytes());
    hash.update(hex(read_key(key_file).unwrap_or_default().as_bytes()).as_bytes());
  }
  for (name, text) in hooks {
    hash.update(format!("\nhook {name}=").as_bytes());
    hash.update(hex(text).as_bytes());
  }
  format!("{:x}", hash.finalize())[..16].to_owned()
}
