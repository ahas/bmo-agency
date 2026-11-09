//! The plugin an agent that names its own skills is handed (docs/role-agents.md, 「What a run reads off its
//! agent」): a folder of its own under the agents root, holding the manifest of its role's plugin and the
//! skills it names alone, each copied from the first of the release's plugins that carries it. It is written
//! whole whenever what it should hold differs from what it holds, so a release that moved a skill's text and
//! an agent whose list changed both reach the next run.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{Trees, fault};
use crate::Refusal;

// Every plugin the release carries: the `plugin` folder of each agent folder under the prompts, by name.
fn plugins(trees: &Trees) -> Vec<PathBuf> {
  let Ok(read) = std::fs::read_dir(trees.code.join("prompts").join("agents")) else {
    return vec![];
  };
  let mut held: Vec<PathBuf> = read
    .filter_map(Result::ok)
    .map(|entry| entry.path().join("plugin"))
    .filter(|plugin| plugin.join("skills").is_dir())
    .collect();

  held.sort();
  held
}

/// The names of every skill a plugin of the release carries, which an agent may name as its own.
pub fn carried(trees: &Trees) -> BTreeSet<String> {
  plugins(trees)
    .iter()
    .filter_map(|plugin| std::fs::read_dir(plugin.join("skills")).ok())
    .flatten()
    .filter_map(Result::ok)
    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect()
}

// Every file under a folder, as its path from the folder, in order.
fn files_under(root: &Path) -> Vec<PathBuf> {
  let mut found = vec![];
  let mut open = vec![root.to_owned()];

  while let Some(dir) = open.pop() {
    let Ok(read) = std::fs::read_dir(&dir) else { continue };

    for entry in read.filter_map(Result::ok) {
      let path = entry.path();

      if path.is_dir() {
        open.push(path);
      } else if let Ok(inside) = path.strip_prefix(root) {
        found.push(inside.to_owned());
      }
    }
  }

  found.sort();
  found
}

// One skill's folder copied whole.
fn copy_skill(from: &Path, to: &Path) -> Result<(), Refusal> {
  for inside in files_under(from) {
    let target = to.join(&inside);

    if let Some(dir) = target.parent() {
      std::fs::create_dir_all(dir).map_err(|why| fault(format!("{}: {why}", dir.display())))?;
    }

    std::fs::copy(from.join(&inside), &target)
      .map_err(|why| fault(format!("{}: {why}", target.display())))?;
  }

  Ok(())
}

/// The plugin folder of the agent `agent`, holding `skills` alone under the manifest of `base`, its role's
/// plugin: each skill from `base` where it carries it, else from the first other plugin of the release that
/// does. A skill no plugin of the release carries is left out, and said. The folder stands under the agents
/// root at the same path run after run, so the command a run goes out on names it the same way every time.
pub fn placed(trees: &Trees, base: &Path, agent: &str, skills: &[String]) -> Result<PathBuf, Refusal> {
  let home = trees.agents.join("role-agents").join(agent);
  let target = home.join("plugin");
  let manifest = base.join(".claude-plugin").join("plugin.json");
  let mut sources: Vec<(String, PathBuf)> = vec![];

  for skill in skills {
    if sources.iter().any(|(held, _)| held == skill) {
      continue;
    }

    let found = std::iter::once(base.to_owned())
      .chain(plugins(trees))
      .map(|plugin| plugin.join("skills").join(skill))
      .find(|folder| folder.is_dir());

    match found {
      Some(folder) => sources.push((skill.clone(), folder)),
      None => eprintln!(
        "agency: the agent {agent} names the skill {skill}, which no plugin of this release carries"
      ),
    }
  }

  // What the folder should hold, as one digest: the manifest and every file of every skill it names.
  let mut digest = Sha256::new();

  digest.update(std::fs::read(&manifest).unwrap_or_default());

  for (skill, folder) in &sources {
    digest.update(skill.as_bytes());

    for inside in files_under(folder) {
      digest.update(inside.to_string_lossy().as_bytes());
      digest.update(std::fs::read(folder.join(&inside)).unwrap_or_default());
    }
  }

  let stamp = format!("{:x}", digest.finalize());
  let stamp_path = home.join("plugin.stamp");

  if target.is_dir() && std::fs::read_to_string(&stamp_path).is_ok_and(|held| held.trim() == stamp) {
    return Ok(target);
  }

  // Written beside its place and moved into it whole.
  let fresh = home.join("plugin.new");
  let _ = std::fs::remove_dir_all(&fresh);

  std::fs::create_dir_all(fresh.join("skills"))
    .map_err(|why| fault(format!("{}: {why}", fresh.display())))?;

  if manifest.is_file() {
    let folder = fresh.join(".claude-plugin");

    std::fs::create_dir_all(&folder).map_err(|why| fault(format!("{}: {why}", folder.display())))?;
    std::fs::copy(&manifest, folder.join("plugin.json"))
      .map_err(|why| fault(format!("{}: {why}", folder.display())))?;
  }

  for (skill, folder) in &sources {
    copy_skill(folder, &fresh.join("skills").join(skill))?;
  }

  let _ = std::fs::remove_dir_all(&target);

  std::fs::rename(&fresh, &target).map_err(|why| fault(format!("{}: {why}", target.display())))?;
  std::fs::write(&stamp_path, &stamp).map_err(|why| fault(format!("{}: {why}", stamp_path.display())))?;

  Ok(target)
}

