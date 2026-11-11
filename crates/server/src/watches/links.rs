//! The links to the work nodes, kept by agency's server (docs/work-nodes.md, 「The link」): one spec per
//! ready workplace, written from the store whenever the workers or the nodes move, its SSH login asked of
//! the node service; every place of an agent placed on a node marked with its node; and one unit per
//! ready workplace holding the link, started where it is not running and stopped where its workplace is
//! no longer ready. A process on a copy of the data root keeps none, since a link is the machine's unit
//! for that node.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use agent_host::place::Place;
use harness_machine::model_use::with_user_bus;
use harness_providers::Machine;
use node_link::frame::Pull;
use node_link::spec::{Forward, Landing, Push, Spec};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::Value;
use store::all;

use crate::Refusal;
use crate::control::claude_accounts::accounts;
use crate::peers::node::NodeService;

/// The place files that go to the node: what the launcher writes and the program on the node reads.
const PUSHED_PLACE_FILES: [&str; 4] = ["program.json", "settings.json", "run.json", "handed"];

/// What a place's running record and start failure are pulled as.
const PULLED_PLACE_FILES: [&str; 2] = ["running.json", "start-failed.json"];

/// The variables a program on a node is started with, of those this machine's launch gave it.
const NODE_ENV_KEEP: [&str; 15] = [
  "BMO_AGENT_PLACE",
  "BMO_SOURCE_DIR",
  "BMO_MCP_TOKEN",
  "BMO_OWNER_ID",
  "BMO_RUN_NAME",
  "BMO_TASK_LOCKED",
  "BMO_WORK_ORDER_STEP",
  "BMO_TURN_*",
  "CLAUDE_CODE_*",
  "CLAUDE_CONFIG_DIR",
  "HOME",
  "LANG",
  "LC_*",
  "TERM",
  "COLORTERM",
];

/// The file a place on a node carries naming its node, which every question about its processes reads.
const PLACE_NODE_FILE: &str = "node.json";

/// The tool host's port where the environment names none.
const MCP_HOST_PORT: u16 = 3290;

/// The verbs' binary, as the deploy tree's `bin/` holds it.
const VERBS_BINARY: &str = "bmo-agency";

/// The unit one node's link runs under.
pub fn link_unit(node: &str) -> String {
  format!("bmo-node-link-{node}")
}

/// The unit a node's copy of the code is brought up under after a deployment.
pub fn sync_unit(node: &str) -> String {
  format!("bmo-node-sync-{node}")
}

/// The trees and settings of this machine a link's spec and its units are made of.
#[derive(Clone, Debug)]
pub struct Roots {
  pub data: PathBuf,
  /// The deploy tree's link; the release a process reads its code from is `Ground::code_root`'s.
  pub deploy: PathBuf,
  /// The CLI's own deploy tree, whose `bmo` the home bridge runs for a node (docs/work-nodes.md, 「The link」).
  pub cli_deploy: PathBuf,
  pub agents: PathBuf,
  pub home: PathBuf,
  pub mcp_port: u16,
  /// The CLI's own configuration directory, where an account keeps its records unless it names a
  /// directory of its own.
  pub config_dir: PathBuf,
  /// The search path the units are handed.
  pub path: String,
  /// The Bun runtime the TypeScript scripts this machine starts run on.
  pub bun: PathBuf,
}

impl Roots {
  /// The roots as the machine's environment names them, each at the shared installation's place under
  /// the home where it names none, on the deploy tree's link the configuration names.
  pub fn of_machine(machine: &Machine, deploy: PathBuf) -> Self {
    let home = PathBuf::from(machine.var("HOME").unwrap_or("/"));
    let shared = home.join(".local").join("share").join("bmo");

    Self {
      data: machine.data_root().to_path_buf(),
      deploy,
      cli_deploy: home.join(".local").join("share").join("bmo-cli").join("deploy"),
      agents: machine.var("BMO_AGENTS_DIR").map_or_else(|| shared.join("agents"), PathBuf::from),
      mcp_port: machine
        .var("BMO_MCP_HOST_PORT")
        .and_then(|port| port.parse().ok())
        .filter(|&port| port > 0)
        .unwrap_or(MCP_HOST_PORT),
      config_dir: machine.var("CLAUDE_CONFIG_DIR").map_or_else(|| home.join(".claude"), PathBuf::from),
      path: machine.var("PATH").unwrap_or("/usr/bin:/bin").to_owned(),
      bun: machine.program("bun").unwrap_or_else(|| PathBuf::from("bun")),
      home,
    }
  }

