use std::path::Path;

use agency_server::control::claude_account_changes::*;

#[test]
fn an_id_is_minted_from_the_name_or_else_stands_as_account() {
  assert_eq!(minted_id("Main Login!", &[]).unwrap(), "main-login");
  assert_eq!(minted_id("--.x", &[]).unwrap(), "account");
  assert_eq!(minted_id("Cuenta-ñ", &["cuenta"]).unwrap(), "cuenta-2");
  assert_eq!(minted_id("ñ ü", &["account"]).unwrap(), "account-2");
  assert_eq!(minted_id("a.b_c-", &[]).unwrap(), "a.b_c");
}

#[test]
fn a_name_is_refused_empty_too_long_or_taken() {
  assert!(name_fault("  ", &[]).is_some());
  assert!(name_fault(&"é".repeat(41), &[]).is_some());
  assert_eq!(
    name_fault(" Spare ", &["Spare"]).unwrap(),
    "There is already an account named \"Spare\"."
  );
  assert_eq!(name_fault("Main", &["Spare"]), None);
}

#[test]
fn a_credential_must_be_an_absolute_path_of_its_own_kind() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("token");
  let credential = |kind: &str, path: &Path| Credential {
    kind: kind.into(),
    path: path.display().to_string(),
  };

  std::fs::write(&file, "t").unwrap();

  assert_eq!(credential_fault(&credential("directory", dir.path())), None);
  assert_eq!(credential_fault(&credential("token", &file)), None);
  assert!(credential_fault(&credential("token", dir.path())).is_some());
  assert!(credential_fault(&credential("directory", &file)).is_some());
  assert!(credential_fault(&credential("directory", Path::new("relative"))).is_some());
  assert!(credential_fault(&credential("directory", &dir.path().join("nowhere"))).is_some());
}
