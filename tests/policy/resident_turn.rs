use policy::resident_turn::*;

#[test]
fn the_lead_vouches_for_the_paste_and_the_clocks_keep_their_order() {
  assert!(RESIDENT_TURN.pasted_lead.starts_with("The text pasted after this line"));
  assert!(RESIDENT_TURN.poll_ms < RESIDENT_TURN.redraw_ms);
  assert!(RESIDENT_TURN.gone_looks >= 1);
}
