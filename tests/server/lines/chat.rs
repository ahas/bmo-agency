use std::collections::HashMap;
use std::path::Path;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::lines::Trees;
use agency_server::lines::chat::*;
use agency_server::lines::claude::Session;
use agency_server::places::Ground;
use harness_providers::Machine;
use policy::chat_line::CHAT_LINE;

use super::token::{PARAGRAPH, tool_host};
use crate::scratch::Scratch;

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

// The tree this checkout is, which the chat's documents and rulebooks are read out of.
fn tree() -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize().unwrap()
}

#[test]
fn the_tool_hosts_paragraph_stands_right_after_the_brains_discipline_and_before_the_persona() {
  let home = tempfile::tempdir().unwrap();
  let trees = Trees {
    code: tree(),
    agents: home.path().join("agents"),
    source: home.path().to_path_buf(),
    data: home.path().join("data"),
    home: home.path().to_path_buf(),
  };
  let header = &CHAT_LINE.persona.header;
  let carried =
    chat_system(&trees, Some("the persona"), Some("the brain's discipline"), Some(PARAGRAPH), None)
      .unwrap();
  let without =
    chat_system(&trees, Some("the persona"), Some("the brain's discipline"), None, None).unwrap();

  assert!(
    carried.ends_with(&format!("the brain's discipline\n\n{PARAGRAPH}\n\n{header}\nthe persona")),
    "{carried}"
  );
  assert!(without.ends_with(&format!("the brain's discipline\n\n{header}\nthe persona")));
  assert!(!without.contains(PARAGRAPH));
}

// A chat launch's system prompt as the chat line composes it on a machine whose tool host is the socket
// named.
async fn launched_system(scratch: &Scratch, home: &Path, socket: &Path) -> String {
  let at = |path: &Path| path.to_string_lossy().into_owned();
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), at(home)),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
      ("BMO_DEPLOY_DIR".to_owned(), at(&tree())),
      ("BMO_AGENTS_DIR".to_owned(), at(&home.join("agents"))),
      ("BMO_MCP_HOST_SOCKET".to_owned(), at(socket)),
      ("BMO_MCP_HOST_ISSUER_TOKEN".to_owned(), "issuer".to_owned()),
    ],
    home.join("data"),
  );
  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let ground = Ground::new(&service, &machine, None);
  let launch = ChatLaunch {
    agent: "chat-agent",
    chat_id: -5,
    room: RoomContext {
      chat_id: -5,
      text: "the room's block".into(),
      mark: "[a]".into(),
    },
    via: None,
    resume: None,
    privileged: false,
    ping: false,
    base_persona: Some("the persona"),
    model: Some("claude-model"),
    cwd: Some(&at(home)),
  };
  let run = chat_run(&ground, &machine, &launch, "hello", false).await.unwrap();

  std::fs::read_to_string(&run.system_path).unwrap().trim_end().to_owned()
}

#[tokio::test]
async fn a_chat_launch_carries_the_tool_hosts_paragraph_where_the_host_answers() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let socket = home.path().join("mcp-host.sock");
  let header = &CHAT_LINE.persona.header;

  tool_host(&socket, "issuer");
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "session_scope", "turn_shape", "default_answerer")
         VALUES ('chat-agent', 'chat', 'room', 'window', 1)"#,
      vec![],
    )
    .await;

  let answered = launched_system(&scratch, home.path(), &socket).await;
  let unanswered = launched_system(&scratch, home.path(), &home.path().join("nobody.sock")).await;

  assert!(answered.ends_with(&format!("{PARAGRAPH}\n\n{header}\nthe persona")), "{answered}");
  assert!(!unanswered.contains(PARAGRAPH));
  assert!(unanswered.ends_with(&format!("\n\n{header}\nthe persona")));
}

// The chat's system prompt as this checkout composes it, with no persona and no paragraph of the brain's or
// the tool host's.
fn composed() -> String {
  let home = tempfile::tempdir().unwrap();
  let trees = Trees {
    code: tree(),
    agents: home.path().join("agents"),
    source: home.path().to_path_buf(),
    data: home.path().join("data"),
    home: home.path().to_path_buf(),
  };

  chat_system(&trees, None, None, None, None).unwrap()
}

