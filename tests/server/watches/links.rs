//! The links to the work nodes: paths as a node reads them, a place marked with its node, and a pass that
//! writes each ready workplace's spec over the SSH login the node service answers and stands its link.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use agency_server::peers::Asking;
use agency_server::peers::node::{NodeRow, NodeService};
use agency_server::watches::links::{
  LinkUnits, Roots, Skipped, deployed_commit, link_unit, mark_place_node, node_path, place_node,
  project_dir, settle,
};
use node_link::spec::{Spec, rewrite};

use crate::support::scratch::Scratch;

#[test]
fn a_path_from_the_home_is_read_against_the_nodes_home() {
  assert_eq!(node_path("~/node-root", Some("/home/tester")), "/home/tester/node-root");
  assert_eq!(node_path("~", Some("/home/tester")), "/home/tester");
  assert_eq!(node_path("/srv/node", Some("/home/tester")), "/srv/node");
  assert_eq!(node_path("~/node-root", None), "~/node-root");
}

#[test]
fn the_deployed_commit_is_read_off_the_record_beside_the_deploy_tree_s_link() {
  let root = tempfile::tempdir().unwrap();

  std::fs::write(root.path().join("deployed.json"), r#"{"commit": "abc123", "subject": "s"}"#).unwrap();

  assert_eq!(deployed_commit(&root.path().join("deploy")).as_deref(), Some("abc123"));
  assert_eq!(deployed_commit(&root.path().join("deploy").join("deploy")), None);
}

#[test]
fn a_working_directory_names_its_records_with_every_other_character_a_hyphen() {
  assert_eq!(project_dir("/home/tester/work-n1", "/r/projects"), "/r/projects/-home-tester-work-n1");
}

#[test]
fn a_place_is_marked_with_its_node_and_unmarked_when_it_comes_back() {
  let scratch = tempfile::tempdir().unwrap();
  let place = scratch.path().join("agent-places").join("worker");

  assert_eq!(place_node(&place), None);
  mark_place_node(&place, Some("n1")).unwrap();
  assert_eq!(place_node(&place), Some("n1".into()));
  mark_place_node(&place, None).unwrap();
  mark_place_node(&place, None).unwrap();
  assert_eq!(place_node(&place), None);
}

/// A node service that logs in to `n1` alone.
struct LogsInToOne;

const LOGIN: [&str; 4] = ["ssh", "-o", "BatchMode=yes", "tester@n1.example"];

impl NodeService for LogsInToOne {
  fn link_up<'a>(&'a self, _node: &'a str) -> Asking<'a, bool> {
    Box::pin(async { Ok(false) })
  }

  fn node_rows(&self) -> Asking<'_, Vec<NodeRow>> {
    Box::pin(async { Ok(vec![]) })
  }

  fn ssh_login<'a>(&'a self, node: &'a str) -> Asking<'a, Option<Vec<String>>> {
    Box::pin(async move { Ok((node == "n1").then(|| LOGIN.map(str::to_owned).to_vec())) })
  }
}

/// Units that start every link asked of them and run none before.
#[derive(Default)]
struct Units {
  started: Mutex<Vec<(String, PathBuf)>>,
}

impl LinkUnits for Units {
  async fn running(&self, _unit: &str) -> bool {
    false
  }

  async fn start(&self, unit: &str, spec: &Path) -> bool {
    self.started.lock().unwrap().push((unit.to_owned(), spec.to_path_buf()));
    true
  }

  async fn stop(&self, _unit: &str) {}

  async fn sync(&self, _node: &str) -> bool {
    false
  }
}

#[tokio::test]
async fn a_ready_workplace_gets_its_spec_over_the_node_services_login_and_one_with_no_route_is_skipped() {
  let scratch = Scratch::new().await;
  let trees = tempfile::tempdir().unwrap();
  let roots = Roots {
    data: trees.path().join("data"),
    deploy: trees.path().join("agency/deploy"),
    cli_deploy: trees.path().join("cli/deploy"),
    agents: trees.path().join("agents"),
    home: trees.path().to_path_buf(),
    mcp_port: 3290,
    config_dir: trees.path().join(".claude"),
    path: "/usr/bin:/bin".into(),
    bun: "bun".into(),
  };

  for node in ["n1", "n2"] {
    scratch
      .run(
        r#"INSERT INTO "node_workplaces" ("node", "root", "home", "state", "created_at", "updated_at")
           VALUES ($1, '~/node-root', '/home/tester', 'ready', 0, 0)"#,
        vec![node.into()],
      )
      .await;
  }
  crate::support::rows::repository(scratch.db(), "worker", "~/work", &[("node", "n1".into())]).await;
  std::fs::create_dir_all(trees.path().join("data/nodes/n1")).unwrap();
  std::fs::write(
    trees.path().join("data/nodes/n1/setup.json"),
    r#"{ "herdrSession": "node-session", "herdrSocket": "/home/tester/.config/herdr/herdr.sock" }"#,
  )
  .unwrap();

  let units = Units::default();
  let settled = settle(scratch.db(), &LogsInToOne, &units, &roots).await.unwrap();

  assert_eq!(settled.started, ["n1"]);
  assert_eq!(
    settled.skipped,
    [Skipped {
      node: "n2".into(),
      why: "n2 has no SSH route".into(),
    }]
  );
  assert_eq!(*units.started.lock().unwrap(), [(link_unit("n1"), roots.spec_path("n1"))]);
  assert_eq!(place_node(&roots.place("worker")).as_deref(), Some("n1"));

  let spec = Spec::read(&roots.spec_path("n1")).unwrap();

  assert_eq!(spec.ssh, LOGIN);
  assert_eq!(spec.root, "/home/tester/node-root");
  assert_eq!(spec.herdr.session.as_deref(), Some("node-session"));
  assert_eq!(spec.push.len(), 1);
  assert!(spec.pull.iter().any(|landing| landing.pull.remote == "/home/tester/work/graph"));
  assert_eq!(spec.deploy, roots.cli_deploy, "the home bridge runs the CLI's tree, not agency's");

  let agency_bin = roots.deploy.join("bin").display().to_string();
  let on_node = rewrite(&agency_bin, spec.rewrite.iter().map(|(from, to)| (from.as_str(), to.as_str())));

  assert_eq!(
    on_node, "/home/tester/node-root/deploy/bin",
    "agency's own tree reads as the node's copy"
  );
}
