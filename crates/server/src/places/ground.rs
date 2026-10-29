//! What every place is stood on (docs/agent-control.md, 「One standing pane per agent」, 「The job's
//! release」): the store, the service's configuration, peers, events and hooks, the machine's trees and
//! environment, this machine's herdr, and the source root a worker with no directory of its own works in;
//! and an agent's row as the places read it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cache_siren::Keeping;
use harness_providers::Machine;
use herdr::Herdr;
use policy::fill_marks;
use policy::place_words::PLACE_WORDS;
use sea_orm::{DatabaseConnection, QueryResult};
use store::{all, one};

use super::home::{HEAD_MARK, HEAD_STOPPED, Home, Launch};
use crate::config::Config;
use crate::events::Bus;
use crate::hooks::Hooks;
use crate::peers::Peers;
use crate::readiness::Readiness;
use crate::watches::links::{self, Roots};
use crate::{Refusal, Service};

// The variable a run is told its own name in, and the job runner's marker, which a run apart never
// carries (docs/live-claude-guard.md).
const RUN_NAME: &str = "BMO_RUN_NAME";
const JOB_RUNNER_MARKER: &str = "BMO_TASK_LOCKED";

/// The variable naming the tree a process reads its code out of: the deploy tree, or the release a
/// process is pinned to (docs/agent-control.md, 「The job's release」).
pub const DEPLOY_VARIABLE: &str = "BMO_DEPLOY_DIR";

/// The verbs' binary, as a code tree's `bin/` holds it.
pub const VERBS_BINARY: &str = "bmo-agency";

/// One agent's row as the places read it.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentRow {
  pub name: String,
  pub kind: String,
  pub label: Option<String>,
  pub herdr_label: Option<String>,
  pub node: Option<String>,
  pub cwd: Option<String>,
  pub dev_command: Option<String>,
  pub support_of: Option<String>,
  pub resting: bool,
}

const AGENT_COLUMNS: &str = r#""name", "kind", "label", "herdr_label", "node", "cwd", "dev_command", "support_of",
  "resting_since" IS NOT NULL AS "resting""#;

impl AgentRow {
  fn read(row: &QueryResult) -> Result<Self, sea_orm::DbErr> {
    Ok(Self {
      name: row.try_get("", "name")?,
      kind: row.try_get("", "kind")?,
      label: row.try_get("", "label")?,
      herdr_label: row.try_get("", "herdr_label")?,
      node: row.try_get("", "node")?,
      cwd: row.try_get("", "cwd")?,
      dev_command: row.try_get("", "dev_command")?,
      support_of: row.try_get("", "support_of")?,
      resting: row.try_get("", "resting")?,
    })
  }

  /// Every agent but a resting worker, which keeps its row for its record alone.
  pub async fn standing(db: &DatabaseConnection) -> Result<Vec<Self>, Refusal> {
    let sql = format!(
      r#"SELECT {AGENT_COLUMNS} FROM "agents" WHERE "resting_since" IS NULL ORDER BY "kind", "name""#
    );

    Ok(all(db, &sql, vec![]).await?.iter().map(Self::read).collect::<Result<_, _>>()?)
  }

  /// The workers that are not resting, in the order the queue lists them.
  pub async fn workers(db: &DatabaseConnection) -> Result<Vec<Self>, Refusal> {
    let sql = format!(
      r#"SELECT {AGENT_COLUMNS} FROM "agents" a WHERE a."kind" = 'worker' AND a."resting_since" IS NULL
         ORDER BY COALESCE((SELECT b."builtin" FROM "agents" b WHERE b."name" = a."support_of"), a."builtin") DESC,
           COALESCE(a."support_of", a."name"), COALESCE(a."support_slot", 1)"#
    );

    Ok(all(db, &sql, vec![]).await?.iter().map(Self::read).collect::<Result<_, _>>()?)
  }

