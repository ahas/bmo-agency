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
  assert_eq!(at(&opened, "--disallowedTools").as_deref(), Some("mcp__job__job_item_create"));
  assert_eq!(at(&opened, "--allowedTools").as_deref(), Some("Bash,Read,mcp__bmo"));
  assert_eq!(at(&opened, "--settings"), Some(settings.to_string_lossy().into_owned()));
  assert_eq!(opened[opened.len() - 2..], ["--session-id".to_owned(), "s-1".to_owned()]);
  assert_eq!(resumed[resumed.len() - 2..], ["--resume".to_owned(), "s-1".to_owned()]);
  assert_eq!(opened[..opened.len() - 2], resumed[..resumed.len() - 2]);

  // The composed prompt stands whole in the program's own place; nothing is appended to the program's.
  let words = agency_server::lines::chat::words(&line(dir.path(), &system)).unwrap();

  assert_eq!(at(&words, "--system-prompt-file"), Some(system.to_string_lossy().into_owned()));
  assert!(!words.iter().chain(&opened).any(|word| word == "--append-system-prompt-file"));
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
  let carried = chat_system(
    &trees,
    &ChatDocuments::default(),
    Some("the persona"),
    Some("the brain's discipline"),
    Some(PARAGRAPH),
    None,
  )
  .unwrap();
  let without = chat_system(
    &trees,
    &ChatDocuments::default(),
    Some("the persona"),
    Some("the brain's discipline"),
    None,
    None,
  )
  .unwrap();

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
  let run = launched(scratch, home, socket).await;

  std::fs::read_to_string(&run.system_path).unwrap().trim_end().to_owned()
}

// A chat launch's run as the chat line composes it on a machine whose tool host is the socket named.
async fn launched(scratch: &Scratch, home: &Path, socket: &Path) -> ChatRun {
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
  chat_run(&ground, &machine, &launch, "hello", false).await.unwrap()
}

// The chat's whole system prompt opens on the platform's harness opening read out of the tree the line is
// composed from, then the role, and goes to Claude Code in the program's own place (docs/agent-documents.md,
// 「The chat agent」).
#[tokio::test]
async fn a_chat_launch_opens_on_the_platform_s_harness_and_is_handed_whole() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let harness = std::fs::read_to_string(tree().join("prompts/harness.md")).unwrap();

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "session_scope", "turn_shape", "default_answerer")
         VALUES ('chat-agent', 'chat', 'room', 'window', 1)"#,
      vec![],
    )
    .await;

  let run = launched(&scratch, home.path(), &home.path().join("nobody.sock")).await;
  let system = std::fs::read_to_string(&run.system_path).unwrap();
  let args = agency_server::lines::chat::args(&run).unwrap();
  let flag = args
    .iter()
    .position(|word| word == "--system-prompt-file")
    .expect("the whole-prompt flag");

  assert!(system.starts_with(&format!("{}\n\n", harness.trim())), "{system}");
  assert!(system.find("ABSOLUTE RULES").is_some_and(|rules| rules > harness.trim().len()));
  assert_eq!(args[flag + 1], run.system_path.to_string_lossy());
  assert!(!args.iter().any(|word| word == "--append-system-prompt-file"));

  // A program that is a harness of its own is handed the role and the persona with no opening of ours.
  let trees = Trees {
    code: tree(),
    agents: home.path().join("agents"),
    source: home.path().to_path_buf(),
    data: home.path().join("data"),
    home: home.path().to_path_buf(),
  };
  let on_dsh = run.system_parts.as_ref().unwrap().compose(&trees, &[]).unwrap();

  assert!(!on_dsh.contains(harness.trim()));
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

  chat_system(&trees, &ChatDocuments::default(), None, None, None, None).unwrap()
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
  assert!(language.contains("is your own judgement"), "{language}");
  assert!(language.contains("never make its writer an English speaker"), "{language}");
  assert!(!language.contains("ANSWER LANGUAGE"), "{language}");
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

  // The duplicate rule is the answer-content rulebook's and the voice of every round the persona header's: each
  // is said once in the prompt, where it belongs.
  for elsewhere in ["counts as said", "drafted and not sent", "in the voice the persona sets"] {
    assert!(!drafting.contains(elsewhere), "{elsewhere}: {drafting}");
  }
}

