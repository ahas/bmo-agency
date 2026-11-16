//! The links to the work nodes, kept by agency's server (docs/work-nodes.md, 「The link」): one spec per
//! ready workplace, written from the store whenever the workers or the nodes move, its SSH login or its hub
//! asked of the node service; every place of an agent placed on a node marked with its node; and one unit per
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
use node_link::spec::{Forward, Jake, Landing, Local, Push, Spec};
use policy::account_isolation::ACCOUNT_ISOLATION;
use policy::person_nodes::PERSON_NODES;
use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::Serialize;
use serde_json::Value;
use store::{all, one};

use crate::Refusal;
use crate::control::claude_accounts::accounts;
use crate::peers::node::{HubReach, NodeService};

/// The place files that go to the node: what the launcher writes and the program on the node reads.
const PUSHED_PLACE_FILES: [&str; 4] = ["program.json", "settings.json", "run.json", "handed"];

/// What a place's running record and start failure are pulled as.
const PULLED_PLACE_FILES: [&str; 2] = ["running.json", "start-failed.json"];

/// The variables a program on a node is started with, of those this machine's launch gave it: among them the
/// console's public address and a supervisor's owner account, which its secret requests are written with.
const NODE_ENV_KEEP: [&str; 17] = [
  "BMO_AGENT_PLACE",
  "BMO_CONSOLE_ADDRESS",
  "BMO_SOURCE_DIR",
  "BMO_MCP_TOKEN",
  crate::supervisors::ACCOUNT_VARIABLE,
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

/// The directories ahead of a node's own search path for every command and program of its workplace: the
/// workplace's own runtime, the tools installed into the account, then the workplace's own binaries.
pub fn path_head(home: &str, root: &str) -> Vec<String> {
  let at = |base: &str, part: &str| Path::new(base).join(part).to_string_lossy().into_owned();

  vec![
    at(root, "bun/bin"),
    at(home, ".bun/bin"),
    at(home, ".local/bin"),
    at(home, ".cargo/bin"),
    at(root, "deploy/bin"),
  ]
}

/// The system's own directories behind a node's search path, for a program its far end starts.
const SYSTEM_PATH: [&str; 3] = ["/usr/local/bin", "/usr/bin", "/bin"];

/// The commit the deployment runs, as its record beside the deploy tree's link names it.
pub fn deployed_commit(deploy_link: &Path) -> Option<String> {
  let record = read_json(&deploy_link.parent()?.join("deployed.json"))?;

  text_of(&record, "commit")
}

/// The server's environment file under the data root, as its unit names it.
const SERVER_ENV_FILE: &str = "agency.env";

/// The mode the jake hub's executable carries one byte stream to an agent's port in.
const JAKE_CONNECT: &str = "connect";

/// How long a link may go without a word from the node before it is taken as not heard.
const HEARD_WITHIN_MS: i64 = 20_000;

/// The directory under the data root every agent's place stands in.
const PLACES_DIR: &str = "agent-places";

/// The unit one node's link runs under.
pub fn link_unit(node: &str) -> String {
  format!("bmo-node-link-{node}")
}

/// The unit an isolation node's workplace is stood up under.
pub fn setup_unit(node: &str) -> String {
  format!("bmo-node-setup-{node}")
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
  /// The jake hub's executable, whose connect mode carries a link to a person's PC, where the deployment
  /// names one (docs/person-nodes.md, 「The link over jake」).
  pub jake_hub: Option<PathBuf>,
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
      jake_hub: machine.var(crate::config::JAKE_HUB).map(PathBuf::from),
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
  /// An isolation node's workplace stood up, its user made with it, apart from the server; whether its setup runs.
  fn set_up(&self, node: &str) -> impl Future<Output = bool> + Send;
}

/// The user's own service manager, started on with the server's roots.
pub struct Systemd<'a>(pub &'a Roots);