// The section of the composed prompt under a heading, up to the next heading of the manual's shape.
fn section(system: &str, heading: &str) -> String {
  let at = system
    .find(&format!("\n{heading}:\n"))
    .unwrap_or_else(|| panic!("no {heading} section"));
  let rest = &system[at + heading.len() + 3..];
  let end = regex::Regex::new(r"\n\n[A-Z][A-Z ,()'-]+:")
    .unwrap()
    .find(rest)
    .map_or(rest.len(), |end| end.start());

  rest[..end].to_owned()
}

#[test]
fn no_document_of_the_chat_fixes_the_language_it_answers_in() {
  let system = composed();
  let todo = std::fs::read_to_string(tree().join("prompts/agents/chat/procedures/todo.md")).unwrap();
  let fixed = regex::Regex::new(
    r"(?i)\b(reply|answer|respond|refuse[ds]?|tell[^.\n]{0,40}) in (one short )?(your own )?Korean\b|short Korean (sentence|line)|your own Korean",
  )
  .unwrap();
  let language = section(&system, "THE LANGUAGE YOU ANSWER IN");

  for text in [&system, &todo] {
    assert!(!fixed.is_match(text), "{:?}", fixed.find(text).map(|found| found.as_str()));
  }
  assert!(language.contains("the language they wrote to you in"), "{language}");
  assert!(language.contains("ANSWER LANGUAGE"), "{language}");
  assert!(system.find("THE LANGUAGE YOU ANSWER IN:") < system.find("RESPONSE PROTOCOL:"));
}

#[test]
fn the_room_hears_what_the_agent_will_do_and_never_which_tool_does_it() {
  let system = composed();
  let tools = section(&system, "SPEAKING OF YOUR TOOLS");

  assert!(tools.contains("No message names a tool"), "{tools}");
  assert!(tools.contains("look it up, look into it or translate it"), "{tools}");
  assert!(tools.contains("in your own words"), "{tools}");
  // Nor where it knows a thing from: what it knows is its own knowledge, never something written somewhere.
  assert!(tools.contains("you say as your own knowledge"), "{tools}");
  assert!(
    tools.contains("Never say that something is written, recorded, noted, set or kept somewhere"),
    "{tools}"
  );
  assert!(!tools.contains('"'), "a rule, with no sentence for the agent to repeat: {tools}");
  assert!(system.find("SPEAKING OF YOUR TOOLS:") < system.find("RESPONSE PROTOCOL:"));
}

#[test]
fn the_manual_tells_what_an_answer_to_each_round_of_the_draft_process_is() {
  let system = composed();
  let drafting = section(&system, "DRAFTING A MESSAGE");

  for said in [
    "in rounds",
    "notes",
    "wrote into the brain",
    "still unknown",
    "tools are open in every round",
    "The first round has the most freedom",
    "search for it quietly",
    "Never tell the room that something may be out of date",
    "opens with <<SEND>> is the message",
    "<<SKIP>> anywhere in an answer ends it with nothing sent",
    "In the last round you write the message",
  ] {
    assert!(drafting.contains(said), "{said}: {drafting}");
  }
  assert_eq!(drafting.trim().lines().count(), 1, "one paragraph: {drafting}");
}

#[test]
fn only_the_chat_line_writes_in_a_speaker_s_language_and_every_other_line_writes_english() {
  let language = section(&composed(), "THE LANGUAGE YOU ANSWER IN");
  let prompt = |path: &str| std::fs::read_to_string(tree().join("prompts").join(path)).unwrap();

  assert!(
    language.contains("Only you, answering people in rooms, write in a speaker's language"),
    "{language}"
  );
  assert!(prompt("agents/worker/standing-rules.md").contains("Everything a job writes is English"));
  assert!(prompt("agents/companion/handoff.md").contains("Write it in English"));

  for runner in ["agents/search/branch.md", "agents/browse/task.md"] {
    assert!(prompt(runner).contains("English, "), "{runner}");
    assert!(!prompt(runner).contains("Korean"), "{runner}");
  }

  // The search's result is the document its asker reads, so it is written in the question's language
  // (docs/research-runs.md, 「One run, step by step」).
  assert!(prompt("agents/search/task.md").contains("in the language the question is asked in"));
}

