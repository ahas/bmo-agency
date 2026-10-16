use agency_server::control::roads::*;

#[test]
fn a_road_is_the_walk_a_provider_or_laya() {
  for mode in ["auto", "laya", "claude-code", "codex", "openrouter", "registered"] {
    assert!(is_route_mode(mode), "{mode}");
  }

  assert!(!is_route_mode("j3090"));
  assert!(!is_route_mode("on"));
}

#[test]
fn a_key_with_no_row_takes_its_profiles_road() {
  assert_eq!(default_mode("output-format"), "laya");
  assert_eq!(default_mode("bmo"), "auto");
  assert_eq!(default_mode("no-such-key"), "auto");
}
