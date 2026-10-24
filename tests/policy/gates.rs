use policy::gates::*;

#[test]
fn a_companion_s_turn_passes_its_reference_and_the_outgoing_sanitiser() {
  let gates: Vec<_> = COMPANION_TURN.gates();

  assert_eq!(gates.last(), Some(&("post", "sanitizer")));
  assert_eq!(gates.len(), 6);
}

#[test]
fn no_gate_stands_twice_in_one_part_of_a_companion_s_turn() {
  let gates: Vec<_> = COMPANION_TURN.gates();
  let keys: Vec<&str> = gates.iter().map(|(_, key)| *key).collect();
  let mut once = keys.clone();

  once.sort_unstable();
  once.dedup();

  assert_eq!(keys.len(), once.len(), "a gate of a companion's turn stands once");
  assert!(COMPANION_TURN.pre.iter().all(|step| !step.places.is_empty()));
  assert!(COMPANION_TURN.post.iter().all(|step| !step.places.is_empty()));
}
