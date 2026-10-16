//! The workers as the store holds them: the built-in standing whether its row is written or not, a base's
//! support workers brought to its count, a checkout on a node named by the node service's label, and a
//! support worker removed publishing `worker-removed`.

use std::path::Path;

use agency_server::events::Bus;
use agency_server::peers::Asking;
use agency_server::peers::node::{NodeRow, NodeService, Room};
use agency_server::queue::workers::{BUILTIN_WORKER, WorkerRemoved, settle_support, support_name, workers};

use crate::scratch::Scratch;

/// A node service keeping one node, labelled.
struct OneNode;

impl NodeService for OneNode {
  fn machine_room(&self) -> Asking<'_, Room> {
    Box::pin(async { Ok(Room::default()) })
  }

  fn link_up<'a>(&'a self, _node: &'a str) -> Asking<'a, bool> {
    Box::pin(async { Ok(true) })
  }

  fn node_rows(&self) -> Asking<'_, Vec<NodeRow>> {
    Box::pin(async {
      Ok(vec![NodeRow {
        node: "n1".into(),
        label: Some("Bench".into()),
        state: "connected".into(),
        detail: None,
      }])
    })
  }

  fn ssh_login<'a>(&'a self, _node: &'a str) -> Asking<'a, Option<Vec<String>>> {
    Box::pin(async { Ok(None) })
  }
}

#[tokio::test]
async fn the_built_in_stands_first_whether_its_row_is_written_or_not() {
  let scratch = Scratch::new().await;
  let listed = workers(scratch.db(), Path::new("/source")).await.unwrap();

  assert_eq!(listed[0].name, BUILTIN_WORKER);
  assert_eq!(listed[0].cwd, "/source");
}

#[tokio::test]
async fn a_bases_support_workers_follow_its_count_and_a_node_checkout_its_nodes_label() {
  let scratch = Scratch::new().await;
  let events = Bus::default();
  let mut removed = events.subscribe::<WorkerRemoved>();

  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "label", "cwd", "support_count") VALUES ('infra', 'worker', 'Infra', '/repos/infra', 2)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "label", "cwd", "support_of", "node") VALUES ('infra-n1', 'worker', 'old', '/n1/infra', 'infra', 'n1')"#,
      vec![],
    )
    .await;

  let settled = settle_support(scratch.db(), &OneNode, &events, "infra", Path::new("/source"), |_| false)
    .await
    .unwrap();

  assert_eq!(settled.added, [support_name("infra", 2), support_name("infra", 3)]);
  assert!(settled.removed.is_empty());

  let label = |name: &'static str| {
    let db = scratch.db().clone();

    async move {
      store::one(&db, r#"SELECT "label", "cwd" FROM "agents" WHERE "name" = $1"#, vec![name.into()])
        .await
        .unwrap()
        .map(|row| (row.try_get::<String>("", "label").unwrap(), row.try_get::<String>("", "cwd").unwrap()))
    }
  };

  assert_eq!(label("infra-w2").await, Some(("Infra#2".into(), "/repos/infra-w2".into())));
  assert_eq!(label("infra-n1").await.map(|(label, _)| label).as_deref(), Some("Infra@Bench"));

  scratch
    .run(r#"UPDATE "agents" SET "support_count" = 1 WHERE "name" = 'infra'"#, vec![])
    .await;

  let settled = settle_support(scratch.db(), &OneNode, &events, "infra", Path::new("/source"), |_| false)
    .await
    .unwrap();

  assert_eq!(settled.removed, [support_name("infra", 3)]);
  assert_eq!(removed.ready().map(|gone| gone.worker), Some(support_name("infra", 3)));
  assert_eq!(removed.ready(), None);
}
