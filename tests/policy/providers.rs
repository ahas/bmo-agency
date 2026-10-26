use policy::providers::*;

#[test]
fn every_provider_reads_and_names_the_lines_its_program_carries() {
  for declared in [
    &claude_code::CLAUDE_CODE.declared,
    &codex::CODEX.declared,
    &openrouter::OPENROUTER.declared,
    &registered::REGISTERED.declared,
  ] {
    assert!(!declared.label.is_empty());
    assert!(declared.carries("chat"), "{} carries the chat", declared.label);
    assert!(
      declared.effort.launch == Launch::Either || declared.effort.fault.is_some(),
      "{} says why",
      declared.label
    );
  }
}

#[test]
fn a_launch_rule_refuses_what_it_names() {
  assert_eq!(codex::CODEX.declared.effort.fault(None), Some("This harness requires explicit effort"));
  assert_eq!(codex::CODEX.declared.effort.fault(Some("max")), None);
  assert!(openrouter::OPENROUTER.declared.effort.fault(Some("low")).is_some());
  assert_eq!(openrouter::OPENROUTER.declared.effort.fault(None), None);
  assert_eq!(registered::REGISTERED.declared.effort.fault(Some("thinking")), None);
  assert_eq!(claude_code::CLAUDE_CODE.declared.effort.vocabulary().first(), Some(&None));
}

#[test]
fn deepseek_carries_the_search_line_and_claude_code_alone_the_browse_line() {
  assert!(deepseek::DEEPSEEK.declared.carries("search"));
  assert!(!deepseek::DEEPSEEK.declared.carries("browse"));
  assert!(claude_code::CLAUDE_CODE.declared.carries("browse"));
}
