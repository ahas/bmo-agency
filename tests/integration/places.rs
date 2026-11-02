//! The workers' places in herdr: every agent's head, its room panes, a worker's workbench, folder trust and
//! dev pane, the nodes' workspaces and a renamed worker's checkout, each stood against a herdr of the test's
//! own and agency's ground on a store of the test's own.

use crate::integration::common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use agency_server::peers::Asking;
use agency_server::peers::node::{NodeKind, NodeRow, NodeService};
use agency_server::places::follow::FollowTurns;
use agency_server::places::nodes::{NodeList, NodesMoved};
use agency_server::places::{Ground, Places, checkouts, dev, follow, heads, locks, nodes};
use common::Scratch;
use common::herdr::HerdrModel;
use harness_providers::Machine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// A node service whose list is what the test last set.
#[derive(Default)]
struct Listed(Mutex<Vec<NodeRow>>);

impl NodeService for Listed {
  fn link_up<'a>(&'a self, _node: &'a str) -> Asking<'a, bool> {
    Box::pin(async { Ok(false) })
  }

  fn node_rows(&self) -> Asking<'_, Vec<NodeRow>> {
    let rows = self.0.lock().unwrap().clone();

    Box::pin(async move { Ok(rows) })
  }

  fn ssh_login<'a>(&'a self, _node: &'a str) -> Asking<'a, Option<Vec<String>>> {
    Box::pin(async { Ok(None) })
  }
}

/// A machine of the test's own: a home whose data root is the machine's own, a deploy tree holding the
/// programs the panes run, a herdr on a socket in it, and a node service whose list the test sets.
struct Bench {
  scratch: Scratch,
  herdr: HerdrModel,
  home: tempfile::TempDir,
  places: Places,
  listed: Arc<Listed>,
}

