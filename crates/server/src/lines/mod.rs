//! The launch lines as agency composes them (docs/launch-lines.md): the command an agent's
//! resident program stands on, the files that command points at, and the session token its tools are
//! reached with.

pub mod chat;
pub mod claude;
pub mod codex;
pub mod companion;
pub mod dsh;
pub mod openrouter;
pub mod ordered;
pub mod persona;
pub mod place;
pub mod prefix;
pub mod runner;
pub mod stand_in;
pub mod standing;
pub mod token;
pub mod worker;

use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use policy::roles::ROLES;

use crate::Refusal;
use crate::places::Ground;

/// A line that could not be composed, with why.
pub fn fault(detail: impl Into<String>) -> Refusal {
  Refusal::new(StatusCode::INTERNAL_SERVER_ERROR, "line-failed", detail)
}

/// The trees a line is composed out of.
#[derive(Clone, Debug)]
pub struct Trees {
  /// Where the agent documents and the rulebooks are read: the tree the running code ships with.
  pub code: PathBuf,
  /// The agents root: where a composed system prompt is written, and where the configurations a line
  /// is handed that no checkout holds are kept.
  pub agents: PathBuf,
  /// The checkout a run that is no worker's own works in.
  pub source: PathBuf,
  pub data: PathBuf,
  pub home: PathBuf,
}

impl Trees {
  pub fn of(ground: &Ground) -> Self {
    Self {
      code: ground.code_root(),
      agents: ground.roots.agents.clone(),
      source: ground.source.clone(),
      data: ground.roots.data.clone(),
      home: ground.roots.home.clone(),
    }
  }

  /// The folder of one agent's own documents.
  pub fn agent_dir(&self, agent: &str) -> PathBuf {
    self.code.join("prompts").join("agents").join(agent)
  }

  /// A rulebook under the prompts.
  pub fn rulebook_path(&self, name: &str) -> PathBuf {
    self.code.join("prompts").join("rules").join(format!("{name}.md"))
  }

  /// The platform's harness opening, which every line that runs on Claude Code opens its whole system
  /// prompt with (docs/base-prompts.md).
  pub fn harness_path(&self) -> PathBuf {
    self.code.join("prompts").join(HARNESS)
  }
}

// The platform's harness opening, under the prompts.
const HARNESS: &str = "harness.md";

/// A whole system prompt for Claude Code: the platform's harness opening, read out of the code tree, then
/// the line's own text. No line that runs on Claude Code goes out without its harness.
pub fn harnessed(trees: &Trees, text: &str) -> Result<String, Refusal> {
  Ok(format!("{}\n\n{text}", read_required(&trees.harness_path())?))
}

/// A document a line cannot go out without, read whole and trimmed, as the composition reads every
/// agent document.
pub fn read_required(path: &Path) -> Result<String, Refusal> {
  Ok(
    std::fs::read_to_string(path)
      .map_err(|why| fault(format!("{}: {why}", path.display())))?
      .trim()
      .to_owned(),
  )
}

/// A document's `{{name}}` marks filled with what is named, any other left as it stands.
pub fn render(template: &str, values: &[(&str, &str)]) -> String {
  static MARK: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"\{\{(\w+)\}\}").expect("the mark"));

  MARK
    .replace_all(template, |found: &regex::Captures| {
      values
        .iter()
        .find(|(name, _)| *name == &found[1])
        .map_or_else(|| found[0].to_owned(), |(_, value)| (*value).to_owned())
    })
    .into_owned()
}

/// The prompt a role's agents are given before their own (docs/agent-roles.md): the file the policy's
/// role names under `prompts/roles/`, where it names one and the file stands.
pub fn role_prompt(trees: &Trees, role: &str) -> Option<String> {
  let named = ROLES.role(role)?.prompt.as_deref()?;

  std::fs::read_to_string(trees.code.join("prompts").join("roles").join(named)).ok()
}

/// A text written where a line hands it over, published whole by a rename, ending with a newline.
pub fn publish_text(path: &Path, text: &str) -> Result<(), Refusal> {
  let dir = path.parent().ok_or_else(|| fault(format!("{} has no folder", path.display())))?;

  std::fs::create_dir_all(dir).map_err(|why| fault(format!("{}: {why}", dir.display())))?;

  // Each write stages under a name of its own, so two turns publishing one file at once never take each
  // other's.
  let staged = dir.join(format!(
    ".{}.{}",
    path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default(),
    agent_host::disk::staged_suffix()
  ));
  let body = if text.ends_with('\n') { text.to_owned() } else { format!("{text}\n") };

  std::fs::write(&staged, body).map_err(|why| fault(format!("{}: {why}", staged.display())))?;
  std::fs::rename(&staged, path).map_err(|why| fault(format!("{}: {why}", path.display())))
}