impl Systemd<'_> {
  /// The server's own environment file, which a link's units run on as the server does: a sync of a node's
  /// copy asks the node service, whose address stands there.
  fn env_file(&self) -> String {
    format!(
      "--property=EnvironmentFile=-{}",
      self.0.data.join("env").join(SERVER_ENV_FILE).display()
    )
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

  async fn set_up(&self, node: &str) -> bool {
    if self.running(&setup_unit(node)).await {
      return true;
    }

    systemd(
      "systemd-run",
      &[
        "--user".into(),
        "--quiet".into(),
        "--collect".into(),
        format!("--unit={}", setup_unit(node)),
        self.env_file(),
        format!("--setenv=PATH={}", self.0.path),
        format!("--working-directory={}", self.0.deploy.display()),
        self.0.deploy.join("bin").join(VERBS_BINARY).display().to_string(),
        "work-node".into(),
        "setup".into(),
        node.into(),
        "--install".into(),
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
  /// The isolation nodes whose setup was started (docs/account-isolation.md, 「Standing it up」).
  pub setting_up: Vec<String>,
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
  checked_at: Option<i64>,
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
    r#"SELECT "node", "root", "home", "state", "checked_at" FROM "node_workplaces" ORDER BY "node""#,
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
      checked_at: row.try_get("", "checked_at")?,
    })
  })
  .collect::<Result<_, sea_orm::DbErr>>()
  .map_err(Refusal::from)
}