// A chat with no folder of its own works in the deploy tree's link, whose path stays the same across
// releases, so the resident program's record of its session is taken up again after a restart rather
// than a fresh session opened under a new release path (docs/sessions.md, 「A restart hands the session
// over」).
#[tokio::test]
async fn a_chat_with_no_folder_of_its_own_works_in_the_deploy_link() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let deploy = tree();
  let at = |path: &Path| path.to_string_lossy().into_owned();
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), at(home.path())),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
      ("BMO_DEPLOY_DIR".to_owned(), at(&deploy)),
      ("BMO_AGENTS_DIR".to_owned(), at(&home.path().join("agents"))),
      ("BMO_SOURCE_DIR".to_owned(), at(&home.path().join("a-release"))),
    ],
    home.path().join("data"),
  );
  let variables: HashMap<String, String> = [("AGENCY_DEPLOY_DIR".to_owned(), at(&deploy))].into();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let ground = Ground::new(&service, &machine, None);

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "session_scope", "turn_shape", "default_answerer")
         VALUES ('chat-agent', 'chat', 'agent', 'window', 1)"#,
      vec![],
    )
    .await;

  let launch = ChatLaunch {
    agent: "chat-agent",
    chat_id: -5,
    room: RoomContext {
      chat_id: -5,
      text: "the room's block".into(),
      mark: "[a]".into(),
    },
    via: None,
    resume: None,
    privileged: false,
    ping: false,
    base_persona: None,
    model: Some("claude-model"),
    cwd: None,
  };
  let run = chat_run(&ground, &machine, &launch, "hello", false).await.unwrap();

  assert_eq!(run.cwd, at(&deploy));
  assert_ne!(run.cwd, at(&home.path().join("a-release")));
}

#[test]
fn the_tools_paragraph_names_the_programs_own_tools_the_servers_and_the_entries_shown() {
  let paragraph = tools_paragraph(
    &["bash", "read"],
    &["bmo".to_owned(), "job".to_owned()],
    &["bmo/say".to_owned(), "web/web_fetch".to_owned()],
  );

  assert!(paragraph.contains("bash, read"), "{paragraph}");
  assert!(paragraph.contains("bmo, job"), "{paragraph}");
  assert!(paragraph.contains("mcp__bmo__say, mcp__web__web_fetch"), "{paragraph}");
  assert!(paragraph.contains("bmo use --tools") && paragraph.contains("bmo use <tool> --describe"));

  // The read path is named exactly, and every other road to what the system knows is shut.
  for named in [
    "mcp__bmo__brain_list with its subject and type",
    "mcp__bmo__brain_similar with the question's own words",
    "mcp__gate__reference_search, then mcp__gate__reference_piece",
    "never a unit's store, port, database or files",
    "A tool shown to you by name is called as that tool, never through your shell or `bmo use`",
    "Only a hosted tool not shown to you is found with `bmo use --tools`",
    "never written to a file, echoed or named in a command",
  ] {
    assert!(paragraph.contains(named), "{named}: {paragraph}");
  }
}

#[test]
fn the_chats_prompt_names_the_tools_of_the_program_it_is_composed_for_and_the_manual_names_none() {
  let home = tempfile::tempdir().unwrap();
  let trees = Trees {
    code: tree(),
    agents: home.path().join("agents"),
    source: home.path().to_path_buf(),
    data: home.path().join("data"),
    home: home.path().to_path_buf(),
  };
  let parts = SystemParts {
    servers: vec!["bmo".to_owned(), "web".to_owned()],
    ..SystemParts::default()
  };
  let on_claude = parts.compose(&trees, &["Bash", "ListMcpResourcesTool"]).unwrap();
  let on_dsh = parts.compose(&trees, &agency_server::lines::dsh::SESSION_TOOLS).unwrap();
  let manual = std::fs::read_to_string(tree().join("prompts/agents/chat/manual.md")).unwrap();

  assert!(on_claude.contains("own tools are Bash, ListMcpResourcesTool."), "{on_claude}");
  assert!(on_dsh.contains("own tools are bash, read, write, edit"), "{on_dsh}");
  assert!(!on_dsh.contains("ListMcpResourcesTool"));
  assert!(!manual.contains("ListMcpResourcesTool") && !manual.contains("Your tools are Bash"));
}
