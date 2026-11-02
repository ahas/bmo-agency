//! The roles as the store keeps them and the places a bounded role's turns take (docs/agent-roles.md, 「The
//! roles」): a role's bound read off its row as each turn begins, a changed bound holding from the next turn
//! with the turn already running left to finish, and a role the store does not hold giving nothing.

use std::time::Duration;

use agency_server::agent_roles::{RolePlaces, bound_of_agent, bundles_of, prompt_of};

use crate::support::scratch::Scratch;

async fn bounded(scratch: &Scratch) -> (String, usize) {
  bound_of_agent(scratch.db(), "maker")
    .await
    .unwrap()
    .expect("the maker's role is bounded")
}

#[tokio::test]
async fn a_changed_bound_holds_from_the_next_turn_and_cuts_no_running_one() {
  let scratch = Scratch::new().await;

  scratch
    .run(
      r#"INSERT INTO "agent_roles" ("name", "label", "bundles", "max_concurrent", "prompt", "position", "created_at", "updated_at")
         VALUES ('making', 'Making', '["web.read"]', 1, 'Make it.', 0, 1, 1)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "role") VALUES ('maker', 'worker', 'making')"#,
      vec![],
    )
    .await;

  assert_eq!(bundles_of(scratch.db(), "making").await.unwrap(), ["web.read"]);
  assert_eq!(prompt_of(scratch.db(), "making").await.unwrap().as_deref(), Some("Make it."));
  assert!(bundles_of(scratch.db(), "nobody").await.unwrap().is_empty());

  let places = RolePlaces::default();
  let (role, bound) = bounded(&scratch).await;
  let first = places.take(&role, bound).await.unwrap();

  // A second turn under a bound of one waits.
  let (role, bound) = bounded(&scratch).await;
  assert!(
    tokio::time::timeout(Duration::from_millis(200), places.take(&role, bound))
      .await
      .is_err()
  );

  // The bound raised, the next turn goes at once while the first still runs.
  scratch
    .run(r#"UPDATE "agent_roles" SET "max_concurrent" = 2 WHERE "name" = 'making'"#, vec![])
    .await;

  let (role, bound) = bounded(&scratch).await;
  assert_eq!(bound, 2);
  let second = tokio::time::timeout(Duration::from_millis(200), places.take(&role, bound))
    .await
    .expect("the raised bound lets the next turn in")
    .unwrap();
  let third = tokio::time::timeout(Duration::from_millis(200), places.take(&role, bound))
    .await
    .expect("two places under the new bound")
    .unwrap();

  assert!(
    tokio::time::timeout(Duration::from_millis(200), places.take(&role, bound))
      .await
      .is_err()
  );

  drop((first, second, third));

  // A bound cleared leaves the role unbounded.
  scratch
    .run(r#"UPDATE "agent_roles" SET "max_concurrent" = NULL WHERE "name" = 'making'"#, vec![])
    .await;
  assert_eq!(bound_of_agent(scratch.db(), "maker").await.unwrap(), None);
}
