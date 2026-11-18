//! The lines agency runs and what each asks of the program that carries it (docs/launch-lines.md).

use std::collections::BTreeMap;

use serde::Deserialize;

/// What one line declares.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunLine {
  /// What its program has to offer for the line to go out on it.
  pub needs: Vec<String>,
  /// Whether its turns follow one another in one program standing in the agent's pane; a line that stands
  /// none runs each run apart, on the session its role's setting gives it.
  pub keeps_session: bool,
  /// Whether a line a person types into its standing program is handed the composition a turn of the
  /// deployment's would carry.
  #[serde(default)]
  pub composes_typed_lines: bool,
  /// Whether its place's stop hook holds a bot's turn's end while its background work runs, while its final
  /// answer is a tool call written as text, or while that answer uses a banned word.
  #[serde(default)]
  pub holds_stops: bool,
  /// Whether a bot's turn of it ends at its stop while its program's own background work still runs, that
  /// work's finished notice carried to the room it was launched in rather than taken into a turn.
  #[serde(default)]
  pub background_apart: bool,
  /// Whether its programs are jobs', whose shell hook refuses what the job guard refuses of a shell command
  /// and of a file tool's write.
  #[serde(default)]
  pub guards_job_shell: bool,
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
  /// Whether its runs read the brain of the supervisor they work for and write their lessons there, the turn's
  /// agent staying the run's own.
  #[serde(default)]
  pub lessons: bool,
  /// What a supervisor's tool limit names beside the tool host's capabilities: each a capability with the
  /// servers of the line's configuration and the program's own tools that stand under it.
  #[serde(default)]
  pub held: Vec<Held>,
  /// The program's own tools no capability gives that the program of a supervisor whose tool limit names its
  /// capabilities keeps beside those the limit's capabilities hold.
  #[serde(default)]
  pub limited_keeps: Vec<String>,
}

/// One capability a supervisor's tool limit may leave out, and what stands under it outside the tool host.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Held {
  pub capability: String,
  /// The servers of the line's configuration, by the names it gives them.
  #[serde(default)]
  pub servers: Vec<String>,
  /// The program's own tools.
  #[serde(default)]
  pub tools: Vec<String>,
}

/// Every line, by its name.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct RunLines(pub BTreeMap<String, RunLine>);

impl RunLines {
  pub fn line(&self, name: &str) -> Option<&RunLine> {
    self.0.get(name)
  }

  /// What a supervisor's tool limit names beside the tool host's capabilities on the line; none for a line the
  /// policy does not name.
  pub fn held(&self, name: &str) -> &[Held] {
    self.line(name).map_or(&[], |line| &line.held)
  }

  /// The program's own tools no capability gives that a limited supervisor's program keeps on the line; none for
  /// a line the policy does not name.
  pub fn limited_keeps(&self, name: &str) -> &[String] {
    self.line(name).map_or(&[], |line| &line.limited_keeps)
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

  /// Whether the line's place holds a bot's turn's end on its background work, a tool call written as text
  /// or a banned word.
  pub fn holds_stops(&self, name: &str) -> bool {
    self.line(name).is_some_and(|line| line.holds_stops)
  }

  /// Whether a bot's turn of the line ends at its stop while its program's background work runs, that work
  /// carried apart to the room it was launched in (docs/agent-control.md, 「Background work on a line that
  /// carries it apart」).
  pub fn background_apart(&self, name: &str) -> bool {
    self.line(name).is_some_and(|line| line.background_apart)
  }

  /// Whether the line's programs are jobs', whose shell hook refuses what the job guard refuses of a job's
  /// shell command (docs/live-claude-guard.md).
  pub fn guards_job_shell(&self, name: &str) -> bool {
    self.line(name).is_some_and(|line| line.guards_job_shell)
  }

  /// Whether a run of the line writes the deployment's agent's brain rather than its runner's.
  pub fn writes_agent_brain(&self, name: &str) -> bool {
    self.line(name).is_some_and(|line| line.brain_agent)
  }

  /// Whether a run of the line works for a supervisor's brain, which its record names: a line writing the
  /// deployment's agent's brain, or one whose runs read it and write their lessons there.
  pub fn works_for_brain(&self, name: &str) -> bool {
    self.line(name).is_some_and(|line| line.brain_agent || line.lessons)
  }
}

crate::part!(pub RUN_LINES: RunLines = "run-lines.toml");
