use std::collections::BTreeMap;

use agency_server::lines::dsh::*;
use agency_server::lines::ordered::Ordered;
use agency_server::lines::token::TOKEN_VARIABLE;
use policy::providers::deepseek::DEEPSEEK;
use policy::providers::openrouter::OPENROUTER;
use serde_json::Value;

#[test]
fn a_long_task_is_cut_at_spaces_that_join_it_again() {
  // Three bytes and one UTF-16 unit a letter, so the first stretch alone runs past one argument's bytes.
  let task = format!("{} flag -x {}", "€".repeat(WORD_UNITS - 3), "b".repeat(10));
  let words = task_words(&task);

  assert_eq!(words.len(), 2);
  assert!(words.iter().all(|word| word.len() <= WORD_BYTES && !word.starts_with('-')));
  assert_eq!(words.join(" "), task);
  assert_eq!(task_words("€ task\0"), vec!["€ task".to_owned()]);
}

#[test]
fn a_failure_is_dsh_s_own_word_before_the_last_line() {
  assert_eq!(
    failure("dsh: reasoning about it\nError: dsh: no route\nbanner", 1),
    "Error: dsh: no route"
  );
  assert_eq!(failure("dsh: reasoning\nnpm ERR! fetch failed\n", 2), "exit 2: npm ERR! fetch failed");
  assert_eq!(failure("", 3), "exit 3");
}

// The overlay a standing session reads, as a line that grants tools and as one shown none.
fn session_overlay(shown: &'static str) -> Vec<Value> {
  let dir = tempfile::tempdir().unwrap();
  let system = dir.path().join("system.md");

  std::fs::write(&system, "You are the agent.\n").unwrap();

  let run = Composed {
    line: "chat",
    provider: "openrouter",
    system: Some(&system),
    rules: None,
    shown: Some(shown),
    allowed: Some(shown),
    cwd: "/work",
    model: "vendor/model",
    effort: "",
    chosen: false,
    thinking: true,
    mcp: None,
  };
  let patch =
    session_patch(&run, &BTreeMap::new(), dir.path(), OPENROUTER.declared.dsh.as_ref().unwrap()).unwrap();

  serde_json::from_str(&std::fs::read_to_string(&patch).unwrap()).unwrap()
}

#[test]
fn a_session_granting_tools_keeps_dsh_s_defaults_but_the_web_and_the_loops() {
  let overlay = session_overlay("Bash,Read");
  let disabled = |id: &str| overlay.iter().any(|row| row["id"] == id && row["disabled"] == true);

  for id in ["tool-web", "tool-goal", "tool-ralph"] {
    assert!(disabled(id), "{id}");
  }

  for id in ["tool-bash", "tool-fs", "tool-fs-search", "plan-mode", "user-questions"] {
    assert!(!disabled(id), "{id}");
  }

  assert_eq!(overlay[0]["id"], "system-prompt");
  assert!(disabled("agent-instructions"));
  assert!(disabled("session-title-llm"));
}

#[test]
fn a_worker_s_session_on_deepseek_writes_its_rules_after_the_opening_line_on_deepseek_s_own_route() {
  let dir = tempfile::tempdir().unwrap();
  let rules = dir.path().join("rules.md");

  std::fs::write(&rules, "Keep the {{name}} mark as it stands.\n").unwrap();

  let run = Composed {
    line: "worker",
    provider: "deepseek",
    system: None,
    rules: Some(&rules),
    shown: None,
    allowed: None,
    cwd: "/work",
    model: "deepseek-flash",
    effort: "",
    chosen: false,
    thinking: true,
    mcp: None,
  };
  let patch =
    session_patch(&run, &BTreeMap::new(), dir.path(), DEEPSEEK.declared.dsh.as_ref().unwrap()).unwrap();
  let overlay: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&patch).unwrap()).unwrap();
  let row = |id: &str| overlay.iter().find(|row| row["id"] == id).unwrap();
  let persona = row("system-prompt")["config"]["personaPrefix"].as_str().unwrap();

  // dsh reads a `{{name}}` as its own variable and refuses the prompt on one it does not hold, so the
  // rules' braces reach it broken and the opening line's own variable stands.
  assert!(
    persona.starts_with("You are a coding agent powered by the {{model}} model."),
    "{persona}"
  );
  assert!(persona.ends_with("Keep the {\u{2060}{name}} mark as it stands.\n"), "{persona}");
  assert!(row("system-prompt")["config"].get("persona").is_none());

  let route = &row("llm-pi-ai")["config"]["providers"]["deepseek"];

  assert_eq!(route["apiKeyEnv"], "DEEPSEEK_API_KEY");
  assert_eq!(route["baseURL"], "https://api.deepseek.com/v1");
  assert_eq!(row("agent-default-model")["config"]["provider"], "deepseek");
  assert_eq!(row("agent-default-model")["config"]["model"], "deepseek-flash");
}

#[test]
fn a_session_shown_no_tools_switches_every_tool_row_off() {
  let overlay = session_overlay("");
  let disabled = |id: &str| overlay.iter().any(|row| row["id"] == id && row["disabled"] == true);

  for id in [
    "tool-bash",
    "tool-pwsh",
    "tool-jobs",
    "tool-fs",
    "tool-fs-search",
    "tool-skill",
    "tool-subagent-control",
    "tool-subagent-list-agents",
    "tool-subagent",
    "tool-subagent-fork",
    "tool-workflow",
    "tool-todo",
    "tool-goal",
    "tool-ralph",
    "tool-str-replace-editor",
    "tool-web",
    "plan-mode",
    "user-questions",
  ] {
    assert!(disabled(id), "{id}");
  }
}

#[test]
fn the_overlay_writes_the_bearer_as_dsh_reads_it() {
  let overlay =
    Ordered::map(vec![("Authorization", Ordered::Text(format!("{ENVIRONMENT_MARK}{TOKEN_VARIABLE}")))]);

  assert_eq!(
    overlay_text(&overlay).unwrap(),
    "{\n  \"Authorization\": !!js \"`Bearer ${process.env.BMO_MCP_TOKEN}`\"\n}\n"
  );
}
