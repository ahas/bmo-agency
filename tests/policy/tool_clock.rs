use policy::tool_clock::*;

#[test]
fn a_call_is_watched_after_its_minimum_and_stopped_after_its_stall() {
  assert!(TOOL_CLOCK.call.stall_seconds > 0);
  assert!(TOOL_CLOCK.call.min_seconds > 0);
  assert!(TOOL_CLOCK.call.look_seconds > 0);
  assert!(TOOL_CLOCK.call.still_every_seconds >= TOOL_CLOCK.call.log_after_seconds / 2);
  assert!(TOOL_CLOCK.call.stop_bound_seconds >= TOOL_CLOCK.call.stop_stall_seconds);
  assert!(TOOL_CLOCK.turn.bound_minutes >= 60);
  assert!(TOOL_CLOCK.turn.cut.contains("{minutes}"));
}
