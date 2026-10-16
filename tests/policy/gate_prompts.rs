use policy::gate_prompts::GATE_PROMPTS;

#[test]
fn reads_a_ceiling_for_each_gate_told_one() {
  assert!(GATE_PROMPTS.people_search.cap > 0);
  assert!(GATE_PROMPTS.document_seek.cap > 0);
}
