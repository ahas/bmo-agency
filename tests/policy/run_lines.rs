use policy::run_lines::*;

#[test]
fn the_worker_line_keeps_its_session_and_leaves_its_tools_to_its_agents_role() {
  let worker = RUN_LINES.line("worker").expect("the worker line is declared");

  assert!(worker.keeps_session);
  assert!(!RUN_LINES.composes_typed_lines("worker"));
  assert!(RUN_LINES.composes_typed_lines("chat"));
  assert!(RUN_LINES.background_apart("chat") && !RUN_LINES.background_apart("worker"));
  assert!(RUN_LINES.capabilities("worker").is_empty());
  assert_eq!(
    RUN_LINES.capabilities("chat"),
    [
      "room.tools",
      "gate.translate",
      "gate.reference",
      "deploy",
      "research.ask",
      "research.owner"
    ]
  );
  assert_eq!(
    RUN_LINES.namespaces("chat"),
    ["bmo", "gate", "eye", "ear", "testbed", "voice", "research"]
  );
  assert!(RUN_LINES.capabilities("gate").is_empty());
}

// A job's pane has nobody to answer the CLI's own question tool, and the session contract cannot carry
// the option it would pick: a job asks the owner through bmo job ask instead (docs/launch-lines.md).
#[test]
fn the_worker_line_keeps_out_the_cli_s_own_question_tool() {
  let worker = RUN_LINES.line("worker").expect("the worker line is declared");

  assert!(worker.tools_denied.iter().any(|tool| tool == "AskUserQuestion"));
}

#[test]
fn only_the_worker_runner_and_chat_lines_show_their_most_called_entries_by_name() {
  assert!(!RUN_LINES.shows("worker", "worker/graph_impact"), "the graph service is retired");
  assert!(RUN_LINES.shows("worker", "web/web_fetch"));
  assert!(!RUN_LINES.shows("worker", "web/research"));
  assert!(RUN_LINES.shows("search", "web/research"));
  assert!(RUN_LINES.shows("browse", "web/web_fetch"));
  assert!(!RUN_LINES.shows("search", "web/reddit"), "the site tools stay in the legacy repository");
  assert!(RUN_LINES.shows("browse", "eye/see"));
  assert!(RUN_LINES.shows("search", "eye/see"));
  assert!(RUN_LINES.shows("worker", "worker/todo_show"));
  assert!(!RUN_LINES.shows("editor", "fragments/fragment_write"), "no editor line stands");
  assert!(!RUN_LINES.shows("chat", "bmo/todo_show"), "the registry holds no such entry");
  assert!(!RUN_LINES.shows("chat", "bmo/todo_list"));
  assert!(RUN_LINES.shows("chat", "bmo/brain_learn"));
  assert!(RUN_LINES.shows("chat", "bmo/brain_similar"));
  assert!(!RUN_LINES.shows("chat", "bmo/memory_add"));
  assert!(RUN_LINES.shows("chat", "bmo/say"));
  for gone in ["bmo/task", "bmo/running", "bmo/todo_add", "bmo/todo_append", "bmo/todo_approve"] {
    assert!(!RUN_LINES.shows("chat", gone), "{gone}: the registry holds no such entry");
  }
  assert!(!RUN_LINES.shows("chat", "bmo/jobs"));
  assert!(!RUN_LINES.shows("chat", "bmo/digest_now"));
  assert!(!RUN_LINES.shows("chat", "bmo/sticker"));
  assert!(!RUN_LINES.shows("chat", "bmo/note_add"));
  assert!(!RUN_LINES.shows("chat", "web/web_search"));
  assert!(!RUN_LINES.shows("chat", "web/web_fetch"));
  assert!(!RUN_LINES.shows("chat", "web/web_search"));
  assert!(RUN_LINES.shows("chat", "gate/translate"));
  assert_eq!(RUN_LINES.variables("chat"), ["BMO_GATE_URL", "BMO_GATE_TOKEN_FILE"]);
  assert!(RUN_LINES.variables("worker").is_empty());
  assert!(!RUN_LINES.shows("supervisor", "worker/todo_show"));
}

#[test]
fn a_runner_line_writes_the_deployment_agent_s_brain_and_shows_the_tools_for_it() {
  for line in ["search", "browse"] {
    assert!(RUN_LINES.capabilities(line).iter().any(|one| one == "room.chat"), "{line}");
    assert!(RUN_LINES.namespaces(line).iter().any(|one| one == "bmo"), "{line}");
    assert!(RUN_LINES.shows(line, "bmo/brain_learn"), "{line}");
    assert!(RUN_LINES.shows(line, "bmo/brain_similar"), "{line}");
    assert!(RUN_LINES.writes_agent_brain(line), "{line}");
  }

  assert!(!RUN_LINES.writes_agent_brain("worker"));
  assert!(!RUN_LINES.writes_agent_brain("chat"));
}

// "the worker line reads the brain of the supervisor its runs work for and writes their lessons there, through
// the namespace its record alone names, its runs staying their own"
#[test]
fn the_worker_line_works_for_a_supervisor_s_brain_and_stays_its_own() {
  assert!(RUN_LINES.works_for_brain("worker"));
  assert!(!RUN_LINES.writes_agent_brain("worker"));
  assert!(RUN_LINES.namespaces("worker").iter().any(|one| one == "bmo"));
  assert!(!RUN_LINES.shows("worker", "bmo/brain_learn"), "reached with bmo use, shown by no name");

  for line in ["search", "browse"] {
    assert!(RUN_LINES.works_for_brain(line), "{line}");
  }

  assert!(!RUN_LINES.works_for_brain("chat"));
}
