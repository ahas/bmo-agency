use policy::gates::*;

#[test]
fn a_companion_s_turn_passes_no_gate() {
  assert!(COMPANION_TURN.gates().is_empty(), "no picker and no shelved gate stands in a turn");
}