/// A node's workplace root as its programs read it, where its workplace is set up and its home known.
pub async fn node_root<C: ConnectionTrait>(db: &C, node: &str) -> Result<Option<String>, Refusal> {
  let row = one(
    db,
    r#"SELECT "root", "home" FROM "node_workplaces" WHERE "node" = $1"#,
    vec![node.into()],
  )
  .await?;
  let Some(row) = row else { return Ok(None) };
  let root: String = row.try_get("", "root")?;
  let home: Option<String> = row.try_get("", "home")?;

  Ok(home.map(|home| node_path(&root, Some(&home))))
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

/// An agent placed on a node: its name, the folder it works in, and whether that folder is a checkout of the
/// node's, read as the node reads it, or one of this machine's that the node reads through the spec's roots.
struct Placed {
  agent: String,
  cwd: String,
  checkout: bool,
}

/// The agents placed on each node: the workers its rows place there, with their checkouts as the node reads
/// them, and every other place marked as standing there with the folder its mark names, an account's
/// supervisors and its role runs among them (docs/person-nodes.md, 「Which node is a person's」).
async fn placed_agents(
  db: &DatabaseConnection,
  places: &[Workplace],
  roots: &Roots,
) -> Result<BTreeMap<String, Vec<Placed>>, Refusal> {
  let mut placed: BTreeMap<String, Vec<Placed>> = BTreeMap::new();
  let workers = workers(db).await?;

  for (agent, node, cwd) in &workers {
    let (Some(node), Some(cwd)) = (node, cwd) else {
      continue;
    };
    let home = places.iter().find(|place| &place.node == node).and_then(|place| place.home.as_deref());

    placed.entry(node.clone()).or_default().push(Placed {
      cwd: node_path(cwd, home),
      agent: agent.clone(),
      checkout: true,
    });
  }

  let marked = std::fs::read_dir(roots.data.join(PLACES_DIR)).into_iter().flatten().flatten();

  for entry in marked {
    let agent = entry.file_name().to_string_lossy().into_owned();
    let Some(mark) = read_json(&entry.path().join(PLACE_NODE_FILE)) else { continue };
    let (Some(node), Some(cwd)) = (text_of(&mark, "node"), text_of(&mark, "cwd")) else {
      continue;
    };

    if workers.iter().all(|(worker, _, _)| *worker != agent) {
      placed.entry(node).or_default().push(Placed {
        agent,
        cwd,
        checkout: false,
      });
    }
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
  // An account's isolation node is this machine's under a user of its own, reached with neither SSH nor a hub
  // (docs/account-isolation.md, 「The link」).
  let isolation = crate::account_isolation::Isolation::of_node(node);
  let ssh = match &isolation {
    Some(_) => None,
    None => nodes.ssh_login(node).await?,
  };
  let jake = match (&ssh, &isolation) {
    (Some(_), _) | (None, Some(_)) => None,
    (None, None) => match nodes.hub_reach(node).await? {
      None => return Ok(Err(format!("{node} has no SSH route and no hub"))),
      Some(reach) => match jake_of(roots, node, reach, setup.as_ref()) {
        Ok(jake) => Some(jake),
        Err(why) => return Ok(Err(why)),
      },
    },
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
  let agents = placed_agents(db, &places, roots).await?.remove(node).unwrap_or_default();
  let records = record_dirs(db, roots).await?;
  let here = roots.node_dir(node);
  let shown = |path: &Path| path.display().to_string();

  // This machine's roots and the node's for each, and each supervisor's folder of an account sharing the node read
  // as that folder under its account's root there (docs/person-nodes.md, 「Several accounts on one node」), the
  // longest first, so a root under another is taken first.
  let mut rewrite = vec![
    (shown(&roots.data), join(&node_root, "data")),
    (shown(&roots.deploy), join(&node_root, "deploy")),
    (shown(&roots.agents), join(&node_root, "agents")),
    (shown(&roots.home), home.clone()),
  ];

  for (supervisor, account) in crate::person_nodes::sharing_supervisors(db, node).await? {
    let account_root = crate::person_nodes::account_root(&node_root, &account);
    let homes = harness_machine::supervisor_prompt::homes;

    rewrite.push((
      shown(&homes(&roots.data).join(&supervisor)),
      shown(&homes(Path::new(&account_root)).join(&supervisor)),
    ));
  }

  rewrite.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));

  let to_node = |path: &str| {
    node_link::spec::rewrite(path, rewrite.iter().map(|(from, to)| (from.as_str(), to.as_str())))
  };
  let mut pull = Vec::new();

  for placed in &agents {
    let place = shown(&roots.place(&placed.agent));
    // A folder of this machine's reads as the node's through the same roots as every other path.
    let cwd = if placed.checkout { placed.cwd.clone() } else { to_node(&placed.cwd) };

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
          remote: project_dir(&cwd, &join(&to_node(records), "projects")),
          files: vec![],
          tree: true,
          append: true,
          alive: false,
        },
        local: project_dir(&placed.cwd, &join(records, "projects")).into(),
      });
    }

    if !placed.checkout {
      continue;
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
  let local = isolation.as_ref().map(|isolation| {
    let mut path = path_head(&home, &node_root);

    path.extend(SYSTEM_PATH.map(str::to_owned));

    Local {
      run_as: isolation.sandbox(&path.join(":"), roots.mcp_port),
      herdr: isolation.herdr_socket(),
    }
  });
  // A node an account's record names asks the bridge for the policy's verbs alone, so no session of a person's
  // runs here what reaches the hub as this machine's user (docs/person-nodes.md, 「Who reaches a node」).
  let owned = !crate::person_nodes::owners_of(db, node).await?.is_empty();
  let bridge_verbs = owned.then(|| PERSON_NODES.bridge_verbs.clone());

  Ok(Ok(Spec {
    node: node.to_owned(),
    ssh: ssh.unwrap_or_default(),
    root: node_root.clone(),
    binary: join(&node_root, "deploy/bin/node-link"),
    herdr: Forward {
      local: here.join("herdr.sock"),
      remote: socket,
      session: Some(session),
    },
    // A node of this machine reaches the tool host over the loopback it shares, and its bridge in the link's folder.
    ports: if isolation.is_some() { Vec::new() } else { vec![roots.mcp_port] },
    bridge: isolation
      .as_ref()
      .map_or_else(|| here.join("bridge.sock"), |isolation| isolation.bridge_socket()),
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
    path_head: path_head(&home, &node_root),
    env_keep: NODE_ENV_KEEP.map(str::to_owned).to_vec(),
    env_add: vec![
      ("BMO_HOME_BRIDGE".into(), bridge),
      ("BMO_DATA_DIR".into(), join(&node_root, "data")),
      ("BMO_NODE".into(), node.to_owned()),
    ],
    folders: agents
      .iter()
      .filter(|placed| !placed.checkout)
      .map(|placed| to_node(&placed.cwd))
      .collect(),
    bridge_verbs,
    rewrite,
    jake,
    local,
  }))
}

/// The bridge relay port a node's setup record names, where it names one.
pub fn bridge_relay(setup: &Value) -> Option<u16> {
  setup
    .get("bridgeRelay")
    .and_then(Value::as_u64)
    .and_then(|port| u16::try_from(port).ok())
}