  /// One agent's row, a resting worker's included.
  pub async fn named(db: &DatabaseConnection, name: &str) -> Result<Option<Self>, Refusal> {
    let sql = format!(r#"SELECT {AGENT_COLUMNS} FROM "agents" WHERE "name" = $1"#);

    Ok(one(db, &sql, vec![name.into()]).await?.as_ref().map(Self::read).transpose()?)
  }

  /// The line the agent's work goes out on: a runner is named for its line, every other agent's kind
  /// is its line's name.
  pub fn line(&self) -> &str {
    if self.kind == "runner" { &self.name } else { &self.kind }
  }

  /// The label herdr shows it by.
  pub fn shown(&self) -> &str {
    self.herdr_label.as_deref().or(self.label.as_deref()).unwrap_or(&self.name)
  }
}

/// What every place is stood on: the store, the service's configuration, the services it calls, the
/// accounts' usage windows it keeps, the bus its events go on and the hooks its consumers registered,
/// whether the places have stood once, the machine's trees and environment, this machine's herdr, and the
/// source root a worker with no directory of its own works in.
pub struct Ground {
  pub db: DatabaseConnection,
  pub config: Arc<Config>,
  pub peers: Peers,
  pub windows: Keeping,
  pub events: Bus,
  pub hooks: Hooks,
  pub readiness: Arc<Readiness>,
  pub roots: Roots,
  pub herdr: Option<Herdr>,
  pub source: PathBuf,
  vars: BTreeMap<String, String>,
  env: BTreeMap<String, String>,
}

/// Whether a tree holds the code the heads and the lines are composed of.
fn holds_code(tree: &Path) -> bool {
  tree.join("bin").join("pane-head").exists() || tree.join("prompts").join("rules").is_dir()
}

/// The first of these trees that holds code — the one `BMO_DEPLOY_DIR` names, the one the configuration
/// names, the deploy tree's link — and the working directory where none does: a test, or a machine with
/// no deployment.
fn first_code_tree(named: Option<PathBuf>, configured: Option<PathBuf>, deploy: &Path) -> PathBuf {
  named
    .into_iter()
    .chain(configured)
    .chain([deploy.to_path_buf()])
    .find(|tree| holds_code(tree))
    .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| deploy.to_path_buf()))
}

/// The tree the code is read from as a machine names it, before any ground stands on it: what the store's
/// seeds read their files from as the server starts.
pub fn machine_code_root(machine: &Machine) -> PathBuf {
  let trees = crate::config::Trees::of_machine(machine);
  let home = PathBuf::from(machine.var("HOME").unwrap_or("/"));
  let deploy = trees.deploy_link(&home);

  first_code_tree(machine.var(DEPLOY_VARIABLE).map(PathBuf::from), trees.code, &deploy)
}

impl Ground {
  /// The ground of the service on a machine: its trees on the deploy link and the source root the
  /// configuration names. A process whose launcher named no code tree hands its own on to everything it
  /// starts, so no child reads a newer release than its parent (docs/agent-control.md, 「The job's
  /// release」).
  pub fn new(service: &Service, machine: &Machine, herdr: Option<Herdr>) -> Self {
    let home = PathBuf::from(machine.var("HOME").unwrap_or("/"));
    let trees = &service.config.trees;
    let mut ground = Self {
      db: service.db.clone(),
      config: service.config.clone(),
      peers: service.peers.clone(),
      windows: crate::usage_windows::keeping(&service.db, machine),
      events: service.events.clone(),
      hooks: service.hooks.clone(),
      readiness: service.readiness.clone(),
      roots: Roots::of_machine(machine, trees.deploy_link(&home)),
      herdr,
      source: trees.source_root(),
      vars: machine.vars().map(|(name, value)| (name.to_owned(), value.to_owned())).collect(),
      env: machine.every_var().map(|(name, value)| (name.to_owned(), value.to_owned())).collect(),
    };

    if machine.var(DEPLOY_VARIABLE).is_none() {
      let own = ground.code_root().to_string_lossy().into_owned();

      ground.vars.insert(DEPLOY_VARIABLE.to_owned(), own.clone());
      ground.env.insert(DEPLOY_VARIABLE.to_owned(), own);
    }

    ground
  }

  /// The ground of the service on a data root: the machine under that root, and this machine's herdr
  /// where the environment names one.
  pub fn of_data_root(service: &Service, data_root: &Path) -> Self {
    let machine = Machine::of_process(data_root);
    let herdr = herdr::socket_path(machine.var("XDG_CONFIG_HOME"), machine.var("HOME"))
      .map(|socket| Herdr::here(socket, herdr::silences_path(machine.var("BMO_HERDR_SILENCES"))));

    Self::new(service, &machine, herdr)
  }

