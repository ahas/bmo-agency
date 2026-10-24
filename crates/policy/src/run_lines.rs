//! The lines agency runs and what each asks of the program that carries it (docs/launch-lines.md).

use std::collections::BTreeMap;

use serde::Deserialize;

/// What one line declares.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunLine {
  /// What its program has to offer for the line to go out on it.
  pub needs: Vec<String>,
  /// Whether its turns follow one another on one session.
  pub keeps_session: bool,
  /// Whether a line a person types into its standing program is handed the composition a turn of the
  /// deployment's would carry.
  #[serde(default)]
  pub composes_typed_lines: bool,
  /// What its runs may reach of the tool host; none for a line handed no tools.
  #[serde(default)]
  pub capabilities: Vec<String>,
  /// The tool host namespaces a run of it is issued; none for a line handed no tools.
  #[serde(default)]
  pub namespaces: Vec<String>,
  /// The tools of its program's own it is never shown.
  #[serde(default)]
  pub tools_denied: Vec<String>,
  /// The placed tool entries, by full name, its namespaces list by name for every run of it.
  #[serde(default)]
  pub shown: Vec<String>,
  /// The deployment's variables every run of it hands its record, each where the deployment sets it.
  #[serde(default)]
  pub variables: Vec<String>,
  /// Whether the brain its runs write is the deployment's agent's own, rather than the runner's.
  #[serde(default)]
  pub brain_agent: bool,
}

/// Every line, by its name.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct RunLines(pub BTreeMap<String, RunLine>);

impl RunLines {
  pub fn line(&self, name: &str) -> Option<&RunLine> {
    self.0.get(name)
  }

  /// The capabilities a run of the line is issued; none for a line the policy does not name.
  pub fn capabilities(&self, name: &str) -> &[String] {
    self.line(name).map_or(&[], |line| &line.capabilities)
  }

  /// The tool host namespaces a run of the line is issued; none for a line the policy does not name.
  pub fn namespaces(&self, name: &str) -> &[String] {
    self.line(name).map_or(&[], |line| &line.namespaces)
  }

  /// The entries a run of the line is shown by full name; none for a line the policy does not name.
  pub fn shown(&self, name: &str) -> &[String] {
    self.line(name).map_or(&[], |line| &line.shown)
  }

  /// The deployment's variables a run of the line hands its record; none for a line the policy does not
  /// name.
  pub fn variables(&self, name: &str) -> &[String] {
    self.line(name).map_or(&[], |line| &line.variables)
  }

  /// Whether the line's namespaces list this placed entry, by its full name, for every run of it; none
  /// for a line the policy does not name.
  pub fn shows(&self, name: &str, entry: &str) -> bool {
    self.line(name).is_some_and(|line| line.shown.iter().any(|one| one == entry))
  }

  /// Whether a person's typed line in the line's standing program is composed.
  pub fn composes_typed_lines(&self, name: &str) -> bool {
    self.line(name).is_some_and(|line| line.composes_typed_lines)
  }

  /// Whether a run of the line writes the deployment's agent's brain rather than its runner's.
  pub fn writes_agent_brain(&self, name: &str) -> bool {
    self.line(name).is_some_and(|line| line.brain_agent)
  }
}

crate::part!(pub RUN_LINES: RunLines = "run-lines.toml");
