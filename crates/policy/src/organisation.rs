//! What agency reads off the account service's delegation chain for the person an agent serves, beside its
//! prompt layers: the supply sources a person's chain leaves open, the agents an account is given as it is made
//! ready, and who may add and change repositories (docs/supply.md, docs/role-agents.md, docs/repositories.md).

use serde::Deserialize;

use crate::prompt_layers::Kind;

/// The three supply sources' settings and the words their walk says.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
  /// The source word a run on its organisation's own credential carries.
  pub organisation_word: String,
  /// With `{provider}`.
  pub no_organisation: String,
  pub platform: Kind,
  pub organisation: Kind,
  pub personal: Kind,
}

/// The sign-up composition's setting.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composition {
  pub key: String,
  pub description: String,
  pub shape: String,
  pub mode: String,
  pub rules: bool,
  pub levels: Vec<String>,
  pub unit_permission: String,
  pub person_writer: String,
  /// The most agents of one role a composition makes for one person.
  pub most_agents: usize,
}

impl Composition {
  /// The composition as a kind the account service takes, with no default: none set gives the platform's own.
  pub fn kind(&self) -> Kind {
    Kind {
      key: self.key.clone(),
      description: self.description.clone(),
      shape: self.shape.clone(),
      choices: Vec::new(),
      mode: self.mode.clone(),
      rules: self.rules,
      levels: self.levels.clone(),
      unit_permission: self.unit_permission.clone(),
      person_writer: self.person_writer.clone(),
      default: None,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Organisation {
  pub sources: Sources,
  pub composition: Composition,
  pub repositories: Kind,
}

crate::part!(pub ORGANISATION: Organisation = "organisation.toml");