  /// Whether the data root is the machine's own rather than a copy a process was pointed at.
  pub fn own_data_root(&self) -> bool {
    self.data == self.home.join(".local").join("share").join("bmo").join("data")
  }

  fn node_dir(&self, node: &str) -> PathBuf {
    self.data.join("nodes").join(node)
  }

  /// Where a node's link spec is written.
  pub fn spec_path(&self, node: &str) -> PathBuf {
    self.node_dir(node).join("link.json")
  }

  /// Where a node's setup leaves its record: its herdr session and socket, and the commit its copy holds.
  pub fn setup_path(&self, node: &str) -> PathBuf {
    self.node_dir(node).join("setup.json")
  }

  /// An agent's place under the data root.
  pub fn place(&self, agent: &str) -> PathBuf {
    Place::of(&self.data, agent).0
  }
}

/// What starts and stops a link's unit, which a test stands in for.
pub trait LinkUnits {
  fn running(&self, unit: &str) -> impl Future<Output = bool> + Send;
  fn start(&self, unit: &str, spec: &Path) -> impl Future<Output = bool> + Send;
  fn stop(&self, unit: &str) -> impl Future<Output = ()> + Send;
  /// The node's copy of the code brought to the deployed commit, apart from the server.
  fn sync(&self, node: &str) -> impl Future<Output = bool> + Send;
}

/// The user's own service manager, started on with the server's roots.
pub struct Systemd<'a>(pub &'a Roots);

impl Systemd<'_> {
  fn env_file(&self) -> String {
    format!("--property=EnvironmentFile=-{}", self.0.data.join("env").join("bot.env").display())
  }
}

async fn systemd(program: &str, args: &[String]) -> bool {
  let mut command = std::process::Command::new(program);

  command.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit());
  with_user_bus(&mut command);

  tokio::process::Command::from(command)
    .status()
    .await
    .is_ok_and(|status| status.success())
}

impl LinkUnits for Systemd<'_> {
  async fn running(&self, unit: &str) -> bool {
    systemd(
      "systemctl",
      &["--user".into(), "is-active".into(), "--quiet".into(), format!("{unit}.service")],
    )
    .await
  }

  async fn start(&self, unit: &str, spec: &Path) -> bool {
    let binary = self.0.deploy.join("bin").join("node-link");

    if !binary.exists() {
      return false;
    }

    systemd(
      "systemd-run",
      &[
        "--user".into(),
        "--quiet".into(),
        "--collect".into(),
        format!("--unit={unit}"),
        "--property=Restart=always".into(),
        "--property=RestartSec=5".into(),
        self.env_file(),
        format!("--setenv=PATH={}", self.0.path),
        format!("--setenv=BMO_DATA_DIR={}", self.0.data.display()),
        format!("--working-directory={}", self.0.deploy.display()),
        binary.display().to_string(),
        "connect".into(),
        spec.display().to_string(),
      ],
    )
    .await
  }

  async fn stop(&self, unit: &str) {
    systemd("systemctl", &["--user".into(), "stop".into(), format!("{unit}.service")]).await;
  }

  async fn sync(&self, node: &str) -> bool {
    if self.running(&sync_unit(node)).await {
      return true;
    }

    systemd(
      "systemd-run",
      &[
        "--user".into(),
        "--quiet".into(),
        "--collect".into(),
        format!("--unit={}", sync_unit(node)),
        self.env_file(),
        format!("--setenv=PATH={}", self.0.path),
        format!("--working-directory={}", self.0.deploy.display()),
        self.0.deploy.join("bin").join(VERBS_BINARY).display().to_string(),
        "work-node".into(),
        "sync".into(),
        node.into(),
      ],
    )
    .await
  }
}

