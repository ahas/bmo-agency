use std::path::Path;

use agency_server::lines::chat::*;
use agency_server::lines::claude::Session;

// The line's words for a program whose files stand where it is handed them.
fn line<'a>(dir: &'a Path, system: &'a Path) -> Words<'a> {
  Words {
    model: "local-model@node",
    effort: "low",
    system,
    mcp: dir,
    shown: "Bash,Read",
    allowed: "Bash,Read,mcp__bmo",
    host_commands: false,
  }
}

#[test]
fn every_conversational_agent_stands_on_the_chat_line_with_its_own_prompt_and_session() {
  let dir = tempfile::tempdir().unwrap();
  let system = dir.path().join("companion-a.md");
  let settings = dir.path().join("settings.json");

  std::fs::write(&system, "a companion").unwrap();
  std::fs::write(&settings, "{}").unwrap();

  let words = line(dir.path(), &system);
  let opened = standing_words(&words, &settings, &Session::Open("s-1".into())).unwrap();
  let resumed = standing_words(&words, &settings, &Session::Resume("s-1".into())).unwrap();
  let at =
    |args: &[String], flag: &str| args.iter().position(|word| word == flag).map(|at| args[at + 1].clone());

  assert_eq!(opened[0], "claude");
  assert!(!opened.iter().any(|word| word == "-p" || word == "--print"));
  assert!(!opened.iter().any(|word| word == "--disable-slash-commands"));
  assert_eq!(at(&opened, "--system-prompt-file"), Some(system.to_string_lossy().into_owned()));
  assert_eq!(at(&opened, "--permission-mode").as_deref(), Some("dontAsk"));
  assert_eq!(at(&opened, "--allowedTools").as_deref(), Some("Bash,Read,mcp__bmo"));
  assert_eq!(at(&opened, "--settings"), Some(settings.to_string_lossy().into_owned()));
  assert_eq!(opened[opened.len() - 2..], ["--session-id".to_owned(), "s-1".to_owned()]);
  assert_eq!(resumed[resumed.len() - 2..], ["--resume".to_owned(), "s-1".to_owned()]);
  assert_eq!(opened[..opened.len() - 2], resumed[..resumed.len() - 2]);
}
