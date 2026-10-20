use agency_server::control::claude_accounts::CredentialKind;
use agency_server::control::fallback::*;
use serde_json::json;

#[test]
fn the_chat_s_key_is_the_profile_of_the_chat_s_own_kind() {
  assert_eq!(chat_key(), "bmo");
}

#[test]
fn a_credential_is_sent_as_one_of_its_kinds_and_a_path() {
  let given = credential_given(&json!({ "kind": "endpoint", "path": "/x" })).unwrap();

  assert_eq!(given.kind, Some(CredentialKind::Endpoint));
  assert_eq!(given.path.as_deref(), Some("/x"));
  assert!(credential_given(&json!({ "kind": "directory" })).is_ok());
  assert!(credential_given(&json!({})).unwrap().is_empty());
  assert!(credential_given(&json!({ "kind": null })).is_err());
  assert!(credential_given(&json!({ "kind": "cookie" })).is_err());
  assert!(credential_given(&json!({ "path": 3 })).is_err());
}
