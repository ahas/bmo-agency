use policy::run_lines::*;

#[test]
fn the_worker_line_keeps_its_session_and_leaves_its_tools_to_its_agents_role() {
  let worker = RUN_LINES.line("worker").expect("the worker line is declared");

  assert!(worker.keeps_session);
  assert!(!RUN_LINES.composes_typed_lines("worker"));
  assert!(RUN_LINES.composes_typed_lines("chat"));
  assert!(RUN_LINES.capabilities("worker").is_empty());
  assert_eq!(RUN_LINES.capabilities("chat"), ["room.tools", "web.read", "gate.translate"]);
  assert_eq!(RUN_LINES.namespaces("chat"), ["bmo", "web", "gate"]);
  assert!(RUN_LINES.capabilities("gate").is_empty());
}

#[test]
fn only_the_worker_research_and_chat_lines_show_their_most_called_entries_by_name() {
  assert!(RUN_LINES.shows("worker", "worker/graph_impact"));
  assert!(RUN_LINES.shows("worker", "web/web_fetch"));
  assert!(!RUN_LINES.shows("worker", "web/research_branch"));
  assert!(RUN_LINES.shows("search", "web/research_branch"));
  assert!(RUN_LINES.shows("search", "web/reddit"));
  assert!(RUN_LINES.shows("browse", "web/reddit"));
  assert!(RUN_LINES.shows("browse", "web/view_image"));
  assert!(RUN_LINES.shows("worker", "worker/todo_show"));
  assert!(!RUN_LINES.shows("worker", "worker/graph_list"));
  assert!(!RUN_LINES.shows("editor", "fragments/fragment_write"), "no editor line stands");
  assert!(RUN_LINES.shows("chat", "bmo/todo_show"));
  assert!(!RUN_LINES.shows("chat", "bmo/todo_list"));
  assert!(RUN_LINES.shows("chat", "bmo/brain_learn"));
  assert!(RUN_LINES.shows("chat", "bmo/brain_similar"));
  assert!(!RUN_LINES.shows("chat", "bmo/memory_add"));
  assert!(RUN_LINES.shows("chat", "bmo/say"));
  assert!(RUN_LINES.shows("chat", "bmo/task"));
  assert!(RUN_LINES.shows("chat", "bmo/running"));
  assert!(!RUN_LINES.shows("chat", "bmo/jobs"));
  assert!(!RUN_LINES.shows("chat", "bmo/digest_now"));
  assert!(!RUN_LINES.shows("chat", "bmo/sticker"));
  assert!(!RUN_LINES.shows("chat", "bmo/note_add"));
  assert!(!RUN_LINES.shows("chat", "worker/graph_impact"));
  assert!(RUN_LINES.shows("chat", "web/web_search"));
  assert!(RUN_LINES.shows("chat", "web/web_fetch"));
  assert!(!RUN_LINES.shows("chat", "web/reddit"));
  assert!(RUN_LINES.shows("chat", "gate/translate"));
  assert_eq!(RUN_LINES.variables("chat"), ["BMO_GATE_URL", "BMO_GATE_TOKEN_FILE"]);
  assert!(RUN_LINES.variables("worker").is_empty());
  assert!(!RUN_LINES.shows("companion", "worker/graph_impact"));
}