  /// Whether this ground stands a test's server: it reaches no deployment's peer, whatever its process
  /// environment carried, and the calls that would reach one are not made (docs/api.md, 「Calls agency
  /// makes」).
  pub fn testing(&self) -> bool {
    self.vars.get("BMO_TEST_GROUND").is_some_and(|set| set.trim() == "1")
  }

  /// The ground with one variable of its environment set to the value given, or taken out where none is.
  pub fn with_var(mut self, name: &str, value: Option<&str>) -> Self {
    match value {
      Some(value) => self.env.insert(name.to_owned(), value.to_owned()),
      None => self.env.remove(name),
    };

    self
  }

  /// The tree the heads, the lines and the verbs run from, the first of these that holds code: the one
  /// `BMO_DEPLOY_DIR` names — a job's own release — the one the configuration names as the tree the
  /// deployment started this process on, and the deploy tree's link; the working directory where none
  /// does — a test, or a machine with no deployment.
  pub fn code_root(&self) -> PathBuf {
    let named = self.env.get(DEPLOY_VARIABLE).filter(|named| !named.is_empty()).map(PathBuf::from);

    first_code_tree(named, self.config.trees.code.clone(), &self.roots.deploy)
  }

  /// The code tree as text a person or an agent reads names it: the deploy link wherever the link holds
  /// code, so no release path enters what is cached across a deployment.
  pub fn shown_code_root(&self) -> PathBuf {
    if holds_code(&self.roots.deploy) {
      self.roots.deploy.clone()
    } else {
      self.code_root()
    }
  }

  /// The verbs' binary a launch starts one of agency's verbs on: the code tree's own, as its scripts are
  /// the tree's, and the one beside this program where the tree holds none.
  pub fn verbs_binary(&self) -> PathBuf {
    let deployed = self.code_root().join("bin").join(VERBS_BINARY);

    if deployed.exists() {
      return deployed;
    }

    std::env::current_exe()
      .ok()
      .and_then(|own| Some(own.parent()?.join(VERBS_BINARY)))
      .unwrap_or(deployed)
  }

  /// The copy of the data root this process stands on, by the name its workspaces are told apart by;
  /// none on the machine's own.
  pub fn copy(&self) -> Option<String> {
    if self.roots.own_data_root() {
      return None;
    }

    let data = &self.roots.data;
    let own = data.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();

    Some(if own == "data" {
      data
        .parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
    } else {
      own
    })
  }

  /// What a run apart is started with: the search path, the home this process stands under, every
  /// `BMO_` variable this process carries but the job runner's marker, and the run's own name, which
  /// wins. The host starts a run on an emptied environment, and a runner of this binary finds its data
  /// root, its trees and its logins under the home it is handed.
  pub fn run_env(&self, name: &str) -> BTreeMap<String, String> {
    let mut env = BTreeMap::from([("PATH".to_owned(), self.roots.path.clone())]);

    for (variable, value) in &self.vars {
      if variable == "HOME" || variable.starts_with("BMO_") && variable != JOB_RUNNER_MARKER {
        env.insert(variable.clone(), value.clone());
      }
    }

    env.insert(RUN_NAME.to_owned(), name.to_owned());
    env
  }

  /// The machine as the providers ask it: this environment and data root.
  pub fn machine(&self) -> Machine {
    Machine::with_env(self.env.clone(), self.roots.data.clone())
  }

  /// The machine's environment, every variable set to something.
  pub fn vars(&self) -> &BTreeMap<String, String> {
    &self.vars
  }

  /// The environment as this process holds it, a variable set to nothing among it: what a launch
  /// composes a program's environment from.
  pub fn env(&self) -> &BTreeMap<String, String> {
    &self.env
  }

