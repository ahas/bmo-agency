use std::collections::BTreeSet;

use policy::work_keys::*;

#[test]
fn the_profiles_are_the_chat_agent_the_worker_and_the_gates_their_line_runs() {
  let keys: BTreeSet<&str> = WORK_KEYS.profile.iter().map(|one| one.key.as_str()).collect();
  let gates: BTreeSet<&str> = WORK_KEYS
    .profile
    .iter()
    .filter(|one| one.kind == "gate")
    .map(|one| one.key.as_str())
    .collect();

  assert!(keys.contains("bmo") && keys.contains("job-worker"));
  assert!(!gates.contains("bmo") && !gates.contains("job-worker"));
  assert!(
    WORK_KEYS.profile.iter().all(|one| (one.kind == "gate") == (one.line == "gate")),
    "a gate's line is the gate line and no other kind's is"
  );
}

#[test]
fn every_key_is_declared_once_on_a_tier_of_the_ladder() {
  let keys: BTreeSet<&str> = WORK_KEYS.profile.iter().map(|one| one.key.as_str()).collect();

  assert_eq!(keys.len(), WORK_KEYS.profile.len());
  assert!(WORK_KEYS.profile.iter().all(|one| policy::tiers::TIERS.ladder.contains(&one.tier)));
}

#[test]
fn the_memory_picker_takes_the_pickers_road_with_laya_off() {
  let picker = WORK_KEYS.profile("room-memories").expect("the memory picker has a profile");
  let people = WORK_KEYS.profile("people-search").unwrap();

  assert_eq!((picker.kind.as_str(), picker.line.as_str()), ("gate", "gate"));
  assert_eq!(picker.default_route_mode, people.default_route_mode);
  assert_eq!(picker.tier, people.tier);
  assert_eq!(picker.gate_sampling, people.gate_sampling);
  assert_eq!(picker.laya_mode.as_deref(), Some("off"));
  assert!(!picker.laya_road);
}
