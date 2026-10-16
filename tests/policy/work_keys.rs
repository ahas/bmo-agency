use std::collections::BTreeSet;

use policy::gates::GATE_ORDER;
use policy::work_keys::*;

#[test]
fn every_gate_of_the_gate_order_has_a_profile_and_nothing_else_is_a_gate() {
  let gates: BTreeSet<&str> = WORK_KEYS
    .profile
    .iter()
    .filter(|one| one.kind == "gate")
    .map(|one| one.key.as_str())
    .collect();

  assert_eq!(gates, GATE_ORDER.keys().into_iter().collect());
}

#[test]
fn every_key_is_declared_once_on_a_tier_of_the_ladder() {
  let keys: BTreeSet<&str> = WORK_KEYS.profile.iter().map(|one| one.key.as_str()).collect();

  assert_eq!(keys.len(), WORK_KEYS.profile.len());
  assert!(WORK_KEYS.profile.iter().all(|one| policy::tiers::TIERS.ladder.contains(&one.tier)));
}
