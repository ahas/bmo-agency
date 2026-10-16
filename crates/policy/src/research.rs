//! What a research run's settings open and what a search run is bound by (docs/launch-lines.md): the part
//! of the research settings agency reads as it launches one.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Research {
  /// The most places one pool bucket may hold.
  pub pool_capacity_most: i64,
  /// The capabilities a run's research settings open, each with the run variable that opens it.
  pub opens: BTreeMap<String, String>,
  pub search: Search,
}

/// What a search run's standing instructions carry after its task: the rulebooks it is bound by in order,
/// the ones given by their deciding section alone, and that section's heading.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
  pub rulebooks: Vec<String>,
  pub on_request: Vec<String>,
  pub applies_section: String,
}

impl Research {
  /// The capabilities a run's settings leave closed, read through `variable`: every one the table names for
  /// a run carrying no research setting at all, and otherwise those whose variable names nothing.
  pub fn withheld<'v>(
    &self,
    carries_settings: bool,
    variable: impl Fn(&str) -> Option<&'v str>,
  ) -> Vec<&str> {
    self
      .opens
      .iter()
      .filter(|(_, opener)| !carries_settings || !variable(opener).is_some_and(opens))
      .map(|(capability, _)| capability.as_str())
      .collect()
  }
}

/// Whether a run variable's value opens the capability it is tied to: it names at least one comma-separated
/// item other than `0`.
pub fn opens(value: &str) -> bool {
  value.split(',').map(str::trim).any(|item| !item.is_empty() && item != "0")
}

crate::part!(pub RESEARCH: Research = "research.toml");
