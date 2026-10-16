//! The herdr backend's session ids: each names its agent and its scope, and reads back as both.

use agency_server::herdr_backend::{id_of, scope_of};
use agent_session::{Scope, SessionId};

#[test]
fn a_session_id_names_its_agent_and_scope_and_reads_back() {
  for scope in [Scope::Agent, Scope::Worker, Scope::Room("-1001".into())] {
    let id = id_of("companion-a", &scope);

    assert_eq!(scope_of(&id), ("companion-a".to_owned(), scope));
  }

  assert_eq!(id_of("worker-a", &Scope::Worker), SessionId("worker-a@worker".into()));
  assert_eq!(id_of("companion-a", &Scope::Room("7".into())), SessionId("companion-a@7".into()));
}
