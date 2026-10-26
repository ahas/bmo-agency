use policy::handoff::*;

#[test]
fn a_handoff_is_waited_on_past_the_receiver_s_bound_and_asked_again_well_inside_it() {
  assert!(HANDOFF.bound_seconds > 0 && HANDOFF.margin_seconds > 0);
  assert!(HANDOFF.retry_seconds > 0 && HANDOFF.retry_seconds < HANDOFF.bound_seconds);
  assert!(!HANDOFF.reasons.lost.is_empty() && !HANDOFF.reasons.replaced.is_empty());
}
