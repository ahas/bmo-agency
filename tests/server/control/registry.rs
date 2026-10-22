use agency_server::control::fault::Fault;
use agency_server::control::registry::*;
use harness_machine::gateway;
use policy::providers::registered::REGISTERED;

#[test]
fn an_address_is_held_to_the_chat_completions_path_over_http() {
  assert_eq!(
    checked_url("http://127.0.0.1:9/v1/chat/completions").ok(),
    Some("http://127.0.0.1:9/v1/chat/completions".to_owned())
  );

  for refusedurl in [
    "http://127.0.0.1:9/v1/models",
    "ftp://host/v1/chat/completions",
    "https://user@host/v1/chat/completions",
    "https://host/v1/chat/completions?x=1",
    "not an address",
  ] {
    assert!(
      matches!(checked_url(refusedurl), Err(Fault::Field(field, _)) if field == "url"),
      "{refusedurl}"
    );
  }
}

#[test]
fn a_key_file_name_never_climbs_out_of_env() {
  assert!(checked_key_file(Some("registered/local-a.key")).is_ok());
  assert!(checked_key_file(None).is_ok());
  assert!(checked_key_file(Some("registered/../bot.env")).is_err());
  assert!(checked_key_file(Some("/etc/passwd")).is_err());
}

#[test]
fn a_registration_is_a_service_on_the_gateway_and_a_mapping_its_settings() {
  let service = OwnLlmService {
    name: PROVIDER,
    label: "Local".into(),
    short_label: "Local".into(),
    base_url: gateway::url(None),
    key_file: REGISTERED.presence.key_file.clone(),
    registration: "local-a".into(),
  };
  let thinking = Mapping {
    model: "local-a".into(),
    thinking: None,
    effort: Some("thinking".into()),
    answer_tokens: Some(2_048),
  };

  assert_eq!(
    own_llm_settings(&service, Some(&thinking)),
    OwnLlmSettings {
      provider: "registered",
      model: "local-a".into(),
      effort: Some("thinking".into()),
      thinking: true,
      answer_tokens: 2_048,
      request_wait_ms: None,
      context_tokens: None,
    }
  );
  assert_eq!(
    own_llm_settings(&service, None),
    OwnLlmSettings {
      provider: "registered",
      model: String::new(),
      effort: None,
      thinking: false,
      answer_tokens: DEFAULT_ANSWER_TOKENS,
      request_wait_ms: None,
      context_tokens: None,
    }
  );
}
