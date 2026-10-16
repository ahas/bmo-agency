use agency_server::control::tiers::*;

fn mapping(tier: &str, provider: &str, model: &str, answer_tokens: Option<i64>) -> Mapping {
  Mapping {
    provider: provider.into(),
    tier: tier.into(),
    model: model.into(),
    effort: None,
    answer_tokens,
  }
}

#[test]
fn a_mapping_is_held_to_what_holds_whoever_its_provider_is() {
  assert_eq!(check_mapping(&mapping("Simple", "registered", "j3090", Some(2048))), Ok(()));
  assert_eq!(
    check_mapping(&mapping("Huge", "codex", "m", None)).unwrap_err(),
    "Unknown model tier: Huge"
  );
  assert_eq!(check_mapping(&mapping("Simple", "n6", "m", None)).unwrap_err(), "Unknown tier provider");
  assert_eq!(
    check_mapping(&mapping("Simple", "codex", "-m", None)).unwrap_err(),
    "Invalid model identifier"
  );
  assert_eq!(
    check_mapping(&mapping("Simple", "registered", "j3090", Some(3000))).unwrap_err(),
    "Unsupported answer token ceiling"
  );
}