/// What one pass over the links came to: the links started, the ones stopped, the nodes whose copy is
/// being brought up, and the nodes skipped with why.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Settled {
  pub started: Vec<String>,
  pub stopped: Vec<String>,
  pub syncing: Vec<String>,
  pub skipped: Vec<Skipped>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Skipped {
  pub node: String,
  pub why: String,
}

/// One node's workplace as its row holds it.
struct Workplace {
  node: String,
  root: String,
  home: Option<String>,
  state: String,
}

/// A path on a node as its programs take it: one written from the home read against the node's home.
pub fn node_path(path: &str, home: Option<&str>) -> String {
  match home {
    Some(home) if path == "~" => home.to_owned(),
    Some(home) if path.starts_with("~/") => format!("{home}{}", &path[1..]),
    _ => path.to_owned(),
  }
}

async fn workplaces(db: &DatabaseConnection) -> Result<Vec<Workplace>, Refusal> {
  all(
    db,
    r#"SELECT "node", "root", "home", "state" FROM "node_workplaces" ORDER BY "node""#,
    vec![],
  )
  .await?
  .iter()
  .map(|row| {
    Ok(Workplace {
      node: row.try_get("", "node")?,
      root: row.try_get("", "root")?,
      home: row.try_get("", "home")?,
      state: row.try_get("", "state")?,
    })
  })
  .collect::<Result<_, sea_orm::DbErr>>()
  .map_err(Refusal::from)
}

/// The workers, in the order the queue lists them, with the node each stands on and its checkout.
async fn workers(
  db: &DatabaseConnection,
) -> Result<Vec<(String, Option<String>, Option<String>)>, Refusal> {
  Ok(
    crate::queue::workers::workers(db, std::path::Path::new(""))
      .await?
      .into_iter()
      .filter(|worker| !worker.builtin)
      .map(|worker| (worker.name, worker.node, Some(worker.cwd)))
      .collect(),
  )
}

/// An agent placed on a node: its name and its checkout as the node reads it.
struct Placed {
  agent: String,
  cwd: String,
}

/// The agents placed on each node, read off the workers' rows with their paths as the node reads them.
async fn placed_agents(
  db: &DatabaseConnection,
  places: &[Workplace],
) -> Result<BTreeMap<String, Vec<Placed>>, Refusal> {
  let mut placed: BTreeMap<String, Vec<Placed>> = BTreeMap::new();

  for (agent, node, cwd) in workers(db).await? {
    let (Some(node), Some(cwd)) = (node, cwd) else {
      continue;
    };
    let home = places.iter().find(|place| place.node == node).and_then(|place| place.home.as_deref());

    placed.entry(node).or_default().push(Placed {
      cwd: node_path(&cwd, home),
      agent,
    });
  }

  Ok(placed)
}

/// The directories every account keeps its records under, as this machine reads them, each once.
async fn record_dirs(db: &DatabaseConnection, roots: &Roots) -> Result<Vec<String>, Refusal> {
  let mut dirs: Vec<String> = Vec::new();
  let held = accounts(db).await.map_err(|refused| crate::lines::fault(refused.error))?;

  for account in &held {
    let dir = account
      .credential
      .own_home()
      .unwrap_or(roots.config_dir.as_path())
      .display()
      .to_string();

    if !dirs.contains(&dir) {
      dirs.push(dir);
    }
  }

  Ok(dirs)
}

