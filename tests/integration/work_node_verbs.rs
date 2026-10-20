//! A work node's verbs of `bmo-agency` (docs/work-nodes.md): the workplace's setup run against a shell
//! standing in for the node and its rows over a scratch store, never a real node.

use crate::common;

use std::sync::Mutex;

use agency_server::peers::node::NoNodeService;
use agency_server::verbs::work_node::{WorkNodeVerb, work_node_on, work_refusal};
use agency_server::verbs::work_node_setup::{
  NODE_CRATES, SetupCheckout, SetupHands, SetupLogin, SetupOutcome, SetupPlace, SetupShell, ShellRun,
  session_socket, set_up_workplace,
};
use common::Scratch;
use futures_util::future::BoxFuture;
use regex::Regex;

// A node answering each command by the first rule it matches, every command written down.
struct StandIn {
  rules: Vec<(Regex, ShellRun)>,
  ran: Mutex<Vec<String>>,
}

impl StandIn {
  fn new(rules: &[(&str, i32, &str, &str)]) -> Self {
    Self {
      rules: rules
        .iter()
        .map(|(pattern, code, stdout, stderr)| {
          (
            Regex::new(pattern).unwrap(),
            ShellRun {
              code: *code,
              stdout: (*stdout).to_owned(),
              stderr: (*stderr).to_owned(),
            },
          )
        })
        .collect(),
      ran: Mutex::new(Vec::new()),
    }
  }

  fn ran(&self) -> Vec<String> {
    self.ran.lock().unwrap().clone()
  }
}

impl SetupShell for StandIn {
  fn run<'a>(&'a self, command: &'a str) -> BoxFuture<'a, ShellRun> {
    self.ran.lock().unwrap().push(command.to_owned());

    let answer = self
      .rules
      .iter()
      .find(|(pattern, _)| pattern.is_match(command))
      .map(|(_, run)| run.clone());

    Box::pin(async move { answer.unwrap_or_default() })
  }
}

struct Hands {
  copy_code: bool,
  said: Mutex<Vec<String>>,
}

impl SetupHands for Hands {
  fn install(&self) -> bool {
    false
  }

  fn copy_code(&self) -> bool {
    self.copy_code
  }

  fn copy<'a>(&'a self, _from: &'a str, _to: &'a str) -> BoxFuture<'a, (bool, String)> {
    Box::pin(async { (true, String::new()) })
  }

  fn add_machine(&self) -> BoxFuture<'_, (bool, String)> {
    Box::pin(async { (true, String::new()) })
  }

  fn say(&self, line: &str) {
    self.said.lock().unwrap().push(line.to_owned());
  }
}

fn hands(copy_code: bool) -> Hands {
  Hands {
    copy_code,
    said: Mutex::new(Vec::new()),
  }
}

const ACCOUNT: &str = "Account A";

fn to_node(path: &str, home: &str, _root: &str) -> String {
  path.replacen("/h", home, 1)
}

fn place(node_token: bool) -> SetupPlace<'static> {
  SetupPlace {
    node: "n1".into(),
    root: "~/.local/share/bmo-node".into(),
    deploy: "/h/deploy".into(),
    checkouts: vec![SetupCheckout {
      worker: "lab-app-n1".into(),
      cwd: "~/lab-app-n1".into(),
      remote: Some("https://example.com/lab-app.git".into()),
    }],
    logins: vec![SetupLogin {
      account: ACCOUNT.into(),
      own_directory: true,
      path: "/h/.claude".into(),
      node_token,
      token_command: format!("/h/deploy/bin/bmo node claude-token '{ACCOUNT}'"),
    }],
    to_node: &to_node,
    destination: "tester@n1.lan".into(),
    herdr: None,
    rust: None,
    bun: None,
    session: "bmo".into(),
  }
}

const HOME: (&str, i32, &str, &str) = ("printf", 0, "/home/tester\n", "");

