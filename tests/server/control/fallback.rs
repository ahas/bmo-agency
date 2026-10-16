use agency_server::control::fallback::*;
use serde_json::json;

#[test]
fn the_chat_s_key_is_the_profile_of_the_chat_s_own_kind() {
  assert_eq!(chat_key(), "bmo");
}

#[test]
fn a_credential_is_sent_as_one_of_its_two_kinds_and_a_path() {
  assert!(credential_refusal(&json!({ "kind": "directory", "path": "/x" })).is_ok());
  assert!(credential_refusal(&json!({})).is_ok());
  assert!(credential_refusal(&json!({ "kind": null })).is_err());
  assert!(credential_refusal(&json!({ "kind": "cookie" })).is_err());
  assert!(credential_refusal(&json!({ "path": 3 })).is_err());
}
