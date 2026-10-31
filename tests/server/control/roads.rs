use agency_server::control::roads::*;

#[test]
fn a_road_is_the_walk_or_a_provider() {
  for mode in ["auto", "claude-code", "codex", "openrouter", "registered"] {
    assert!(is_route_mode(mode), "{mode}");
  }

  // Laya is no road.
  assert!(!is_route_mode("laya"));
  assert!(!is_route_mode("retired"));
  assert!(!is_route_mode("on"));
}

#[test]
fn a_key_with_no_row_takes_its_profiles_road() {
  assert_eq!(default_mode("output-format"), "registered");
  assert_eq!(default_mode("bmo"), "auto");
  assert_eq!(default_mode("no-such-key"), "auto");
}