// "names everything still missing, one thing per line, and reads the herdr session's socket"
#[tokio::test]
async fn names_everything_still_missing_one_thing_per_line_and_reads_the_herdr_session_s_socket() {
  let shell = StandIn::new(&[
    HOME,
    (r"command -v 'bun'|command -v bun\b", 1, "", ""),
    (
      "herdr session list",
      0,
      "name status directory socket\nbmo running /home/tester/.config/herdr/sessions/bmo /home/tester/.config/herdr/sessions/bmo/herdr.sock\n",
      "",
    ),
    (r"test -d .*lab-app-n1/\.git", 0, "", ""),
    ("git push --dry-run", 1, "", "fatal: could not read Username\n"),
    (r"credentials\.json", 1, "", ""),
  ]);
  let outcome = set_up_workplace(&shell, &place(false), &hands(true)).await;

  assert_eq!(outcome.home.as_deref(), Some("/home/tester"));
  assert_eq!(outcome.root.as_deref(), Some("/home/tester/.local/share/bmo-node"));
  assert_eq!(
    outcome.herdr_socket.as_deref(),
    Some("/home/tester/.config/herdr/sessions/bmo/herdr.sock")
  );
  assert_eq!(
    outcome.missing,
    [
      "bun: not installed; run the setup with --install".to_owned(),
      "lab-app-n1: the node cannot push to its remote: fatal: could not read Username".to_owned(),
      format!(
        "login {ACCOUNT}: no token for the work nodes; on this machine, run /h/deploy/bin/bmo node claude-token '{ACCOUNT}'"
      ),
    ]
  );
  // The checkout is trusted under the login's directory on the node.
  assert!(shell.ran().iter().any(|command| command.contains("/deploy/bin/agent-host trust ")
    && command.contains("/home/tester/.claude/.claude.json")));
}

// "an account whose token for the work nodes this machine holds needs no login on the node"
#[tokio::test]
async fn an_account_whose_token_this_machine_holds_needs_no_login_on_the_node() {
  let shell = StandIn::new(&[HOME, (r"credentials\.json", 1, "", "")]);
  let outcome = set_up_workplace(&shell, &place(true), &hands(false)).await;

  assert!(outcome.missing.iter().all(|line| !line.starts_with("login ")));
  assert!(shell.ran().iter().all(|command| !command.contains("credentials.json")));
}

// "a check copies nothing, clones nothing, and says what the setup would have to make"
#[tokio::test]
async fn a_check_copies_nothing_clones_nothing_and_says_what_the_setup_would_have_to_make() {
  let shell = StandIn::new(&[HOME, ("test -x", 1, "", ""), (r"test -d .*\.git", 1, "", "")]);
  let outcome = set_up_workplace(&shell, &place(false), &hands(false)).await;

  assert!(outcome.missing.contains(
    &"code: the deploy tree's copy or its binaries are not on the node; run the setup".to_owned()
  ));
  assert!(
    outcome
      .missing
      .contains(&"lab-app-n1: no checkout at /home/tester/lab-app-n1; run the setup".to_owned())
  );
  assert!(
    shell
      .ran()
      .iter()
      .all(|command| !command.contains("git clone") && !command.contains("cargo build"))
  );
}

// "a node whose login fails names that alone"
#[tokio::test]
async fn a_node_whose_login_fails_names_that_alone() {
  let shell =
    StandIn::new(&[("printf", 255, "", "ssh: connect to host n1.lan port 22: No route to host\n")]);

  assert_eq!(
    set_up_workplace(&shell, &place(false), &hands(true)).await,
    SetupOutcome {
      home: None,
      root: None,
      missing: vec!["login: ssh: connect to host n1.lan port 22: No route to host".into()],
      herdr_socket: None,
    }
  );
}

// "the session's socket is read off herdr's own listing"
#[test]
fn the_session_s_socket_is_read_off_herdr_s_own_listing() {
  assert_eq!(
    session_socket(
      "name status directory socket\ndefault running /h/.config/herdr /h/.config/herdr/herdr.sock\n",
      "bmo"
    ),
    None
  );
  assert_eq!(
    session_socket("bmo running /h/.config/herdr/sessions/bmo /h/x.sock", "bmo").as_deref(),
    Some("/h/x.sock")
  );
}

