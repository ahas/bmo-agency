use agency_server::control::model_tiers::*;
use harness_providers::{Offered, OfferedModel};
use serde_json::json;

#[test]
fn a_key_goes_out_on_its_profiles_line_or_its_runners() {
  assert_eq!(line_of("bmo"), "chat");
  assert_eq!(line_of("output-format"), "gate");
  assert_eq!(line_of("browse"), "browse");
  assert_eq!(line_of("search:thorough"), "search");
  assert_eq!(line_of("role:scout"), "worker");
}

#[test]
fn an_entry_carries_only_the_facts_its_catalogue_tells() {
  let offered = Offered {
    models: vec![OfferedModel {
      id: "m".into(),
      efforts: vec![None, Some("low".into())],
      free: Some(true),
      in_service: None,
    }],
    stale_read_at: Some(1_790_000_000_000.0),
  };

  assert_eq!(
    catalogue_entries("openrouter", &offered),
    [
      json!({ "provider": "openrouter", "model": "m", "efforts": [null, "low"], "free": true, "staleReadAt": 1_790_000_000_000_i64 })
    ]
  );
}
