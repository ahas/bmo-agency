//! Agency's policy (docs/policy.md): the fixed numbers and words it runs by, one TOML file to a part under
//! `policy/`, compiled in here. Each part is read once, on first use, into its type.

use serde::de::DeserializeOwned;

pub mod agent_renames;
pub mod answer_ceilings;
pub mod base_prompts;
pub mod browser;
pub mod chat_health;
pub mod chat_line;
pub mod companion_runner;
pub mod companions;
pub mod deployment;
pub mod door_words;
pub mod gates;
pub mod job_prompt;
pub mod keepalive;
pub mod model_catalogue;
pub mod node_states;
pub mod place_words;
pub mod providers;
pub mod reasoning;
pub mod registry;
pub mod research;
pub mod resident_turn;
pub mod rest;
pub mod roles;
pub mod run_lines;
pub mod run_stop;
pub mod tiers;
pub mod work_keys;
pub mod workers;

/// A line of the policy with each `{name}` it holds filled in, every one at once so a value that itself
/// holds a mark is left as it is; a mark nothing names stays. The marks are the policy's own syntax, a
/// name of letters, digits and underscores between braces, which no general template format reads as such.
pub fn fill_marks(line: &str, values: &[(&str, &str)]) -> String {
  let mut out = String::new();
  let mut rest = line;

  while let Some(open) = rest.find('{') {
    out.push_str(&rest[..open]);

    let after = &rest[open + 1..];
    let name_end = after
      .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
      .unwrap_or(after.len());
    let value = (name_end > 0 && after[name_end..].starts_with('}'))
      .then(|| values.iter().find(|(name, _)| *name == &after[..name_end]))
      .flatten();

    match value {
      Some((_, value)) => {
        out.push_str(value);
        rest = &after[name_end + 1..];
      }
      None => {
        out.push('{');
        rest = after;
      }
    }
  }

  out + rest
}

/// One part read from its file's text. A file that does not read as its type is a fault of the build,
/// which every part's own test meets before anything runs on it.
pub fn read<T: DeserializeOwned>(file: &str, text: &str) -> T {
  toml::from_str(text).unwrap_or_else(|error| panic!("agency's policy's {file} does not read: {error}"))
}

/// A part of the policy, compiled in from its file under the repository's `policy/` and read once on
/// first use: `policy::part!(pub GATE_ORDER: GateOrder = "gates.toml");`
#[macro_export]
macro_rules! part {
  ($vis:vis $name:ident: $type:ty = $file:literal) => {
    $vis static $name: std::sync::LazyLock<$type> = std::sync::LazyLock::new(|| {
      $crate::read($file, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy/", $file)))
    });
  };
}
