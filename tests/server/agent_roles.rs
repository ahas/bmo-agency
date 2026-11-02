//! The roles as the store keeps them (docs/agent-roles.md, 「What a role row holds」): every field read off a
//! role's row at every use, the callable ones listed apart, and a role the store does not hold giving nothing.

use agency_server::agent_roles::{bundles_of, callable, named, prompt_of};

use crate::support::scratch::Scratch;

#[tokio::test]
async fn a_roles_row_is_read_whole_and_a_role_the_store_does_not_hold_gives_nothing() {
  let scratch = Scratch::new().await;

  scratch
    .run(
      r#"INSERT INTO "roles" ("name", "label", "bundles", "max_concurrent", "prompt", "position", "created_at",
           "updated_at", "description", "line", "provider_order", "quota_share", "fold_ratio", "callable")
         VALUES ('making', 'Making', '["web.read"]', 1, 'Make it.', 0, 1, 1, 'What making is for.', 'worker',
           '[{"provider":"deepseek","model":"deepseek-flash","effort":"low"}]', 0.05, 0.5, 1)"#,
      vec![],
    )
    .await;

  let making = named(scratch.db(), "making").await.unwrap().unwrap();

  assert_eq!(making.max_concurrent, 1);
  assert_eq!(making.line, "worker");
  assert_eq!(making.order.quota_share, 0.05);
  assert_eq!(making.order.entry("deepseek").and_then(|entry| entry.effort.as_deref()), Some("low"));
  assert_eq!(making.fold_ratio, 0.5);
  assert!(making.callable);
  assert_eq!(bundles_of(scratch.db(), "making").await.unwrap(), ["web.read"]);
  assert_eq!(prompt_of(scratch.db(), "making").await.unwrap().as_deref(), Some("Make it."));
  assert_eq!(
    callable(scratch.db())
      .await
      .unwrap()
      .iter()
      .map(|role| role.name.as_str())
      .collect::<Vec<_>>(),
    ["making"]
  );

  assert!(named(scratch.db(), "nobody").await.unwrap().is_none());
  assert!(bundles_of(scratch.db(), "nobody").await.unwrap().is_empty());
}