fn read_json(path: &Path) -> Option<Value> {
  serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn text_of(value: &Value, key: &str) -> Option<String> {
  value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// A working directory as the CLI names its records and a node's shadow names its checkout: every
/// character but a letter or a digit a hyphen.
pub fn hyphened(cwd: &str) -> String {
  cwd.chars().map(|one| if one.is_ascii_alphanumeric() { one } else { '-' }).collect()
}

/// Where a node's link keeps its shadow of a checkout on that node, under the data root here: the copy the
/// checkout's graph is read through on this machine (docs/work-nodes.md, 「The link」).
pub fn checkout_shadow(data_root: &Path, node: &str, cwd: &str) -> PathBuf {
  data_root.join("nodes").join(node).join("shadow").join(hyphened(cwd))
}

/// Where the CLI keeps a working directory's records under a projects directory.
pub fn project_dir(cwd: &str, root: &str) -> String {
  join(root, &hyphened(cwd))
}

fn join(base: &str, part: &str) -> String {
  Path::new(base).join(part).display().to_string()
}

/// The spec of a node's link as the store and the node service stand now, or why the node has none.
pub async fn spec(
  db: &DatabaseConnection,
  nodes: &dyn NodeService,
  roots: &Roots,
  node: &str,
) -> Result<Result<Spec, String>, Refusal> {
  let places = workplaces(db).await?;
  let workplace = places.iter().find(|place| place.node == node);
  let setup = read_json(&roots.setup_path(node));
  let Some(ssh) = nodes.ssh_login(node).await? else {
    return Ok(Err(format!("{node} has no SSH route")));
  };
  let Some((workplace, home)) = workplace.and_then(|place| Some((place, place.home.clone()?))) else {
    return Ok(Err(format!("{node} has no workplace set up")));
  };
  let Some((session, socket)) = setup
    .as_ref()
    .and_then(|setup| Some((text_of(setup, "herdrSession")?, text_of(setup, "herdrSocket")?)))
  else {
    return Ok(Err(format!("{node}'s setup left no record of its herdr session")));
  };
  let node_root = node_path(&workplace.root, Some(&home));
  let agents = placed_agents(db, &places).await?.remove(node).unwrap_or_default();
  let records = record_dirs(db, roots).await?;
  let here = roots.node_dir(node);
  let shown = |path: &Path| path.display().to_string();

  // This machine's roots and the node's for each, the longest first, so a root under another is taken
  // first.
  let mut rewrite = vec![
    (shown(&roots.data), join(&node_root, "data")),
    (shown(&roots.deploy), join(&node_root, "deploy")),
    (shown(&roots.agents), join(&node_root, "agents")),
    (shown(&roots.home), home.clone()),
  ];

  rewrite.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));

  let to_node = |path: &str| {
    node_link::spec::rewrite(path, rewrite.iter().map(|(from, to)| (from.as_str(), to.as_str())))
  };
  let mut pull = Vec::new();

  for placed in &agents {
    let place = shown(&roots.place(&placed.agent));

    pull.push(Landing {
      pull: Pull {
        remote: to_node(&place),
        files: PULLED_PLACE_FILES.map(str::to_owned).to_vec(),
        tree: false,
        append: false,
        alive: true,
      },
      local: place.into(),
    });

    for records in &records {
      pull.push(Landing {
        pull: Pull {
          remote: project_dir(&placed.cwd, &join(&to_node(records), "projects")),
          files: vec![],
          tree: true,
          append: true,
          alive: false,
        },
        local: project_dir(&placed.cwd, &join(records, "projects")).into(),
      });
    }

    pull.push(Landing {
      pull: Pull {
        remote: join(&placed.cwd, "graph"),
        files: vec![],
        tree: false,
        append: false,
        alive: false,
      },
      local: checkout_shadow(&roots.data, node, &placed.cwd).join("graph"),
    });
  }

  let bridge = join(&join(&node_root, "data"), "bridge.sock");

  Ok(Ok(Spec {
    node: node.to_owned(),
    ssh,
    root: node_root.clone(),
    binary: join(&node_root, "deploy/bin/node-link"),
    herdr: Forward {
      local: here.join("herdr.sock"),
      remote: socket,
      session: Some(session),
    },
    ports: vec![roots.mcp_port],
    bridge: here.join("bridge.sock"),
    deploy: roots.cli_deploy.clone(),
    control: here.join("control.sock"),
    push: agents
      .iter()
      .map(|placed| Push {
        local: roots.place(&placed.agent),
        files: PUSHED_PLACE_FILES.map(str::to_owned).to_vec(),
        rewrite: true,
      })
      .collect(),
    pull,
    proc: here.join("proc"),
    state: here.join("link-state.json"),
    follow: vec![shown(&roots.data), shown(&roots.agents)],
    withheld: vec![roots.data.join("env")],
    path_head: vec![
      join(&home, ".bun/bin"),
      join(&home, ".local/bin"),
      join(&home, ".cargo/bin"),
      join(&node_root, "deploy/bin"),
    ],
    env_keep: NODE_ENV_KEEP.map(str::to_owned).to_vec(),
    env_add: vec![
      ("BMO_HOME_BRIDGE".into(), bridge),
      ("BMO_DATA_DIR".into(), join(&node_root, "data")),
      ("BMO_NODE".into(), node.to_owned()),
    ],
    rewrite,
    jake: None,
  }))
}

