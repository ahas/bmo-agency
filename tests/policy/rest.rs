use policy::rest::*;

#[test]
fn a_rest_begins_near_a_full_window() {
  assert!(REST.threshold > 0.5 && REST.threshold <= 1.0 && REST.weekly_threshold <= 1.0);
  assert!(REST.fallback_seconds > 0 && !REST.resting.trim().is_empty());
}
