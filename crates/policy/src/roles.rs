//! The roles an agent may hold and the function bundles a role may give (docs/agent-roles.md): the roles
//! themselves are the store's rows, which these declarations seed once; the bundles, the kinds' roles, the
//! co-worker's role, the sending bundle, the kinds' marks and the shape of a role's name are the policy's.

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

/// One role a store is seeded with: its name, its label as a screen writes it, the file under
/// `prompts/roles/` whose text is the prompt its agents are given before their own, the bundles it gives
/// and the tool entries its agents are shown by name.
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
}

/// Every bundle and every seeded role, each in the order a screen lists them, and the role a co-worker
/// holds.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roles {
  /// The role a co-worker's run holds: its place has no agents row to name one of its own.
  pub coworker: String,
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