#[test]
fn a_draft_that_was_never_sent_is_no_message_the_duplicate_rule_holds() {
  let rulebook = std::fs::read_to_string(tree().join("prompts/rules/answer-content.md")).unwrap();

  assert!(rulebook.contains("Only a message actually sent to the room counts as said"));
  assert!(rulebook.contains("Where an\n    answer was drafted and not sent, the next turn says it"));
  assert!(rulebook.contains("Do not do again what was already done"));
}

#[test]
fn the_reply_target_is_the_message_being_answered_and_nothing_forces_it() {
  let capabilities = std::fs::read_to_string(tree().join("prompts/agents/chat/capabilities.md")).unwrap();

  assert!(capabilities.contains("Pin it to the message you are answering with <<REPLY:9166>>"));
  assert!(capabilities.contains("Never pin another message and never the last one you read"));
  assert!(capabilities.contains("If nothing fits, send plain"));
}

#[test]
fn the_deploy_waits_on_plain_quiet_and_a_worker_never_deploys() {
  let trees = tree();
  let text = std::fs::read_to_string(trees.join("testbed.toml")).unwrap();
  let declaration: toml::Value = toml::from_str(&text).unwrap();
  let quiet = declaration["deploy"]["quiet"].as_str().unwrap();

  assert!(!quiet.contains("--with-jobs"), "the deploy counts no job: {quiet}");
  let script = std::fs::read_to_string(trees.join("deploy/quiet")).unwrap();
  assert!(script.contains(" quiet"), "{script}");
  assert!(script.contains("grep -v ':chat:'"), "a chat turn is not spared: {script}");
  // A job's close hands its commit over, and the chat agent puts it live through the hosted deploy tool.
  assert!(
    std::fs::read_to_string(trees.join("prompts/agents/worker/standing-rules.md"))
      .unwrap()
      .contains("A job never deploys.")
  );
}

#[test]
fn a_research_ask_continuing_a_report_names_the_report_it_follows_from() {
  let manual = std::fs::read_to_string(tree().join("prompts/agents/chat/manual.md")).unwrap();
  let line = manual
    .lines()
    .find(|line| line.contains("research/ask"))
    .expect("the research unit's line");

  for said in [
    "the last one you reported in this room",
    "follows the item that report was told for",
    "document that report's document",
    "because one line",
  ] {
    assert!(line.contains(said), "{said}: {line}");
  }
  assert!(!line.contains('"'), "a rule, with no sentence for the agent to repeat: {line}");
}

#[test]
fn a_result_for_a_person_is_in_the_asker_s_language_and_what_agents_read_of_each_other_is_english() {
  let language = section(&composed(), "THE LANGUAGE YOU ANSWER IN");
  let prompt = |path: &str| std::fs::read_to_string(tree().join("prompts").join(path)).unwrap();
  let rule = "A result written for a person is written in the language of the person who asked";

  assert!(
    language.contains("You, answering people in rooms, write in a speaker's language"),
    "{language}"
  );
  // A result written for a person is in the asker's language, which the item's brief names for a run
  // (docs/agent-documents.md, docs/worker-language.md).
  assert!(language.contains(rule), "{language}");
  assert!(
    language.contains("every brief names the language of the person who asked"),
    "{language}"
  );
  assert!(prompt("agents/chat/procedures/todo.md").contains("(`Results in: <language>`)"));
  assert!(prompt("agents/worker/standing-rules.md").contains(rule));
  assert!(prompt("agents/worker/standing-rules.md").contains("Everything else a job writes is English"));
  assert!(
    prompt("agents/worker/plugin/skills/bmo-closing-a-job/SKILL.md")
      .contains("written in the language of the\nperson who asked, as the item's brief names it")
  );
  assert!(prompt("agents/companion/handoff.md").contains("Write it in English"));
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

  // The read path is named exactly, by the names this program calls the tools.
  for named in [
    "mcp__bmo__brain_list with its subject and type",
    "mcp__bmo__brain_similar with the question's own words",
    "mcp__gate__reference_search, then mcp__gate__reference_piece",
    "A tool shown to you by name is called as that tool, never through your shell or `bmo use`",
    "only a hosted tool not shown to you is reached with `bmo use`, as the opening says",
  ] {
    assert!(paragraph.contains(named), "{named}: {paragraph}");
  }
  // How `bmo use` finds, reads and calls a tool, the session's token and the units' stores are the harness
  // opening's to say, once.
  for opening in ["bmo use --tools", "--describe", "token", "store, port, database or files"] {
    assert!(!paragraph.contains(opening), "{opening}: {paragraph}");
  }
}

