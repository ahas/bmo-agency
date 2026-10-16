//! `read_rulebook`: the rulebooks a research run's prompt carries only the deciding section of, handed out whole
//! by name out of the tree the tool is placed in (docs/tools.md, 「The rulebook reader」). Which they are is the
//! run's own, read off its record; one it was not handed is refused with the ones it was. A run past its research
//! close still reads one, since it searches nothing.

use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};
use tool_call::{Answer, Asked, Handed, Session};

/// The entry's namespace and name, as the manifest declares them.
pub const NAMESPACE: &str = "web";
pub const NAME: &str = "read_rulebook";

/// The entry's title and description, as the manifest declares them.
pub const TITLE: &str = "Read rulebook";
pub const DESCRIPTION: &str = "Returns one of this run's rulebooks whole. The prompt carries only the section of each that decides whether it applies to this search; once that section says it does, read the rulebook here before searching, and it binds the search as if it stood in the prompt.";

/// The capabilities a session must hold to be shown the entry.
pub const CAPABILITIES: &[&str] = &["web.rulebooks"];

/// The entry's full name, as a call names it.
pub fn entry() -> String {
  format!("{NAMESPACE}/{NAME}")
}

/// What a call hands the entry, as `read_rulebook.json` declares it.
#[derive(Debug, Deserialize)]
pub struct RulebookInput {
  /// The rulebook, named as its file under prompts/rules/ is named, without .md.
  pub name: String,
}

/// The rulebook of this name the record was handed, read out of the tree at `root`.
pub fn read(session: &Session, name: &str, root: &Path) -> Answer {
  let handed = session.rulebooks();

  if !handed.iter().any(|one| one == name) {
    return Answer::failure(format!("{name} is not one of this run's rulebooks: {}", handed.join(", ")));
  }

  let path = root.join("prompts").join("rules").join(format!("{name}.md"));

  match std::fs::read_to_string(&path) {
    Ok(text) => Answer::text(format!("RULEBOOK prompts/rules/{name}.md:\n{text}")),
    Err(why) => Answer::failure(format!("{} could not be read: {why}", path.display())),
  }
}

/// One call of the entry with its arguments, out of the tree at `root`; arguments that do not fit the entry
/// are refused with why, never read.
pub fn call(session: &Session, arguments: Value, root: &Path) -> Result<Answer, String> {
  let input: RulebookInput =
    serde_json::from_value(arguments).map_err(|why| format!("invalid arguments: {NAME}: {why}"))?;

  if input.name.is_empty() {
    return Err(format!("invalid arguments: {NAME}: name must not be empty"));
  }

  Ok(read(session, &input.name, root))
}

/// One request answered out of the tree the tool is placed in: a call of the entry, a call of any other
/// refused as no entry this tool holds, and resource requests answered as a tool that serves none.
pub async fn answer(handed: Handed) -> Result<Value, String> {
  let here = std::env::current_dir().unwrap_or_default();
  let root = tool_call::prompts_tree(&here);
  let answered = match handed.asked {
    Asked::Call {
      entry: asked,
      arguments,
    } if asked == entry() => call(&handed.session, arguments, &root).unwrap_or_else(Answer::failure),
    Asked::Call { entry: asked, .. } => Answer::failure(format!("this executable holds no entry {asked}")),
    Asked::ResourcesList => return Ok(json!({ "resources": [] })),
    Asked::ResourcesRead { .. } => return Ok(json!({ "error": "this executable serves no resources" })),
    Asked::Other => return Err("this executable answers calls alone".to_owned()),
  };

  serde_json::to_value(answered).map_err(|why| format!("the answer could not be written: {why}"))
}
