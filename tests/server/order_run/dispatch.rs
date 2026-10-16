//! What the pool keeps of a request: a text trimmed and held to its length, and a role and a key held to
//! their shapes.

use agency_server::order_run::dispatch::{KEY, role_name, text};

#[test]
fn a_pool_text_is_trimmed_and_held_to_its_length() {
  assert_eq!(text("  done  ", "pool outcome", 500).unwrap(), "done");
  assert_eq!(text("   ", "pool outcome", 500).unwrap_err(), "Invalid pool outcome");
  assert_eq!(text(&"a".repeat(501), "pool outcome", 500).unwrap_err(), "Invalid pool outcome");
}

#[test]
fn a_role_and_a_key_are_held_to_their_shapes() {
  assert!(role_name("scout").is_ok());
  assert!(role_name("Scout").is_err());
  assert!(KEY.is_match("work-order:12:stage-1:codex:light"));
  assert!(!KEY.is_match(":leading"));
}
