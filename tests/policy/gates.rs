use policy::gates::*;

#[test]
fn the_gate_order_reads_every_gate_once_where_it_first_stands() {
  assert_eq!(
    GATE_ORDER.keys(),
    [
      "system-documents",
      "room-rules",
      "library-knowledge",
      "people-search",
      "situation-rules",
      "sanitizer",
      "output-format",
      "people-standing",
      "document-seek",
      "translator",
      "voice-polish",
      "voice-reading",
    ]
  );
}

#[test]
fn a_companions_turn_passes_its_reference_and_the_outgoing_sanitiser() {
  let gates: Vec<_> = COMPANION_TURN.gates();

  assert_eq!(gates.last(), Some(&("post", "sanitizer")));
  assert_eq!(gates.len(), 6);
}
