//! The base prompts' bounds and the values a prompt may carry (docs/base-prompts.md).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasePromptRules {
  /// The pattern a prompt's name matches.
  pub name_pattern: String,
  /// The longest a prompt's name may be, in characters.
  pub name_most: usize,
  /// The longest a prompt's label may be, in characters.
  pub label_most: usize,
  /// The longest a prompt's text may be, in characters.
  pub text_most: usize,
  /// The names of the values a prompt may carry as `{{name}}`.
  pub values: Vec<String>,
}

crate::part!(pub BASE_PROMPTS: BasePromptRules = "base-prompts.toml");