// The role follows the harness opening as one prompt: what the opening says (how words reach the session, the
// deploy tree against a checkout, how a refusal is read, how a hosted tool is found) stands there alone, and no
// statement the current documents have left behind stays in the role.
#[test]
fn the_role_says_nothing_the_opening_says_and_nothing_no_longer_true() {
  let system = composed();
  let procedures = ["todo.md", "fasttrack.md", "overtime.md"]
    .map(|name| std::fs::read_to_string(tree().join("prompts/agents/chat/procedures").join(name)).unwrap());
  let capabilities = std::fs::read_to_string(tree().join("prompts/agents/chat/capabilities.md")).unwrap();

  for gone in [
    "headless",
    "never from the checkout your shell stands in",
    "never take a tool's name from memory",
    "A tool call that fails says why in its own words",
    "Your program's own plan list",
    "Run privilege line",
    "run-header lines",
    "BMO QUEUE",
    "formatting gate",
    "WORDING.md",
    "a separate check",
    "an item event does, and nothing else does",
    "is not folded",
    "toolbook",
    "the ABSOLUTE RULES at the top",
    "Put it to the owner in his private chat,",
    "`bmo.db`",
  ] {
    assert!(!system.contains(gone), "{gone}");
  }
  for text in procedures.iter().chain([&capabilities]) {
    for gone in ["no room outbox", "no tool that sends", "docs/agent-control.md", "(the harness)"] {
      assert!(!text.contains(gone), "{gone}");
    }
  }
  assert!(!CHAT_LINE.persona.header.contains("at the top of this prompt"));
  assert!(capabilities.contains("SENDING TOOLS ON THIS ROAD"), "{capabilities}");

  // Each of the three kinds of turn says what it asks and what ends it.
  let turn = section(&system, "THE TURN AND THE ROOM");
  for said in [
    "a room's round carries a room's words",
    "it ends with the process's last answer",
    "\"Quiet instruction from the system\" block, closed by END OF QUIET INSTRUCTION",
    "It ends once carried out",
    "A line the owner types into your pane",
    "Your answer to it ends it",
  ] {
    assert!(turn.contains(said), "{said}: {turn}");
  }
  assert!(section(&system, "RESPONSE PROTOCOL").contains("under YOUR QUEUE"));
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

#[test]
fn every_program_s_chat_prompt_is_the_role_then_the_persona_whose_voice_holds_in_every_round() {
  let home = tempfile::tempdir().unwrap();
  let trees = Trees {
    code: tree(),
    agents: home.path().join("agents"),
    source: home.path().to_path_buf(),
    data: home.path().join("data"),
    home: home.path().to_path_buf(),
  };
  let parts = SystemParts {
    persona: Some("To the owner, the persona's own voice.".to_owned()),
    servers: vec!["bmo".to_owned()],
    ..SystemParts::default()
  };
  let programs: [(&str, &[&str]); 2] = [
    ("Claude Code", &["Bash", "ListMcpResourcesTool"]),
    ("dsh", &agency_server::lines::dsh::SESSION_TOOLS),
  ];

  for (program, tools) in programs {
    let system = parts.compose(&trees, tools).unwrap();
    let persona = system
      .find(&CHAT_LINE.persona.header)
      .unwrap_or_else(|| panic!("{program}: no persona"));

    // The role opens with the chat's own opening and its absolute rules, the persona closes it, and its header
    // says the voice it sets holds for every message, a report among them.
    assert!(system.find("ABSOLUTE RULES").is_some_and(|rules| rules < persona), "{program}");
    assert!(system.trim_end().ends_with("To the owner, the persona's own voice."), "{program}");
    assert!(CHAT_LINE.persona.header.contains("in every round"), "{program}");
    assert!(CHAT_LINE.persona.header.contains("the owner included"), "{program}");
    assert!(CHAT_LINE.persona.header.contains("a report"), "{program}");

    // No past case, no translation hint and no gate's rulebook stands in the role.
    for gone in ["real case", "translate it through gate/translate", "formatting gate reads"] {
      assert!(!system.contains(gone), "{program}: {gone}");
    }

    // What the room hears of the tools: never that a thing is written or recorded.
    assert!(system.contains("Never say that something is written, recorded"), "{program}");
  }
}

#[tokio::test]
async fn the_chat_s_servers_file_items_for_its_agent_and_the_job_plugin_serves_no_item_creation() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let at = |path: &Path| path.to_string_lossy().into_owned();
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), at(home.path())),
      ("BMO_AGENTS_DIR".to_owned(), at(&home.path().join("agents"))),
      ("BMO_JOB_PLUGIN".to_owned(), "/opt/bmo-job".to_owned()),
      ("BMO_JOB_URL".to_owned(), "http://127.0.0.1:1".to_owned()),
      ("BMO_AGENCY_URL".to_owned(), "http://127.0.0.1:2".to_owned()),
      ("BMO_EXPLORER_PLUGIN".to_owned(), "/opt/bmo-explorer".to_owned()),
      ("BMO_DATA_DIR".to_owned(), at(&home.path().join("data"))),
      ("AGENCY_CONTROL_TOKEN".to_owned(), "control-token-value".to_owned()),
      ("BMO_JOB_TOKEN".to_owned(), "job-token-value".to_owned()),
    ],
    home.path().join("data"),
  );
  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
  let ground = Ground::new(&service, &machine, None);
  let path = mcp_config(&ground, &Trees::of(&ground), "chat-agent", false).unwrap();
  let text = std::fs::read_to_string(&path).unwrap();

  // No token stands in the configuration by value: each server is handed the file it stands in, and the job
  // plugin, which reads its token from its environment alone, is started behind a shell that reads the file.
  assert!(!text.contains("control-token-value") && !text.contains("job-token-value"), "{text}");
  let config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
  let args = |server: &str| -> Vec<String> {
    config["mcpServers"][server]["args"]
      .as_array()
      .unwrap_or_else(|| panic!("{server}: {config}"))
      .iter()
      .map(|arg| arg.as_str().unwrap().to_owned())
      .collect()
  };
  let job = args("job");
  let agency = args("agency");
  let explorer = args("explorer");

  assert!(path.ends_with("chat.chat-agent.mcp.json"), "{}", path.display());
  let root = at(&home.path().join("data"));
  assert_eq!(explorer, vec!["mcp".to_owned(), "--data-root".to_owned(), root]);
  assert_eq!(config["mcpServers"]["explorer"]["command"].as_str(), Some("/opt/bmo-explorer"));
  assert!(job.windows(2).any(|pair| pair == ["--without", "job_item_create"]), "{job:?}");
  assert!(agency.windows(2).any(|pair| pair == ["--agent", "chat-agent"]), "{agency:?}");
  assert_eq!(config["mcpServers"]["job"]["command"].as_str(), Some("/bin/sh"));
  assert_eq!(job[0], "-c");
  assert_eq!(job[2], "/opt/bmo-job");
  assert_eq!(job[3], "mcp");

  let job_file = config["mcpServers"]["job"]["env"]["BMO_JOB_TOKEN_FILE"].as_str().unwrap();
  let control_file = config["mcpServers"]["agency"]["env"]["AGENCY_CONTROL_TOKEN_FILE"].as_str().unwrap();

  assert_eq!(std::fs::read_to_string(job_file).unwrap(), "job-token-value");
  assert_eq!(std::fs::read_to_string(control_file).unwrap(), "control-token-value");

  // The shell hands the plugin the token the file holds.
  let shell = std::process::Command::new("/bin/sh")
    .args(["-c", &job[1], "/bin/sh", "-c", r#"printf %s "$BMO_JOB_TOKEN""#])
    .env("BMO_JOB_TOKEN_FILE", job_file)
    .output()
    .unwrap();

  assert_eq!(String::from_utf8_lossy(&shell.stdout), "job-token-value");

  // A companion's configuration carries the same servers but for agency's own plugin: it files no item
  // and launches nothing.
  let companion = mcp_config(&ground, &Trees::of(&ground), "companion-agent", true).unwrap();
  let config: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&companion).unwrap()).unwrap();

  assert!(config["mcpServers"]["agency"].is_null(), "{config}");
  assert!(config["mcpServers"]["job"].is_object(), "{config}");
  assert!(config["mcpServers"]["explorer"].is_object(), "{config}");
}
