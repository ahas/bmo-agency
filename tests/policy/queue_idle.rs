use policy::queue_idle::*;

#[test]
fn the_idle_queue_is_looked_at_well_inside_its_minutes_and_its_notice_names_a_code() {
  assert!(QUEUE_IDLE.idle_minutes > 0);
  assert!(QUEUE_IDLE.check_seconds > 0 && (QUEUE_IDLE.check_seconds as i64) < QUEUE_IDLE.idle_minutes * 60);
  assert!(!QUEUE_IDLE.code.is_empty() && !QUEUE_IDLE.ask.is_empty());
}

#[test]
fn a_draft_s_order_carries_every_mark_and_a_bounded_brief() {
  assert!(QUEUE_IDLE.draft_brief_lines > 0 && QUEUE_IDLE.draft_brief_chars > 0);

  for mark in ["{item}", "{title}", "{raised_by}", "{repository}", "{brief}"] {
    assert!(QUEUE_IDLE.draft_order.contains(mark), "{mark}");
  }
}
