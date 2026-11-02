//! The roles an agent may hold and the function bundles a role may give (docs/agent-roles.md): the roles
//! themselves are the store's rows, which these declarations seed once; the bundles, the kinds' roles, the
//! co-worker's role, the role delegated work goes to, the roles of agency's own runs that stand for no agent
//! row, the sending bundle, the kinds' marks and the shape of a role's name are the policy's.

use std::collections::BTreeMap;

use serde::Deserialize;

/// One function bundle a role may give: a capability of the tool host, with the short name a screen
/// writes it by.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
  pub name: String,
  pub label: String,
}

/// Whether a role's runs go on in the session an earlier run left while its cache is warm, or each opens its
/// own (docs/agent-roles.md, 「A role's sessions」).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Sessions {
  #[default]
  Keep,
  Fresh,
}

impl Sessions {
  /// The word a row and the routes write it by.
  pub fn word(self) -> &'static str {
    match self {
      Self::Keep => "keep",
      Self::Fresh => "fresh",
    }
  }

  /// The setting a word names, where it names one.
  pub fn of(word: &str) -> Option<Self> {
    match word {
      "keep" => Some(Self::Keep),
      "fresh" => Some(Self::Fresh),
      _ => None,
    }
  }

  /// Whether a run under it goes on in a kept session.
  pub fn keeps(self) -> bool {
    self == Self::Keep
  }
}

/// One role a store is seeded with: its name, its label as a screen writes it, the file under
/// `prompts/roles/` whose text is the prompt its agents are given before their own, the bundles it gives,
/// the tool entries its agents are shown by name and whether its runs keep their sessions.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
  pub name: String,
  pub label: String,
  #[serde(default)]
  pub prompt: Option<String>,
  pub bundles: Vec<String>,
  #[serde(default)]
  pub shown: Vec<String>,
  /// The most agents of this role that run at once, where the deployment bounds them.
  #[serde(default)]
  pub max_concurrent: Option<usize>,
  /// Whether its runs keep their sessions; `keep` where the declaration names none.
  #[serde(default)]
  pub sessions: Sessions,
}

/// Every bundle and every seeded role, each in the order a screen lists them, and the role a co-worker
/// holds.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roles {
  /// The role a co-worker's run holds: its place has no agents row to name one of its own.
  pub coworker: String,
  /// The role whose agents carry a delegated task where its ask names none.
  pub delegate: String,
  /// The role the usage reader's renewal turn holds, a run that stands for no agent row.
  pub renewal: String,
  /// The role the gate project's judging batch holds, a run that stands for no agent row.
  pub judging: String,
  /// The bundle that opens sending into a room.
  pub send: String,
  /// The shape of a role's name.
  pub name_pattern: String,
  /// The most characters a role's name holds.
  pub name_most: usize,
  /// The most characters a role's label holds.
  pub label_most: usize,
  /// The role a row of each kind holds where its writer names none, by the kind.
  pub kinds: BTreeMap<String, String>,
  /// The mark a screen shows beside every agent of each kind, by the kind.
  pub icons: BTreeMap<String, String>,
  pub bundles: Vec<Bundle>,
  pub roles: Vec<Role>,
}

impl Roles {
  /// The mark every agent of a kind is shown with, where the policy names one for it.
  pub fn icon(&self, kind: &str) -> Option<&str> {
    self.icons.get(kind).map(String::as_str)
  }

  /// The seed of a role of this name.
  pub fn role(&self, name: &str) -> Option<&Role> {
    self.roles.iter().find(|role| role.name == name)
  }

  /// Whether the policy seeds a role of this name.
  pub fn holds(&self, name: &str) -> bool {
    self.role(name).is_some()
  }

  /// The bundles a seeded role gives; none for a role the policy does not seed.
  pub fn bundles_of(&self, name: &str) -> &[String] {
    self.role(name).map_or(&[], |role| &role.bundles)
  }

  /// Whether the policy declares a bundle of this name, which a role may then give.
  pub fn declares_bundle(&self, name: &str) -> bool {
    self.bundles.iter().any(|bundle| bundle.name == name)
  }

  /// Whether a role of this name is a kind's own.
  pub fn kind_default(&self, name: &str) -> bool {
    self.kinds.values().any(|role| role == name)
  }

  /// The short name a screen writes a bundle by, or the bundle's own name where none is declared.
  pub fn bundle_label<'a>(&'a self, name: &'a str) -> &'a str {
    self
      .bundles
      .iter()
      .find(|bundle| bundle.name == name)
      .map_or(name, |bundle| &bundle.label)
  }
}

crate::part!(pub ROLES: Roles = "roles.toml");
