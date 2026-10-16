//! A cancellation covers the run asked for, its descendants and what is rooted in it, and is taken back
//! once the work it was raised over has ended.

use agency_server::queue::cancels::{covered, take_back};
use agency_server::queue::running::EXECUTION_CANCEL_PREFIX;
use serde_json::json;

use crate::scratch::Scratch;

#[test]
fn a_cancellation_covers_its_descendants_and_what_is_rooted_in_it_and_nothing_else() {
  let rows = [
    json!({ "id": "child", "parent": "run" }),
    json!({ "id": "grandchild", "parent": "child" }),
    json!({ "id": "rooted", "root": "run" }),
    json!({ "id": "other", "parent": "elsewhere", "root": "place" }),
  ];

  assert_eq!(
    covered(&rows, "run").into_iter().collect::<Vec<_>>(),
    ["child", "grandchild", "rooted", "run"]
  );
}

#[tokio::test]
async fn taking_a_cancellation_back_drops_the_markers_it_raised_and_no_other() {
  let scratch = Scratch::new().await;

  for (id, body) in [
    ("child", json!({ "id": "child", "parent": "run" })),
    ("other", json!({ "id": "other" })),
  ] {
    scratch
      .run(
        r#"INSERT INTO "agent_executions" ("id", "body") VALUES ($1, $2)"#,
        vec![id.into(), body.to_string().into()],
      )
      .await;
  }

  for name in ["run", "child", "other"] {
    scratch
      .run(
        r#"INSERT INTO "settings" ("key", "value") VALUES ($1, '1')"#,
        vec![format!("{EXECUTION_CANCEL_PREFIX}{name}").into()],
      )
      .await;
  }

  let mut dropped = take_back(scratch.db(), "run").await.unwrap();

  dropped.sort();
  assert_eq!(dropped, ["child", "run"]);
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "settings""#, vec![]).await, 1);
}