#[cfg(test)]
mod tests {
  use super::*;

  // A folder of the test's own, gone with it.
  struct Scratch(PathBuf);

  impl Scratch {
    fn new(name: &str) -> Self {
      let root = std::env::temp_dir().join(format!("agency-agent-plugin-{}-{name}", std::process::id()));
      let _ = std::fs::remove_dir_all(&root);

      std::fs::create_dir_all(&root).unwrap();
      Self(root)
    }

    fn path(&self) -> &Path {
      &self.0
    }
  }

  impl Drop for Scratch {
    fn drop(&mut self) {
      let _ = std::fs::remove_dir_all(&self.0);
    }
  }

  fn trees(root: &Path) -> Trees {
    Trees {
      code: root.join("code"),
      agents: root.join("agents"),
      source: root.join("source"),
      data: root.join("data"),
      home: root.join("home"),
    }
  }

  fn skill(root: &Path, plugin: &str, name: &str, text: &str) {
    let folder = root.join("code/prompts/agents").join(plugin).join("plugin/skills").join(name);

    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("SKILL.md"), text).unwrap();
  }

  fn manifest(root: &Path, plugin: &str) {
    let folder = root.join("code/prompts/agents").join(plugin).join("plugin/.claude-plugin");

    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("plugin.json"), format!(r#"{{"name":"{plugin}"}}"#)).unwrap();
  }

  #[test]
  fn an_agents_plugin_holds_the_skills_it_names_alone_under_its_roles_manifest() {
    let root = Scratch::new("holds");
    let trees = trees(root.path());

    manifest(root.path(), "coder");
    skill(root.path(), "coder", "clean", "coder's clean");
    skill(root.path(), "coder", "design", "design");
    skill(root.path(), "worker", "clean", "worker's clean");
    skill(root.path(), "worker", "closing", "closing");

    assert_eq!(
      carried(&trees).into_iter().collect::<Vec<_>>(),
      ["clean", "closing", "design"].map(str::to_owned)
    );

    let base = trees.agent_dir("coder").join("plugin");
    let names = ["clean", "closing", "gone", "clean"].map(str::to_owned);
    let placed_at = placed(&trees, &base, "coder-03", &names).unwrap();

    assert_eq!(placed_at, root.path().join("agents/role-agents/coder-03/plugin"));
    // Its role's own text of a skill stands before another plugin's, and a skill nothing carries is left out.
    assert_eq!(
      std::fs::read_to_string(placed_at.join("skills/clean/SKILL.md")).unwrap(),
      "coder's clean"
    );
    assert_eq!(std::fs::read_to_string(placed_at.join("skills/closing/SKILL.md")).unwrap(), "closing");
    assert!(!placed_at.join("skills/design").exists());
    assert!(!placed_at.join("skills/gone").exists());
    assert_eq!(
      std::fs::read_to_string(placed_at.join(".claude-plugin/plugin.json")).unwrap(),
      r#"{"name":"coder"}"#
    );
  }

  #[test]
  fn an_agents_plugin_is_written_again_only_when_what_it_should_hold_moved() {
    let root = Scratch::new("moved");
    let trees = trees(root.path());

    manifest(root.path(), "coder");
    skill(root.path(), "coder", "clean", "first");

    let base = trees.agent_dir("coder").join("plugin");
    let placed_at = placed(&trees, &base, "coder-03", &["clean".to_owned()]).unwrap();

    // A file of the run's own left in the folder stands while nothing moved.
    std::fs::write(placed_at.join("kept"), "x").unwrap();
    placed(&trees, &base, "coder-03", &["clean".to_owned()]).unwrap();
    assert!(placed_at.join("kept").exists());

    // The release's text of the skill moved, so the folder is written whole again.
    skill(root.path(), "coder", "clean", "second");
    placed(&trees, &base, "coder-03", &["clean".to_owned()]).unwrap();
    assert!(!placed_at.join("kept").exists());
    assert_eq!(std::fs::read_to_string(placed_at.join("skills/clean/SKILL.md")).unwrap(), "second");

    // A list that names none leaves a plugin that carries no skill.
    placed(&trees, &base, "coder-03", &[]).unwrap();
    assert!(placed_at.join("skills").is_dir());
    assert!(!placed_at.join("skills/clean").exists());
  }
}
