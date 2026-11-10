//! The repositories as the store keeps them (docs/repositories.md, 「What a repository row holds」): a place
//! runs work in, never an agent; every field is read off its row at every use.

use sea_orm::{ConnectionTrait, QueryResult};
use store::{Fault, all, one};

/// A provider and one model of it, as a repository's pin (docs/repositories.md, 「A pinned model」), an item's
/// model and a row's forced provider (docs/provider-order.md, 「A provider the owner forces」) name one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
  pub provider: String,
  pub model: String,
}

impl Pin {
  /// The pin two columns hold, none where either is missing or empty.
  pub fn of_parts(provider: Option<String>, model: Option<String>) -> Option<Self> {
    provider
      .filter(|provider| !provider.trim().is_empty())
      .zip(model.filter(|model| !model.trim().is_empty()))
      .map(|(provider, model)| Self { provider, model })
  }

  /// The model an item's `model` label names (docs/repositories.md, 「A model an item names」):
  /// `<provider>/<model>`, the provider before the first `/` and the model all after it. None for a label
  /// that is `auto`, empty or absent; the refusal's sentence for one that names no provider and model.
  pub fn of_label(label: Option<&str>) -> Result<Option<Self>, String> {
    let Some(label) = label.map(str::trim).filter(|label| !label.is_empty() && *label != AUTO) else {
      return Ok(None);
    };
    let named = label.split_once('/').and_then(|(provider, model)| {
      Self::of_parts(Some(provider.trim().to_owned()), Some(model.trim().to_owned()))
    });

    named.map(Some).ok_or_else(|| {
      format!("The label {MODEL_LABEL}={label} names no provider and model, as <provider>/<model>.")
    })
  }

  /// The pin as JSON answers it.
  pub fn view(&self) -> serde_json::Value {
    serde_json::json!({ "provider": self.provider, "model": self.model })
  }
}

/// The label an item names the model its job runs on with.
pub const MODEL_LABEL: &str = "model";

/// The label an item names the role its job runs as with (docs/repositories.md, 「A role an item names」).
pub const ROLE_LABEL: &str = "role";

/// The label an item names the agent its job runs on with (docs/repositories.md, 「An agent an item names」).
pub const AGENT_LABEL: &str = "agent";

/// The value of an item's `model` label that leaves its job to the order.
pub const AUTO: &str = "auto";

/// One repository as its row holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
  pub name: String,
  pub label: Option<String>,
  pub owner_account: Option<i64>,
  /// The base checkout, where its jobs work.
  pub path: String,
  /// The work node the base checkout stands on, where it stands on one.
  pub node: Option<String>,
  /// How many support worktrees stand beside the base.
  pub support_count: i64,
  /// The paths of the documents a run's prefix carries here, in order.
  pub prefix_documents: Vec<String>,
  pub preamble: Option<String>,
  /// The prompt folder under `prompts/agents/` its rules are read from.
  pub rules: Option<String>,
  pub dev_command: Option<String>,
  pub pin: Option<Pin>,
  pub downgrade_heal: bool,
}

impl Repository {
  pub fn of(row: &QueryResult) -> Result<Self, Fault> {
    let documents: Option<String> = row.try_get("", "prefix_documents")?;
    let provider: Option<String> = row.try_get("", "pinned_provider")?;
    let model: Option<String> = row.try_get("", "pinned_model")?;

    Ok(Self {
      name: row.try_get("", "name")?,
      label: row.try_get("", "label")?,
      owner_account: row.try_get("", "owner_account")?,
      path: row.try_get("", "path")?,
      node: row.try_get("", "node")?,
      support_count: row.try_get("", "support_count")?,
      prefix_documents: documents.and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default(),
      preamble: row.try_get("", "preamble")?,
      rules: row.try_get("", "prompts")?,
      dev_command: row.try_get("", "dev_command")?,
      pin: Pin::of_parts(provider, model),
      downgrade_heal: row.try_get::<i64>("", "downgrade_heal")? != 0,
    })
  }
}

/// Every repository the store holds, by name.
pub async fn every<C: ConnectionTrait>(db: &C) -> Result<Vec<Repository>, Fault> {
  all(db, r#"SELECT * FROM "repositories" ORDER BY "name""#, vec![])
    .await?
    .iter()
    .map(Repository::of)
    .collect()
}

/// The repository of this name, where the store holds one.
pub async fn named<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<Repository>, Fault> {
  one(db, r#"SELECT * FROM "repositories" WHERE "name" = $1"#, vec![name.into()])
    .await?
    .as_ref()
    .map(Repository::of)
    .transpose()
}

/// The repository whose base checkout is this path, where one is.
pub async fn at_path<C: ConnectionTrait>(db: &C, path: &str) -> Result<Option<Repository>, Fault> {
  one(db, r#"SELECT * FROM "repositories" WHERE "path" = $1"#, vec![path.into()])
    .await?
    .as_ref()
    .map(Repository::of)
    .transpose()
}
