//! The roles an agent may hold and the function bundles each gives (docs/agent-roles.md).

use serde::Deserialize;

/// One function bundle a role may give: a capability of the tool host, with the short name a screen
/// writes it by.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
  pub name: String,
  pub label: String,
}

/// One role: its name, its label as a screen writes it, the bundles it gives and the tool entries its
/// agents are shown by name.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
  pub name: String,
  pub label: String,
  pub bundles: Vec<String>,
  #[serde(default)]
  pub shown: Vec<String>,
}

/// Every bundle and every role, each in the order a screen lists them, and the role a co-worker holds.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roles {
  /// The role a co-worker's run holds: its place has no agents row to name one of its own.
  pub coworker: String,
  /// The bundle that opens sending into a room.
  pub send: String,
  pub bundles: Vec<Bundle>,
  pub roles: Vec<Role>,
}

impl Roles {
  pub fn role(&self, name: &str) -> Option<&Role> {
    self.roles.iter().find(|role| role.name == name)
  }

  /// Whether the policy declares a role of this name.
  pub fn holds(&self, name: &str) -> bool {
    self.role(name).is_some()
  }

  /// The bundles a role gives; none for a role the policy does not declare.
  pub fn bundles_of(&self, name: &str) -> &[String] {
    self.role(name).map_or(&[], |role| &role.bundles)
  }

  /// The short name a screen writes a bundle by, or the bundle's own name where none is declared.
  pub fn bundle_label<'a>(&'a self, name: &'a str) -> &'a str {
    self
      .bundles
      .iter()
      .find(|bundle| bundle.name == name)
      .map_or(name, |bundle| &bundle.label)
  }

  /// Every role's label, in the policy's order, as a refusal lists them.
  pub fn labels(&self) -> Vec<&str> {
    self.roles.iter().map(|role| role.label.as_str()).collect()
  }
}

crate::part!(pub ROLES: Roles = "roles.toml");
