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
  pub draft: Draft,
}

/// The rounds a research line asks after its first answer, and the questions they ask by name.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
  pub questions: BTreeMap<String, String>,
  /// The lines that ask rounds, each by the work key its runs go out under.
  pub lines: BTreeMap<String, DraftLine>,
}

/// One research line's rounds: the questions in order, how long the process may take, and what a
/// process past that time does.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftLine {
  pub rounds: Vec<String>,
  pub ceiling_seconds: i64,
  pub past_ceiling: PastCeiling,
}

/// What a research process past its ceiling does: ask the final question at once, or return the answer
/// it already wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PastCeiling {
  Final,
  Last,
}

impl PastCeiling {
  /// The word a route and a settings row keep it under.
  pub fn word(self) -> &'static str {
    match self {
      Self::Final => "final",
      Self::Last => "last",
    }
  }

  /// The word read back, none where it names neither.
  pub fn read(word: &str) -> Option<Self> {
    match word {
      "final" => Some(Self::Final),
      "last" => Some(Self::Last),
      _ => None,
    }
  }
}

impl Draft {
  /// One line's rounds, where the policy gives it any.
  pub fn line(&self, line: &str) -> Option<&DraftLine> {
    self.lines.get(line)
  }

  /// A question's text by name.
  pub fn question(&self, name: &str) -> Option<&str> {
    self.questions.get(name).map(String::as_str)
  }

  /// The question every line's list ends in.
  pub const FINAL: &'static str = "final";
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