// How the link reaches a person's PC through its hub, or why it cannot yet.
fn jake_of(roots: &Roots, node: &str, reach: HubReach, setup: Option<&Value>) -> Result<Jake, String> {
  let Some(program) = &roots.jake_hub else {
    return Err(format!(
      "{node} is reached through a hub, and the deployment names no hub program in {}",
      crate::config::JAKE_HUB
    ));
  };
  let Some(bridge_relay) = setup.and_then(bridge_relay) else {
    return Err(format!("{node}'s setup left no bridge relay port"));
  };
  let ports = &PERSON_NODES;
  let hub_env = match reach.token_file.strip_prefix("~/") {
    Some(rest) => roots.home.join(rest),
    None => PathBuf::from(&reach.token_file),
  };

  Ok(Jake {
    agent: reach.agent,
    connect: vec![program.display().to_string(), JAKE_CONNECT.to_owned()],
    hub_env,
    mirror_port: ports.mirror_port,
    herdr_port: ports.herdr_port,
    bridge_port: ports.bridge_port,
    bridge_relay,
  })
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

/// The folder of this machine's a place marked as standing on a node works in, as its mark names it.
pub fn place_cwd(place: &Path) -> Option<String> {
  text_of(&read_json(&place.join(PLACE_NODE_FILE))?, "cwd")
}

/// A place that is no worker's marked as standing on a node with the folder of this machine's its program works
/// in, which the node reads through the spec's roots and whose records the link brings back; or unmarked where
/// the node is none.
pub fn mark_place_on(place: &Path, node: Option<&str>, cwd: &str) -> Result<(), String> {
  let Some(node) = node else { return mark_place_node(place, None) };
  let body = serde_json::to_string_pretty(&serde_json::json!({ "node": node, "cwd": cwd }))
    .map_err(|why| why.to_string())?;

  write_whole(&place.join(PLACE_NODE_FILE), &format!("{body}\n"))
}

/// Whether a node's link was heard from lately, as the link last wrote its state here: connected, and heard
/// within the last twenty seconds.
pub fn link_heard(data_root: &Path, node: &str) -> bool {
  let Some(state) = read_json(&data_root.join("nodes").join(node).join("link-state.json")) else {
    return false;
  };
  let heard = state.get("heard").and_then(Value::as_i64);

  state.get("state").and_then(Value::as_str) == Some("connected")
    && heard.is_some_and(|heard| crate::now_ms() - heard <= HEARD_WITHIN_MS)
}

/// One node's spec written again where a workplace is ready for it, so a place just marked for it is mirrored
/// without waiting for the next pass.
pub async fn write_node_spec(
  db: &DatabaseConnection,
  nodes: &dyn NodeService,
  roots: &Roots,
  node: &str,
) -> Result<(), String> {
  match spec(db, nodes, roots, node).await.map_err(|refusal| refusal.detail)? {
    Ok(spec) => write_spec(roots, &spec).map(|_| ()),
    Err(why) => Err(why),
  }
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

  // A supervisor of an account with a node stands there, working in its own folder.
  for (agent, node) in crate::person_nodes::supervisors_placed(db).await.map_err(refused)? {
    let place = roots.place(&agent);

    if place_node(&place) != node {
      let folder = harness_machine::supervisor_prompt::homes(&roots.data).join(&agent);

      mark_place_on(&place, node.as_deref(), &folder.display().to_string())?;
    }
  }

  // An account placed on its isolation node whose workplace is not ready is stood up apart from the server: at once
  // where it has none, and again once a while has passed since its last setup said what it missed
  // (docs/account-isolation.md, 「Standing it up」).
  let held = workplaces(db).await.map_err(refused)?;
  let now = chrono::Utc::now().timestamp();

  for account in crate::person_nodes::accounts_placed(db).await.map_err(refused)? {
    let Some(node) = crate::person_nodes::node_of(db, &account).await.map_err(refused)? else {
      continue;
    };

    if crate::account_isolation::Isolation::of_node(&node).is_none() {
      continue;
    }

    let due = match held.iter().find(|place| place.node == node) {
      None => true,
      Some(place) => {
        place.state != "ready"
          && place.checked_at.is_none_or(|at| now - at >= ACCOUNT_ISOLATION.setup_again_seconds)
      }
    };

    if due && units.set_up(&node).await {
      settled.setting_up.push(node);
    }
  }

  let deployed = deployed_commit(&roots.deploy);

  for workplace in held {
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
