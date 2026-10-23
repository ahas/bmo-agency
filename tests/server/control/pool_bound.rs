//! A lease bound to its launched child is given back once that child has ended (docs/coworker-pool.md,
//! 「Requests and leases」): only an active, bound lease past the launch's grace, never a native call's, and
//! never one whose child still stands.

use std::collections::HashSet;

use agency_server::order_run::dispatch::{BOUND_GRACE_MS, bound_leases, release_ended};

use crate::scratch::Scratch;

#[tokio::test]
async fn a_lease_whose_launched_child_ended_is_given_back_and_one_that_stands_is_kept() {
  let scratch = Scratch::new().await;
  let now = 10 * BOUND_GRACE_MS;
  let old = now - 2 * BOUND_GRACE_MS;

  scratch
    .run(
      r#"INSERT INTO "coworker_pool_requests"
           ("token", "request_key", "owner", "source", "role", "work_key", "provider", "model", "effort", "tier",
            "capacity", "generation", "state", "enqueued_at", "leased_at", "child_id")
         VALUES ('gone', 'k-1', 'order:1', 'worker', 'builder', 'role:builder', 'claude-code', 'm', 'low', 'Frontier', 4, 1, 'active', $1, $1, 'bmo-work-1-build-0-1'),
                ('stands', 'k-2', 'order:1', 'worker', 'builder', 'role:builder', 'claude-code', 'm', 'low', 'Frontier', 4, 1, 'active', $1, $1, 'bmo-work-1-build-1-1'),
                ('fresh', 'k-3', 'order:1', 'worker', 'builder', 'role:builder', 'claude-code', 'm', 'low', 'Frontier', 4, 1, 'active', $2, $2, 'bmo-work-1-build-2-1'),
                ('waits', 'k-4', 'order:1', 'cowork', 'tracer', 'role:tracer', 'codex', 'm', 'low', 'Light', 4, 1, 'waiting', $1, NULL, NULL),
                ('native', 'k-5', 'order:1', 'native', 'tracer', 'role:tracer', 'codex', 'm', 'low', 'Light', 4, 1, 'active', $1, $1, 'toolu_1')"#,
      vec![old.into(), (now - 1).into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "coworker_pool_native"
           ("token", "session", "tool_use_id", "waiter_pid", "waiter_start", "owner_pid", "owner_start")
         VALUES ('native', 's', 'toolu_1', 1, '1', 1, '1')"#,
      vec![],
    )
    .await;

  let leases = bound_leases(scratch.db(), now - BOUND_GRACE_MS).await.unwrap();
  let tokens: Vec<&str> = leases.iter().map(|(token, _)| token.as_str()).collect();

  assert_eq!(tokens, ["gone", "stands"]);

  let standing = HashSet::from(["bmo-work-1-build-1-1".to_owned()]);

  assert_eq!(release_ended(scratch.db(), &leases, &standing).await.unwrap(), 1);
  assert_eq!(release_ended(scratch.db(), &leases, &standing).await.unwrap(), 0, "a finish ends once");

  for (token, expected) in [("gone", false), ("stands", true), ("fresh", true), ("native", true)] {
    assert_eq!(scratch_state(&scratch, token).await == "active", expected, "{token}");
  }
}

async fn scratch_state(scratch: &Scratch, token: &str) -> String {
  use sea_orm::{ConnectionTrait, DbBackend, Statement};

  scratch
    .db()
    .query_one(Statement::from_sql_and_values(
      DbBackend::Postgres,
      r#"SELECT "state" FROM "coworker_pool_requests" WHERE "token" = $1"#,
      vec![token.into()],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "state")
    .unwrap()
}