/// A spec written where it moved, so the link reads it again only when there is something new to read;
/// whether it was.
pub fn write_spec(roots: &Roots, spec: &Spec) -> Result<bool, String> {
  let path = roots.spec_path(&spec.node);
  let body = format!("{}\n", serde_json::to_string_pretty(spec).map_err(|why| why.to_string())?);

  if std::fs::read_to_string(&path).is_ok_and(|kept| kept == body) {
    return Ok(false);
  }

  write_whole(&path, &body)?;

  Ok(true)
}

/// A file written whole beside itself and moved into place, readable by this account alone.
fn write_whole(path: &Path, body: &str) -> Result<(), String> {
  use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

  let shown = |why: std::io::Error| format!("{}: {why}", path.display());
  let parent = path.parent().unwrap_or(Path::new("/"));
  let temporary = path.with_extension(format!("json.{}", std::process::id()));

  std::fs::DirBuilder::new().recursive(true).mode(0o700).create(parent).map_err(shown)?;

  let mut file = std::fs::OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .mode(0o600)
    .open(&temporary)
    .map_err(shown)?;

  std::io::Write::write_all(&mut file, body.as_bytes()).map_err(shown)?;
  std::fs::rename(&temporary, path).map_err(shown)
}

/// The node a place stands on, or none for a place here.
pub fn place_node(place: &Path) -> Option<String> {
  text_of(&read_json(&place.join(PLACE_NODE_FILE))?, "node")
}

/// A place marked as standing on a node, or on this machine where the node is none.
pub fn mark_place_node(place: &Path, node: Option<&str>) -> Result<(), String> {
  match node {
    None => match std::fs::remove_file(place.join(PLACE_NODE_FILE)) {
      Err(why) if why.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {why}", place.display())),
      _ => Ok(()),
    },
    Some(node) => {
      let body = serde_json::to_string_pretty(&serde_json::json!({ "node": node }))
        .map_err(|why| why.to_string())?;

      write_whole(&place.join(PLACE_NODE_FILE), &format!("{body}\n"))
    }
  }
}

/// Every link brought to what the store says: a spec written and its unit standing for every ready
/// workplace, the unit stopped for every other, and each worker's place marked with the node it stands
/// on, or unmarked where it came back.
pub async fn settle(
  db: &DatabaseConnection,
  nodes: &dyn NodeService,
  units: &impl LinkUnits,
  roots: &Roots,
) -> Result<Settled, String> {
  let refused = |refusal: Refusal| refusal.detail;
  let mut settled = Settled::default();

  for (agent, node, _) in workers(db).await.map_err(refused)? {
    let place = roots.place(&agent);

    if place_node(&place) != node {
      mark_place_node(&place, node.as_deref())?;
    }
  }

  let deployed = read_json(&roots.data.join("deployed.json")).and_then(|record| text_of(&record, "commit"));

  for workplace in workplaces(db).await.map_err(refused)? {
    let node = workplace.node;
    let unit = link_unit(&node);

    if workplace.state != "ready" {
      if units.running(&unit).await {
        units.stop(&unit).await;
        settled.stopped.push(node);
      }

      continue;
    }

    let spec = match spec(db, nodes, roots, &node).await.map_err(refused)? {
      Ok(spec) => spec,
      Err(why) => {
        settled.skipped.push(Skipped { node, why });
        continue;
      }
    };

    write_spec(roots, &spec)?;

    // A deployment moved past the code the node holds: its copy is brought up apart from the server,
    // and the sync stands the link again once it has.
    let copied = read_json(&roots.setup_path(&node)).and_then(|setup| text_of(&setup, "commit"));

    if deployed.is_some() && copied != deployed && units.sync(&node).await {
      settled.syncing.push(node.clone());
    }

    if !units.running(&unit).await {
      if units.start(&unit, &roots.spec_path(&node)).await {
        settled.started.push(node);
      } else {
        settled.skipped.push(Skipped {
          node,
          why: "its link would not start".into(),
        });
      }
    }
  }

  Ok(settled)
}
