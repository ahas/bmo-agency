//! The editor line as a run of it is composed (docs/launch-lines.md, 「What is fixed」, editor runs), where
//! agency sends one out — an editor's batch, the keepalive's ping on an editor key: a run apart on a
//! session of its own (docs/agents.md, 「Runs apart」), the program's own instructions, exactly the file
//! tools shown and the fragment server's tools allowed beside them, the standing MCP configuration naming
//! that server, the tier the key is assigned on Claude Code with its agent's own thinking and effort laid
//! on it, and the brain repository to stand in, as the configuration names it.

use std::path::{Path, PathBuf};

use super::Trees;
use super::runner::{handed_config, line_servers};
use super::worker::{effort_of, reasoning_of};
use crate::Refusal;
use crate::control::tiers;
use crate::places::Ground;

/// The line's name.
pub const LINE: &str = "editor";

/// The provider an editor's line is warmed on.
pub const PROVIDER: &str = "claude-code";

/// The file tools an editor is shown, and the fragment server's beside them in what it is allowed.
pub const FILE_TOOLS: &str = "Read,Write,Edit,Glob,Grep";
pub const ALLOWED_TOOLS: &str = "Read,Write,Edit,Glob,Grep,mcp__fragments,mcp__toolbook";

/// The variable a run of the editor line names its fragment desk by.
pub const DESK_VARIABLE: &str = "BMO_FRAGMENT_DESK";

/// One editor key's run as composed, less its prompt, which is put into its program.
#[derive(Clone, Debug)]
pub struct EditorRun {
  pub key: String,
  pub model: String,
  pub effort: String,
  pub tier: String,
  pub mcp_config: PathBuf,
  pub thinking: bool,
  pub cwd: PathBuf,
}

/// The line's standing MCP configuration, written where nothing has written it: the tool host's fragments
/// namespace, and the toolbook.
pub(crate) fn mcp_config(ground: &Ground, trees: &Trees) -> Result<PathBuf, Refusal> {
  handed_config(trees.agents.join("editor.mcp.json"), || Ok(line_servers(ground, &["fragments"])))
}

/// An editor key's run as it goes out on Claude Code in `brain`, the brain repository the configuration
/// names (`Config::brain_root`): the tier its key resolves to there, and the effort and thinking its
/// agent's own choice makes of that; none where its key resolves to nothing or the brain is no repository
/// yet.
pub async fn editor_run(
  ground: &Ground,
  trees: &Trees,
  brain: &Path,
  key: &str,
) -> Result<Option<EditorRun>, Refusal> {
  let mapping = match tiers::resolve_execution(&ground.db, key, PROVIDER, None).await {
    Ok(mapping) => mapping,
    Err(why) => {
      eprintln!("{key}: the line resolves to no model — {why}");
      return Ok(None);
    }
  };
  let cwd = brain.to_path_buf();

  if !cwd.join(".git").exists() {
    eprintln!("{key}: the brain at {} is no repository yet", cwd.display());
    return Ok(None);
  }

  let reasoning = reasoning_of(ground, key).await?;
  let effort = effort_of(
    ground,
    &ground.machine(),
    PROVIDER,
    &mapping.model,
    mapping.effort.as_deref(),
    Some(&reasoning),
  )
  .await?
  .unwrap_or_default();

  Ok(Some(EditorRun {
    key: key.to_owned(),
    model: mapping.model,
    effort,
    tier: mapping.tier,
    mcp_config: mcp_config(ground, trees)?,
    thinking: reasoning.thinking.unwrap_or(true),
    cwd,
  }))
}

impl EditorRun {
  /// The run as Claude Code's words for it, opening the session named.
  pub fn args(&self, session: &str) -> Result<Vec<String>, Refusal> {
    super::claude::apart_args(
      &super::claude::ApartRun {
        line: LINE,
        model: &self.model,
        effort: &self.effort,
        system_prompt: None,
        mcp_config: Some(&self.mcp_config),
        tools_shown: FILE_TOOLS,
        tools_allowed: ALLOWED_TOOLS,
        host_commands: false,
      },
      session,
    )
  }
}
