//! The tree agency's tools read `prompts/` from (docs/tools.md, 「A tool's folder」).

use std::path::{Path, PathBuf};

/// The tree holding `prompts/`: the nearest folder above `here` that holds one, which is the deploy tree a tool
/// is placed in; else the tree `BMO_DEPLOY_DIR` names; else the deploy link under the home folder.
pub fn prompts_tree(here: &Path) -> PathBuf {
  if let Some(tree) = here.ancestors().find(|tree| tree.join("prompts").is_dir()) {
    return tree.to_path_buf();
  }

  match std::env::var_os("BMO_DEPLOY_DIR") {
    Some(named) if !named.is_empty() => PathBuf::from(named),
    _ => std::env::var_os("HOME")
      .map_or_else(|| PathBuf::from("/"), PathBuf::from)
      .join(".local/share/bmo/deploy"),
  }
}