// "takes each dependency as the root workspace declares it, its members are the crates the setup copies, and
// they reach no crate outside it"
#[test]
fn the_node_build_takes_each_dependency_as_the_root_workspace_declares_it() {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
  let read =
    |path: &str| -> toml::Table { std::fs::read_to_string(root.join(path)).unwrap().parse().unwrap() };
  let dependencies = |table: &toml::Table| -> toml::Table {
    table
      .get("workspace")
      .and_then(|workspace| workspace.get("dependencies"))
      .and_then(toml::Value::as_table)
      .cloned()
      .unwrap_or_default()
  };
  let whole = dependencies(&read("Cargo.toml"));
  let node = dependencies(&read("crates/node-link/node-build.toml"));

  for (name, declared) in &node {
    assert_eq!(Some(declared), whole.get(name), "{name}");
  }

  let build = read("crates/node-link/node-build.toml");
  let members: Vec<&str> = build
    .get("workspace")
    .and_then(|workspace| workspace.get("members"))
    .and_then(toml::Value::as_array)
    .into_iter()
    .flatten()
    .filter_map(toml::Value::as_str)
    .collect();

  assert_eq!(members, NODE_CRATES);

  for krate in NODE_CRATES {
    let manifest = read(&format!("{krate}/Cargo.toml"));

    for section in ["dependencies", "dev-dependencies"] {
      for (name, spec) in manifest.get(section).and_then(toml::Value::as_table).into_iter().flatten() {
        if spec.get("path").is_some() {
          assert!(
            NODE_CRATES.iter().any(|one| one.ends_with(&format!("/{name}"))),
            "{krate} reaches {name}"
          );
        } else {
          assert!(node.contains_key(name), "{krate} takes {name}, which the node's build does not declare");
        }
      }
    }
  }
}

#[test]
fn a_node_takes_work_only_where_the_node_service_names_a_login_and_reserves_it_for_none_but_jobs() {
  assert_eq!(work_refusal("n1", true, None), None);
  assert_eq!(work_refusal("n1", true, Some("jobs+voice")), None);
  assert_eq!(
    work_refusal("n1", true, Some("voice")).as_deref(),
    Some("n1 is reserved for voice and carries no jobs")
  );
  assert_eq!(work_refusal("n1", false, None).as_deref(), Some("n1 has no SSH route to carry a job"));
}

#[tokio::test]
async fn a_workplace_is_listed_its_runs_switched_and_let_go() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let db = scratch.db();
  let listed = work_node_on(db, &NoNodeService, data.path(), None).await.unwrap();

  assert_eq!(listed.lines, ["no workplace on any node"]);
  scratch
    .run(
      r#"INSERT INTO "node_workplaces" ("node", "root", "runs", "state", "detail", "created_at", "updated_at") VALUES ('n1', '~/w', 0, 'missing', 'bun: not installed', 1, 1)"#,
      vec![],
    )
    .await;

  let listed = work_node_on(db, &NoNodeService, data.path(), None).await.unwrap();

  assert_eq!(
    listed.lines,
    [
      "n1           missing     runs off root ~/w · checked never · no link",
      "  - bun: not installed"
    ]
  );
  assert_eq!(
    work_node_on(
      db,
      &NoNodeService,
      data.path(),
      Some(WorkNodeVerb::Runs {
        node: "n1".into(),
        switched: "on".into()
      })
    )
    .await
    .unwrap()
    .lines,
    ["n1: heavy runs on"]
  );
  assert_eq!(
    work_node_on(db, &NoNodeService, data.path(), Some(WorkNodeVerb::Remove { node: "n1".into() }))
      .await
      .unwrap()
      .lines,
    ["n1: its workplace is no longer one; what stands on the node is left as it is"]
  );
  assert!(
    work_node_on(db, &NoNodeService, data.path(), Some(WorkNodeVerb::Remove { node: "n1".into() }))
      .await
      .is_err()
  );
}