impl Bench {
  async fn new() -> Self {
    let scratch = Scratch::new().await;
    let home = tempfile::tempdir().unwrap();
    let deploy = home.path().join("deploy");

    for script in ["bin/pane-head", "bin/bmo-agency"] {
      std::fs::create_dir_all(deploy.join(script).parent().unwrap()).unwrap();
      std::fs::write(deploy.join(script), format!("// {script}\n")).unwrap();
    }

    let herdr = HerdrModel::stand(home.path().join("herdr.sock"));
    let listed = Arc::new(Listed::default());
    let places = Self::places_on(&scratch, home.path(), &herdr, listed.clone());

    scratch.run(r#"DELETE FROM "agents""#, vec![]).await;

    Self {
      scratch,
      herdr,
      home,
      places,
      listed,
    }
  }

  fn places_on(scratch: &Scratch, home: &Path, herdr: &HerdrModel, listed: Arc<Listed>) -> Places {
    let machine = Machine::with_env(
      [
        ("HOME".to_owned(), home.to_string_lossy().into_owned()),
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ("BMO_DEPLOY_DIR".to_owned(), home.join("deploy").to_string_lossy().into_owned()),
        ("BMO_SOME_SETTING".to_owned(), "carried".to_owned()),
        ("BMO_TASK_LOCKED".to_owned(), "never carried".to_owned()),
      ],
      home.join(".local/share/bmo/data"),
    );
    let mut service = common::service(
      scratch,
      &[("AGENCY_SOURCE_DIR", home.join("source").to_string_lossy().into_owned())],
    );

    service.peers.node = listed;

    let ground = Ground::new(&service, &machine, Some(herdr::Herdr::there(herdr.socket.clone())));

    Places::new(ground, Arc::default())
  }

  fn ground(&self) -> &Ground {
    &self.places.ground
  }

  fn deploy(&self) -> PathBuf {
    self.home.path().join("deploy")
  }

  fn data(&self) -> PathBuf {
    self.home.path().join(".local/share/bmo/data")
  }

  async fn agent(&self, name: &str, kind: &str, label: &str) {
    self
      .scratch
      .run(
        r#"INSERT INTO "agents" ("name", "kind", "label") VALUES ($1, $2, $3)"#,
        vec![name.into(), kind.into(), label.into()],
      )
      .await;
  }

  fn head(&self, agent: &str) -> Vec<String> {
    self.ground().head_command(agent, &self.deploy())
  }

  fn bun_head(&self, agent: &str) -> Vec<String> {
    words(&[
      "/bin/sh",
      "-c",
      r#"while :; do "$1" "$2" "$3"; case $? in 3) exit 0;; esac; sleep 1; done"#,
      &format!("bmo-head-{agent}"),
      "bun",
      &self.deploy().join("scripts/agent-pane.ts").to_string_lossy(),
      agent,
    ])
  }

  fn version(&self, sources: &[&str]) -> String {
    let mut hash = Sha256::new();

    for source in sources {
      hash.update(std::fs::read(self.deploy().join(source)).unwrap());
    }

    format!("{:x}", hash.finalize())[..12].to_owned()
  }
}

fn words(parts: &[&str]) -> Vec<String> {
  parts.iter().map(|part| (*part).to_owned()).collect()
}

fn as_words(command: &[String]) -> Vec<&str> {
  command.iter().map(String::as_str).collect()
}

fn node(name: &str, label: &str, state: &str) -> NodeRow {
  NodeRow {
    node: name.into(),
    label: Some(label.into()),
    kind: NodeKind::Ssh,
    state: state.into(),
    detail: None,
  }
}

// The pass that stands every agent's head on its host, over the heads it finds there.
#[tokio::test]
async fn every_head_on_the_hosts_is_stood_a_head_of_the_other_kind_replaced_once_idle_and_a_renamed_workspace_closed()
 {
  let bench = Bench::new().await;
  let herdr = &bench.herdr;

  bench.agent("worker", "worker", "Main worker").await;
  bench.agent("busy", "gate", "Busy gate").await;

  // The worker's home with a bun head left in it, and a shell a rebuilt herdr left.
  let bun_head = bench.bun_head("worker");
  let (worker_home, _) =
    herdr.workspace("Main worker", json!({ "agent": "worker" }), Some(&as_words(&bun_head)));

  herdr.tab(&worker_home, None);

  // A gate whose bun head is still at work for an open execution keeps it.
  let busy_head = bench.bun_head("busy");
  let (_, busy_pane) =
    herdr.workspace("Busy gate", json!({ "agent": "busy" }), Some(&as_words(&busy_head)));

  bench
    .scratch
    .run(
      r#"INSERT INTO "agent_executions" ("id", "body") VALUES ('e1', $1)"#,
      vec![json!({ "paneId": busy_pane }).to_string().into()],
    )
    .await;

  // The input sanitiser's old name, whose row is gone; a co-work role with no row, on a bun head.
  herdr.workspace("Context gate", json!({ "agent": "hacking-gate" }), Some(&["zsh"]));

  let role_head = bench.bun_head("role");

  herdr.workspace("role", json!({ "agent": "role" }), Some(&as_words(&role_head)));
  heads::stand_all(&bench.places).await;

  assert_eq!(herdr.commands_of("agent", "worker"), [bench.head("worker")]);
  // Its last pane closed took the workspace with it, and the home stood again under its label.
  assert_eq!(
    herdr
      .workspaces()
      .iter()
      .find(|one| one.tokens.as_ref().is_some_and(|tokens| tokens["agent"] == "worker"))
      .map(|one| (one.id == worker_home, one.label.clone())),
    Some((false, "Main worker".to_owned()))
  );
  assert_eq!(herdr.commands_of("agent", "busy"), [busy_head]);
  assert!(herdr.commands_of("agent", "hacking-gate").is_empty());
  assert_eq!(herdr.commands_of("agent", "role"), [bench.head("role")]);

  // A gate with no workspace yet is given one under its label, its token reported.
  bench.agent("fresh", "gate", "New gate").await;
  heads::stand_all(&bench.places).await;

  let fresh = herdr.workspaces().into_iter().find(|one| one.label == "New gate").unwrap();

  assert_eq!(fresh.tokens.unwrap()["agent"], "fresh");
  assert_eq!(herdr.commands_of("agent", "fresh"), [bench.head("fresh")]);
  // Standing again changes nothing.
  assert_eq!(herdr.commands_of("agent", "worker"), [bench.head("worker")]);
  assert_eq!(herdr.workspaces().iter().filter(|one| one.label == "New gate").count(), 1);
}

// The follow pane each room session of a companion stands in.
#[tokio::test]
async fn each_room_session_has_its_follow_pane_named_by_its_room_an_older_one_replaced() {
  let bench = Bench::new().await;
  let herdr = &bench.herdr;

  bench.agent("pal", "companion", "Pal").await;
  bench
    .scratch
    .run(
      r#"INSERT INTO "room_sessions" ("agent", "chat_id", "generation", "opened_at") VALUES ('pal', 7, 1, 1), ('pal', 9, 1, 1)"#,
      vec![],
    )
    .await;

  // A follow pane of an older version, beside the head.
  let head = bench.head("pal");
  let (home, _) = herdr.workspace("Pal", json!({ "agent": "pal" }), Some(&as_words(&head)));

  herdr.tab(
    &home,
    Some(&[
      "/bin/sh",
      "-c",
      r#"exec "$@""#,
      "bmo-standing-follow",
      "/old/bin/bmo",
      "companion",
      "follow",
      "7",
      "--agent",
      "pal",
      "--version=old",
    ]),
  );
  follow::stand_all(bench.ground(), &FollowTurns::default()).await;

  let version = bench.version(&["bin/bmo-agency"]);
  let binary = bench.deploy().join("bin/bmo-agency").to_string_lossy().into_owned();
  let follow = |chat: &str| {
    words(&[
      "/bin/sh",
      "-c",
      r#"exec "$@""#,
      "bmo-standing-follow",
      &binary,
      "companion",
      "follow",
      chat,
      "--agent",
      "pal",
      &format!("--version={version}"),
    ])
  };

  assert_eq!(herdr.commands_of("agent", "pal"), [head.clone(), follow("7"), follow("9")]);

  follow::stand_all(bench.ground(), &FollowTurns::default()).await;
  assert_eq!(herdr.commands_of("agent", "pal"), [head, follow("7"), follow("9")]);
}

// A worker's dev pane, and its folder trusted as `agents/agent/claude-trust.test.ts` holds it for the
// TypeScript side.
#[tokio::test]
async fn a_worker_is_settled_its_dev_pane_follows_its_row_and_comes_back_only_for_a_job_at_work() {
  let bench = Bench::new().await;
  let herdr = &bench.herdr;
  let checkout = bench.home.path().join("checkout");
  // A job's request for the worker's dev server, as the start verb leaves it.
  let wanted = |worker: &str| bench.data().join("workbench").join(worker).join("dev-server.wanted");
  let ask = |worker: &str| {
    std::fs::create_dir_all(bench.data().join("workbench").join(worker)).unwrap();
    std::fs::write(wanted(worker), "asked\n").unwrap();
  };

  std::fs::create_dir_all(&checkout).unwrap();
  ask("w");
  ask("w2");
  bench
    .scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "label", "cwd", "dev_command") VALUES
         ('w', 'worker', 'Worker', $1, 'nuxi dev'),
         ('w2', 'worker', 'Worker 2', $1, NULL),
         ('quiet', 'worker', 'Quiet worker', '/nowhere', 'nuxi dev')"#,
      vec![checkout.to_string_lossy().into_owned().into()],
    )
    .await;
  bench
    .scratch
    .run(r#"UPDATE "agents" SET "support_of" = 'w' WHERE "name" = 'w2'"#, vec![])
    .await;

  dev::settle(bench.ground(), "w").await;

  let binary = bench.deploy().join("bin/bmo-agency").to_string_lossy().into_owned();
  let dev_pane =
    words(&[&binary, "dev-pane", "w", &format!("--version={}", bench.version(&["bin/bmo-agency"]))]);

  assert!(bench.data().join("workbench").join("w").is_dir());
  assert_eq!(herdr.commands_of("agent", "w"), [bench.head("w"), dev_pane.clone()]);

  let config: Value =
    serde_json::from_str(&std::fs::read_to_string(bench.home.path().join(".claude.json")).unwrap())
      .unwrap();

  assert_eq!(config["projects"][checkout.to_string_lossy().as_ref()]["hasTrustDialogAccepted"], true);

  // Settled again, the pane stands as it was.
  dev::settle(bench.ground(), "w").await;
  assert_eq!(herdr.commands_of("agent", "w"), [bench.head("w"), dev_pane.clone()]);

  // The request taken back closes it, and asked again, it stands again.
  std::fs::remove_file(wanted("w")).unwrap();
  dev::settle(bench.ground(), "w").await;
  assert_eq!(herdr.commands_of("agent", "w"), [bench.head("w")]);
  ask("w");
  dev::settle(bench.ground(), "w").await;
  assert_eq!(herdr.commands_of("agent", "w"), [bench.head("w"), dev_pane]);

  // The command taken off the row closes the pane.
  bench
    .scratch
    .run(r#"UPDATE "agents" SET "dev_command" = NULL WHERE "name" = 'w'"#, vec![])
    .await;
  dev::settle(bench.ground(), "w").await;
  assert_eq!(herdr.commands_of("agent", "w"), [bench.head("w")]);

  // Put back after herdr came back: only the worker whose checkout a job holds, a support worker
  // running its base's command.
  bench
    .scratch
    .run(r#"UPDATE "agents" SET "dev_command" = 'nuxi dev' WHERE "name" = 'w'"#, vec![])
    .await;

  let hold = locks::try_hold(&locks::job_hold_path(&bench.data(), &checkout.to_string_lossy())).unwrap();

  dev::dev_panes(bench.ground()).await;
  drop(hold);

  assert!(
    herdr
      .commands_of("agent", "w2")
      .iter()
      .any(|command| command.get(2).map(String::as_str) == Some("w2"))
  );
  assert!(herdr.commands_of("agent", "quiet").is_empty());
  assert!(bench.data().join("workbench").join("quiet").is_dir());
}

// The nodes' workspaces, and `nodes-moved` published when their list moved.
#[tokio::test]
async fn the_nodes_workspaces_stand_last_in_the_lists_order_and_a_list_that_moved_is_published() {
  let bench = Bench::new().await;
  let herdr = &bench.herdr;
  let mut moved = bench.ground().events.subscribe::<NodesMoved>();
  let list = NodeList::default();
  let pc = node("pc", "home pc", "connected");
  let nas = node("nas", "NAS", "unknown");

  *bench.listed.0.lock().unwrap() = vec![pc.clone(), nas.clone()];
  // A gone node's workspace, a rebuilt one that lost its token, and somebody's own after them.
  herdr.workspace("old · connected", json!({ "node": "gone" }), Some(&["zsh"]));
  herdr.workspace(&nodes::label(&pc), Value::Null, None);
  herdr.workspace("mine", Value::Null, None);

  nodes::settle(bench.ground(), &list).await;

  let order: Vec<String> = herdr.workspaces().iter().map(|one| one.label.clone()).collect();

  assert_eq!(order, ["mine".to_owned(), nodes::label(&pc), nodes::label(&nas)]);
  assert_eq!(herdr.commands_of("node", "pc")[0][3], "bmo-node-pc");
  // The first pass has no list before it.
  assert_eq!(moved.ready(), None);

  let renamed = node("pc", "home pc 2", "auth");

  *bench.listed.0.lock().unwrap() = vec![renamed.clone(), nas.clone()];
  nodes::settle(bench.ground(), &list).await;

  assert_eq!(
    herdr.workspaces().iter().map(|one| one.label.clone()).collect::<Vec<_>>(),
    ["mine".to_owned(), nodes::label(&renamed), nodes::label(&nas)]
  );
  assert_eq!(moved.ready(), Some(NodesMoved));

  // A list that stood still publishes nothing.
  nodes::settle(bench.ground(), &list).await;
  assert_eq!(moved.ready(), None);
}

// A renamed worker's checkout.
#[tokio::test]
async fn a_renamed_workers_checkout_moves_with_its_row_unless_a_job_holds_it() {
  let bench = Bench::new().await;
  let repo = bench.home.path().join("repo");
  let from = bench.home.path().join("old-name");
  let to = bench.home.path().join("new-name");
  let git = |args: &[&str]| {
    let done = std::process::Command::new("git")
      .args(args)
      .env("GIT_AUTHOR_NAME", "tester")
      .env("GIT_AUTHOR_EMAIL", "tester@example.com")
      .env("GIT_COMMITTER_NAME", "tester")
      .env("GIT_COMMITTER_EMAIL", "tester@example.com")
      .output()
      .unwrap();

    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
  };
  let at = |path: &Path| path.to_string_lossy().into_owned();

  git(&["init", "-q", &at(&repo)]);
  git(&["-C", &at(&repo), "commit", "-q", "--allow-empty", "-m", "start"]);
  git(&["-C", &at(&repo), "worktree", "add", "-q", "--detach", &at(&from)]);
  bench
    .scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind", "label", "cwd") VALUES ('renamed', 'worker', 'New name', $1)"#,
      vec![at(&from).into()],
    )
    .await;

  let held = locks::try_hold(&locks::lock_path(&bench.data(), &at(&from))).unwrap();

  assert!(!checkouts::move_checkout(bench.ground(), "renamed", &at(&from), &at(&to)).await);
  drop(held);
  assert!(checkouts::move_checkout(bench.ground(), "renamed", &at(&from), &at(&to)).await);
  assert!(to.join(".git").exists() && !from.exists());
  assert_eq!(
    bench
      .scratch
      .count(r#"SELECT COUNT(*) AS "count" FROM "agents" WHERE "cwd" = $1"#, vec![at(&to).into()])
      .await,
    1
  );
  // Done once, the row no longer names the old directory and nothing moves again.
  assert!(!checkouts::move_checkout(bench.ground(), "renamed", &at(&from), &at(&to)).await);
}
