//! The registered models: present while the gateway's key file is private and holds a key.

use std::os::unix::fs::PermissionsExt;

use harness_providers::registered::*;
use harness_providers::{Machine, Provider};

#[test]
fn the_key_file_answers_only_while_it_is_private_and_holds_a_key() {
  let dir = tempfile::tempdir().unwrap();
  let machine = Machine::with_env([], dir.path());
  let key = Registered::key_file(&machine);

  assert!(Registered.missing(&machine).is_some());

  std::fs::create_dir_all(key.parent().unwrap()).unwrap();
  std::fs::write(&key, "secret\n").unwrap();
  std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
  assert!(Registered.missing(&machine).is_some());

  std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
  assert_eq!(Registered.missing(&machine), None);

  std::fs::write(&key, "").unwrap();
  assert!(Registered.missing(&machine).is_some());
}
