use policy::workers::*;

#[test]
fn the_rules_read_and_the_first_support_worker_follows_the_base() {
  assert!(WORKERS.name_pattern.starts_with('^'));
  assert_eq!(WORKERS.first_support_slot, 2);
}