  /// A head pane's command: `pane-head` under a loop that starts it again wherever it ends but on the one
  /// exit it obeys, under the agent's mark.
  pub fn head_command(&self, agent: &str, root: &Path) -> Vec<String> {
    vec![
      "/bin/sh".to_owned(),
      "-c".to_owned(),
      format!(
        r#"deaths=0; window=$(date +%s); while :; do "$1" "$2"; code=$?; case $code in {HEAD_STOPPED}) exit 0;; esac; if [ -n "$AGENCY_CONTROL_TOKEN" ]; then data="${{BMO_DATA_DIR:-$HOME/.local/share/bmo/data}}"; last=$(tail -n 1 "$data/agent-places/$2/stderr.log" 2>/dev/null); curl -s -m 3 -X POST --get --data-urlencode "agent=$2" --data-urlencode "code=$code" --data-urlencode "last=$last" -H "Authorization: Bearer $AGENCY_CONTROL_TOKEN" "http://127.0.0.1:$AGENCY_CONTROL_PORT/pane-deaths" >/dev/null 2>&1; fi; now=$(date +%s); if [ $((now-window)) -le 60 ]; then deaths=$((deaths+1)); else deaths=1; window=$now; fi; if [ "$deaths" -ge 3 ]; then echo "pane $2: $deaths deaths inside a minute; the loop stops (last exit $code)"; exit 1; fi; sleep 1; done"#
      ),
      format!("{HEAD_MARK}{agent}"),
      root.join("bin").join("pane-head").to_string_lossy().into_owned(),
      agent.to_owned(),
    ]
  }

  /// The home of a name under a label, on this machine's herdr; none where this machine names no herdr.
  pub fn home_of(&self, agent: &str, label: &str) -> Option<Home> {
    let copy = self.copy();
    let root = self.code_root();

    Some(Home {
      agent: agent.to_owned(),
      resting: None,
      label: match &copy {
        None => label.to_owned(),
        Some(name) => fill_marks(&PLACE_WORDS.copy_label, &[("label", label), ("name", name)]),
      },
      data: copy.map(|_| self.roots.data.to_string_lossy().into_owned()),
      head: Launch {
        command: self.head_command(agent, &root),
        cwd: root.to_string_lossy().into_owned(),
        env: self.run_env(&format!("{HEAD_MARK}{agent}")),
      },
      herdr: self.herdr.clone()?,
    })
  }

  /// An agent's home by its row: under the label herdr shows it by; on a work node, in that node's herdr,
  /// its head the node's own `pane-head` on the node's roots.
  pub async fn home_of_row(&self, row: &AgentRow) -> Result<Option<Home>, Refusal> {
    let Some(mut home) = self.home_of(&row.name, row.shown()) else {
      return Ok(None);
    };
    let Some(node) = &row.node else { return Ok(Some(home)) };

    home.herdr = Herdr::there(self.roots.data.join("nodes").join(node).join("herdr.sock"));

    let workplace = one(
      &self.db,
      r#"SELECT "root", "home" FROM "node_workplaces" WHERE "node" = $1"#,
      vec![node.as_str().into()],
    )
    .await?;
    let Some(workplace) = workplace else { return Ok(Some(home)) };
    let (root, node_home): (String, Option<String>) =
      (workplace.try_get("", "root")?, workplace.try_get("", "home")?);
    let Some(node_home) = node_home else { return Ok(Some(home)) };
    let node_root = PathBuf::from(links::node_path(&root, Some(&node_home)));
    let code = node_root.join("deploy");
    let at = |path: &Path| path.to_string_lossy().into_owned();

    home.head = Launch {
      command: self.head_command(&row.name, &code),
      cwd: at(&code),
      env: BTreeMap::from([
        ("HOME".to_owned(), node_home.clone()),
        (
          "PATH".to_owned(),
          [
            format!("{node_home}/.bun/bin"),
            format!("{node_home}/.local/bin"),
            format!("{node_home}/.cargo/bin"),
            at(&node_root.join("deploy").join("bin")),
            "/usr/local/bin".into(),
            "/usr/bin".into(),
            "/bin".into(),
          ]
          .join(":"),
        ),
        ("LANG".to_owned(), self.vars.get("LANG").cloned().unwrap_or_else(|| "C.UTF-8".into())),
        ("BMO_DATA_DIR".to_owned(), at(&node_root.join("data"))),
        ("BMO_SOURCE_DIR".to_owned(), at(&code)),
        ("BMO_HOME_BRIDGE".to_owned(), at(&node_root.join("data").join("bridge.sock"))),
        ("BMO_NODE".to_owned(), node.clone()),
        (RUN_NAME.to_owned(), format!("{HEAD_MARK}{}", row.name)),
      ]),
    };

    Ok(Some(home))
  }

  /// The directory a worker works in: its own, or the source root.
  pub fn checkout_of(&self, row: &AgentRow) -> String {
    row.cwd.clone().unwrap_or_else(|| self.source.to_string_lossy().into_owned())
  }
}
