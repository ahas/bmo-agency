//! The workplaces on the work nodes (docs/work-nodes.md, 「The workplace on a node」): every workplace
//! with its state and its link; `setup` stands one — the tools, installed into the node's account with
//! `--install`, the deploy tree's commit copied and built there, the herdr session, the checkouts placed
//! there cloned, set up and trusted — and writes down what is still missing; `check` reads the same
//! without writing anything on the node; `sync` copies and builds the code again after a deployment;
//! `runs` says whether heavy runs go to it; `remove` lets it go.

use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use clap::Subcommand;
use futures_util::future::BoxFuture;
use policy::providers::claude_code::CLAUDE_CODE;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use store::{all, one, run};

use super::work_node_setup::{
  COPY_LEFT_OUT, SetupCheckout, SetupHands, SetupLogin, SetupOutcome, SetupPlace, SetupShell, ShellRun,
  set_up_workplace, shell_word,
};
use super::{Answer, VerbError, deploy_link, home, service, tell_server};
use crate::control::claude_accounts::accounts;
use crate::peers::node::NodeService;
use crate::watches::links::{link_unit, node_path};

/// The root a workplace takes where its setup names none, under the node's home.
pub const DEFAULT_WORKPLACE_ROOT: &str = "~/.local/share/bmo-node";

/// The herdr session the deployment's agents stand in on a node, apart from any session a person keeps
/// there.
pub const HERDR_SESSION: &str = "bmo";

/// The service a node's role names to carry a worker's jobs (docs/work-nodes.md, 「Which machines take
/// work」).
const JOBS_SERVICE: &str = "jobs";

// The longest any one step may take there: a cold build of the node's binaries takes the most.
const COMMAND_MOST_SECONDS: u32 = 2_400;

// How long a link may go without a word from the node before it is taken as down.
const HEARD_WITHIN_MS: i64 = 20_000;

#[derive(Subcommand, Clone, Debug)]
pub enum WorkNodeVerb {
  /// Stands the workplace up, installing missing tools into the node's account with --install.
  Setup {
    node: String,
    #[arg(long)]
    root: Option<String>,
    #[arg(long)]
    install: bool,
  },
  /// Reads the workplace as the setup would, writing nothing on the node.
  Check { node: String },
  /// Copies and builds the code again after a deployment.
  Sync { node: String },
  /// Whether heavy runs go to the workplace.
  Runs {
    node: String,
    #[arg(value_parser = ["on", "off"])]
    switched: String,
  },
  /// Lets the workplace go; what stands on the node is left as it is.
  Remove { node: String },
}

/// One workplace as its row holds it.
#[derive(Clone, Debug)]
pub struct Workplace {
  pub node: String,
  pub root: String,
  pub home: Option<String>,
  pub runs: bool,
  pub state: String,
  pub detail: Option<String>,
  pub checked_at: Option<i64>,
}

fn workplace_of(row: &sea_orm::QueryResult) -> Result<Workplace, VerbError> {
  Ok(Workplace {
    node: row.try_get("", "node")?,
    root: row.try_get("", "root")?,
    home: row.try_get("", "home")?,
    runs: row.try_get::<i64>("", "runs")? != 0,
    state: row.try_get("", "state")?,
    detail: row.try_get("", "detail")?,
    checked_at: row.try_get("", "checked_at")?,
  })
}

/// Every workplace, by node.
pub async fn workplaces(db: &DatabaseConnection) -> Result<Vec<Workplace>, VerbError> {
  all(db, r#"SELECT * FROM "node_workplaces" ORDER BY "node""#, vec![])
    .await?
    .iter()
    .map(workplace_of)
    .collect()
}

/// One node's workplace.
pub async fn workplace(db: &DatabaseConnection, node: &str) -> Result<Option<Workplace>, VerbError> {
  one(db, r#"SELECT * FROM "node_workplaces" WHERE "node" = $1"#, vec![node.into()])
    .await?
    .as_ref()
    .map(workplace_of)
    .transpose()
}

fn now_seconds() -> i64 {
  chrono::Utc::now().timestamp()
}

/// A workplace written where none stands, or its root moved; a root moved clears the last check.
pub async fn place_workplace(
  db: &DatabaseConnection,
  node: &str,
  root: &str,
) -> Result<Workplace, VerbError> {
  let at = now_seconds();

  match workplace(db, node).await? {
    None => {
      run(
        db,
        r#"INSERT INTO "node_workplaces" ("node", "root", "runs", "state", "created_at", "updated_at") VALUES ($1, $2, 0, 'unknown', $3, $3)"#,
        vec![node.into(), root.into(), at.into()],
      )
      .await?;
    }
    Some(before) if before.root != root => {
      run(
        db,
        r#"UPDATE "node_workplaces" SET "root" = $1, "state" = 'unknown', "detail" = NULL, "checked_at" = NULL, "updated_at" = $2 WHERE "node" = $3"#,
        vec![root.into(), at.into(), node.into()],
      )
      .await?;
    }
    Some(_) => {}
  }

  workplace(db, node)
    .await?
    .ok_or_else(|| VerbError::Refused(format!("{node} has no workplace")))
}

/// Why a node takes no work, or none where it may: a node the node service names an SSH login to, whose
/// state it says is not reserved for a service other than the jobs.
pub fn work_refusal(name: &str, logs_in: bool, role: Option<&str>) -> Option<String> {
  if !logs_in {
    return Some(format!("{name} has no SSH route to carry a job"));
  }

  role
    .filter(|role| !role.split('+').any(|service| service == JOBS_SERVICE))
    .map(|role| format!("{name} is reserved for {role} and carries no {JOBS_SERVICE}"))
}

/// A moment in epoch seconds as the listings write it, in UTC.
pub fn clock(seconds: Option<i64>) -> String {
  seconds
    .and_then(|at| chrono::DateTime::from_timestamp(at, 0))
    .map_or_else(|| "never".to_owned(), |at| at.format("%Y-%m-%d %H:%M:%S UTC").to_string())
}

// Where this machine keeps what it knows of a node.
fn node_dir(data_root: &Path, node: &str) -> PathBuf {
  data_root.join("nodes").join(node)
}

fn read_json(path: &Path) -> Option<Value> {
  serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

// How long ago a moment was, in whole minutes.
fn ago(seconds: Option<i64>) -> String {
  seconds.map_or_else(
    || "never".to_owned(),
    |at| format!("{}m ago", ((now_seconds() - at) as f64 / 60.0).round().max(0.0) as i64),
  )
}

// How a node's link stands, as its far end last wrote it.
fn link_words(data_root: &Path, node: &str) -> String {
  let Some(state) = read_json(&node_dir(data_root, node).join("link-state.json")) else {
    return "no link".to_owned();
  };
  let heard = state.get("heard").and_then(Value::as_i64);
  let now = chrono::Utc::now().timestamp_millis();
  let up = state.get("state").and_then(Value::as_str) == Some("connected")
    && heard.is_some_and(|heard| now - heard <= HEARD_WITHIN_MS);

  if up {
    let since = state.get("since").and_then(Value::as_i64).unwrap_or(0);

    return format!("link up since {}", clock(Some(since.div_euclid(1000))));
  }

  match state.get("dropped").and_then(Value::as_str) {
    Some(dropped) => format!("link down: {dropped}"),
    None => "link down".to_owned(),
  }
}

/// Every workplace, its state and its link, as the bare verb prints them.
pub async fn listing(db: &DatabaseConnection, data_root: &Path) -> Result<Vec<String>, VerbError> {
  let held = workplaces(db).await?;

  if held.is_empty() {
    return Ok(vec!["no workplace on any node".to_owned()]);
  }

  let mut lines = Vec::new();

  for one in held {
    lines.push(format!(
      "{:<12} {:<11} runs {} root {} · checked {} · {}",
      one.node,
      one.state,
      if one.runs { "on " } else { "off" },
      one.root,
      ago(one.checked_at),
      link_words(data_root, &one.node)
    ));

    for line in one.detail.as_deref().unwrap_or_default().split('\n').filter(|line| !line.is_empty()) {
      lines.push(format!("  - {line}"));
    }
  }

  Ok(lines)
}

/// The node's shell over the one connection the first command makes and the rest share: a node may refuse
/// a run of logins made in quick succession, as a firewall limiting SSH does.
pub struct SharedLogin {
  pub login: Vec<String>,
}

impl SharedLogin {
  /// The batch login the node service names, held open for a run of commands, kept a minute past the last
  /// of them, the arguments ending at the destination.
  pub fn new(mut login: Vec<String>) -> Self {
    let destination = login.pop().unwrap_or_default();
    let control = shared_login_path();

    login.extend([
      "-o".to_owned(),
      "ControlMaster=auto".to_owned(),
      "-o".to_owned(),
      format!("ControlPath={}", control.display()),
      "-o".to_owned(),
      "ControlPersist=60".to_owned(),
      destination,
    ]);

    Self { login }
  }

  fn destination(&self) -> &str {
    self.login.last().map_or("", String::as_str)
  }

  fn options(&self) -> &[String] {
    &self.login[..self.login.len().saturating_sub(1)]
  }

  /// The port the login names, 22 where it names none.
  fn port(&self) -> u16 {
    self
      .login
      .windows(2)
      .find(|pair| pair[0] == "-p")
      .and_then(|pair| pair[1].parse().ok())
      .unwrap_or(22)
  }
}

// Where the connection a run of commands shares is held: under the runtime directory, since a socket's
// path holds at most 107 bytes and a data root may be deep. OpenSSH names it by the login it holds.
fn shared_login_path() -> PathBuf {
  let runtime = std::env::var_os("XDG_RUNTIME_DIR").filter(|dir| !dir.is_empty()).map_or_else(
    // SAFETY: getuid reads the process's own user id and cannot fail.
    || PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })),
    PathBuf::from,
  );

  runtime.join("agency").join("ssh-%C")
}

async fn captured(mut command: tokio::process::Command) -> ShellRun {
  command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());

  match command.output().await {
    Ok(done) => ShellRun {
      code: done.status.code().unwrap_or(255),
      stdout: String::from_utf8_lossy(&done.stdout).into_owned(),
      stderr: String::from_utf8_lossy(&done.stderr).into_owned(),
    },
    Err(why) => ShellRun {
      code: 255,
      stdout: String::new(),
      stderr: why.to_string(),
    },
  }
}

impl SetupShell for SharedLogin {
  fn run<'a>(&'a self, command: &'a str) -> BoxFuture<'a, ShellRun> {
    Box::pin(async move {
      // Bounded on the node itself: the shared connection holds a command's output for as long as the
      // command runs there, so only its end there ends the wait here.
      let mut ssh = tokio::process::Command::new("ssh");

      ssh
        .args(&self.login)
        .arg(format!("timeout {COMMAND_MOST_SECONDS} bash -c {}", shell_word(command)));
      captured(ssh).await
    })
  }
}

// What the setup may do on this machine's side: copy over the shared login, save the node in herdr.
struct Hands<'a> {
  install: bool,
  copy_code: bool,
  login: &'a SharedLogin,
  label: String,
}

fn said_tail(text: &str) -> String {
  let lines: Vec<&str> = text.trim().split('\n').collect();

  lines[lines.len().saturating_sub(3)..].join(" ")
}

impl SetupHands for Hands<'_> {
  fn install(&self) -> bool {
    self.install
  }

  fn copy_code(&self) -> bool {
    self.copy_code
  }

  fn copy<'a>(&'a self, from: &'a str, to: &'a str) -> BoxFuture<'a, (bool, String)> {
    Box::pin(async move {
      let ssh = std::iter::once("ssh".to_owned())
        .chain(self.login.options().iter().cloned())
        .map(|word| shell_word(&word))
        .collect::<Vec<_>>()
        .join(" ");
      let mut rsync = tokio::process::Command::new("rsync");

      rsync.args(["-a", "--delete"]);

      for pattern in COPY_LEFT_OUT {
        rsync.args(["--exclude", pattern]);
      }

      rsync.args(["-e", &ssh, from, to]);

      let ran = captured(rsync).await;

      (ran.code == 0, said_tail(&ran.stderr))
    })
  }

  fn add_machine(&self) -> BoxFuture<'_, (bool, String)> {
    Box::pin(async move {
      let destination = self.login.destination();
      let port = self.login.port();
      let machine = if port == 22 { destination.to_owned() } else { format!("ssh://{destination}:{port}") };
      // Saved once: herdr keeps every machine added as a profile of its own, the same target or not.
      let mut listed = tokio::process::Command::new("herdr");

      listed.args(["machine", "list", "--json"]);

      let saved: Vec<Value> = serde_json::from_str(&captured(listed).await.stdout).unwrap_or_default();

      if saved.iter().any(|profile| {
        profile.get("target").and_then(Value::as_str) == Some(machine.as_str())
          && profile.get("session").and_then(Value::as_str) == Some(HERDR_SESSION)
      }) {
        return (true, "already saved".to_owned());
      }

      let mut add = tokio::process::Command::new("herdr");

      add.args(["machine", "add", "--label", &self.label, "--remote-session", HERDR_SESSION, &machine]);

      let ran = captured(add).await;

      (ran.code == 0, said_tail(&format!("{}{}", ran.stdout, ran.stderr)))
    })
  }

  fn say(&self, line: &str) {
    println!("{line}");
    let _ = std::io::stdout().flush();
  }
}

// What a program on this machine says its version is: its first line's last word, or the word after
// a name.
fn local_version(program: &str, args: &[&str], after: Option<&str>) -> Option<String> {
  let done = std::process::Command::new(program).args(args).stderr(Stdio::null()).output().ok()?;
  let first = String::from_utf8_lossy(&done.stdout).lines().next()?.trim().to_owned();

  match after {
    Some(name) => first.strip_prefix(name)?.split_whitespace().next().map(str::to_owned),
    None => (!first.is_empty()).then_some(first),
  }
}

// Where a program stands on this machine's search path, its links read through.
fn local_program(program: &str) -> Option<String> {
  let path = std::env::var_os("PATH")?;

  std::env::split_paths(&path)
    .map(|dir| dir.join(program))
    .find(|candidate| candidate.is_file())
    .and_then(|found| std::fs::canonicalize(found).ok())
    .map(|found| found.to_string_lossy().into_owned())
}

// The remote a checkout's base clones from: its origin.
fn remote_of(base_cwd: &str) -> Option<String> {
  let done = std::process::Command::new("git")
    .args(["-C", base_cwd, "remote", "get-url", "origin"])
    .stderr(Stdio::null())
    .output()
    .ok()?;

  (done.status.success())
    .then(|| String::from_utf8_lossy(&done.stdout).trim().to_owned())
    .filter(|url| !url.is_empty())
}

// The node's place in this machine's paths: each of this machine's roots read as the node's, the longest
// first.
fn to_node_path(path: &str, data_root: &Path, node_home: &str, node_root: &str) -> String {
  let installed = home().join(".local/share/bmo");
  let agents = std::env::var_os("BMO_AGENTS_DIR").map_or_else(|| installed.join("agents"), PathBuf::from);
  let mut map = vec![
    (data_root.to_string_lossy().into_owned(), format!("{node_root}/data")),
    (deploy_link().to_string_lossy().into_owned(), format!("{node_root}/deploy")),
    (agents.to_string_lossy().into_owned(), format!("{node_root}/agents")),
    (home().to_string_lossy().into_owned(), node_home.to_owned()),
  ];

  map.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));

  for (from, to) in map {
    if path == from {
      return to;
    }

    if let Some(rest) = path.strip_prefix(&format!("{from}/")) {
      return format!("{to}/{rest}");
    }
  }

  path.to_owned()
}

// The logins a run on the node may go out on, each account as this machine keeps it.
async fn logins(db: &DatabaseConnection, data_root: &Path) -> Result<Vec<SetupLogin>, VerbError> {
  let held = accounts(db).await.map_err(|refused| VerbError::Refused(refused.error))?;
  let config_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
    .filter(|dir| !dir.is_empty())
    .map_or_else(|| home().join(".claude"), PathBuf::from);
  let deploy = deploy_link();

  Ok(
    held
      .iter()
      .map(|account| SetupLogin {
        account: account.name.clone(),
        kind: account.credential_kind.clone(),
        path: if account.credential_kind == "directory" {
          account.credential_path.clone()
        } else {
          config_dir.to_string_lossy().into_owned()
        },
        node_token: account.node_token(data_root).is_some(),
        token_command: CLAUDE_CODE
          .node_login
          .command
          .replace("{deploy}", &deploy.to_string_lossy())
          .replace("{name}", &account.name.replace('\'', r"'\''")),
      })
      .collect(),
  )
}

// The setup, a check or a sync of one node's workplace; whether nothing is missing.
async fn stand(
  db: &DatabaseConnection,
  nodes: &dyn NodeService,
  data_root: &Path,
  verb: &WorkNodeVerb,
) -> Result<Answer, VerbError> {
  let (node, root, install, copy_code) = match verb {
    WorkNodeVerb::Setup {
      node,
      root,
      install,
    } => (node, root.clone(), *install, true),
    WorkNodeVerb::Sync { node } => (node, None, false, true),
    WorkNodeVerb::Check { node } => (node, None, false, false),
    _ => return Err(VerbError::Refused("no such step".into())),
  };
  let unreachable = |fault: crate::peers::PeerFault| VerbError::Refused(fault.to_string());
  let not_ssh = || VerbError::Refused(format!("{node} is no node reached over SSH"));
  let row = nodes
    .node_rows()
    .await
    .map_err(unreachable)?
    .into_iter()
    .find(|row| row.node == *node && row.kind.over_ssh())
    .ok_or_else(not_ssh)?;
  let login = nodes.ssh_login(node).await.map_err(unreachable)?.ok_or_else(not_ssh)?;

  if let Some(refused) = work_refusal(&row.node, true, None) {
    return Err(VerbError::Refused(format!("{node} takes no work: {refused}")));
  }

  let held = workplace(db, node).await?;

  if held.is_none() && !matches!(verb, WorkNodeVerb::Setup { .. }) {
    return Err(VerbError::Refused(format!("{node} has no workplace; set it up first")));
  }

  let root = root
    .or_else(|| held.map(|held| held.root))
    .unwrap_or_else(|| DEFAULT_WORKPLACE_ROOT.to_owned());
  let workplace = place_workplace(db, node, &root).await?;
  let workers = all(
    db,
    r#"SELECT "name", "cwd", "support_of" FROM "agents" WHERE "kind" = 'worker' AND "resting_since" IS NULL AND "node" = $1 AND "cwd" IS NOT NULL ORDER BY "builtin" DESC, "name""#,
    vec![node.clone().into()],
  )
  .await?;
  let mut checkouts = Vec::new();

  for worker in &workers {
    let support_of: Option<String> = worker.try_get("", "support_of")?;
    let base_cwd = match support_of {
      Some(base) => one(db, r#"SELECT "cwd" FROM "agents" WHERE "name" = $1"#, vec![base.into()])
        .await?
        .map(|row| row.try_get::<Option<String>>("", "cwd"))
        .transpose()?
        .flatten(),
      None => None,
    };

    checkouts.push(SetupCheckout {
      worker: worker.try_get("", "name")?,
      cwd: worker.try_get("", "cwd")?,
      remote: base_cwd.as_deref().and_then(remote_of),
    });
  }

  std::fs::DirBuilder::new()
    .recursive(true)
    .mode(0o700)
    .create(node_dir(data_root, node))
    .map_err(|why| VerbError::Refused(why.to_string()))?;

  let shell = SharedLogin::new(login);
  let to_node =
    |path: &str, node_home: &str, node_root: &str| to_node_path(path, data_root, node_home, node_root);
  let place = SetupPlace {
    node: node.clone(),
    root: workplace.root.clone(),
    deploy: deploy_link().to_string_lossy().into_owned(),
    checkouts: checkouts.clone(),
    logins: logins(db, data_root).await?,
    to_node: &to_node,
    destination: shell.destination().to_owned(),
    herdr: local_program("herdr"),
    rust: local_version("rustc", &["--version"], Some("rustc ")),
    bun: local_version("bun", &["--version"], None),
    session: HERDR_SESSION.to_owned(),
  };
  let hands = Hands {
    install,
    copy_code,
    login: &shell,
    label: row.label.clone().unwrap_or_else(|| row.node.clone()),
  };
  let outcome: SetupOutcome = set_up_workplace(&shell, &place, &hands).await;

  // A path written from the node's home reads as the node's own from now on, so every lock, record and
  // transcript of the checkout names the one path.
  if let Some(node_home) = &outcome.home {
    for checkout in &checkouts {
      let absolute = node_path(&checkout.cwd, Some(node_home));

      if absolute != checkout.cwd {
        run(
          db,
          r#"UPDATE "agents" SET "cwd" = $1 WHERE "name" = $2"#,
          vec![absolute.into(), checkout.worker.clone().into()],
        )
        .await?;
      }
    }
  }

  if let Some(socket) = &outcome.herdr_socket {
    let path = node_dir(data_root, node).join("setup.json");
    // The commit the code on the node now stands at, where this run copied it, so agency knows when a
    // deployment has moved past it.
    let commit = if copy_code {
      read_json(&data_root.join("deployed.json")).and_then(|record| record.get("commit").cloned())
    } else {
      read_json(&path).and_then(|setup| setup.get("commit").cloned())
    };
    let body = format!(
      "{}\n",
      serde_json::to_string_pretty(&json!({ "herdrSession": HERDR_SESSION, "herdrSocket": socket, "commit": commit.unwrap_or(Value::Null) }))
        .unwrap_or_default()
    );
    let next = PathBuf::from(format!("{}.{}", path.display(), std::process::id()));
    let written = std::fs::OpenOptions::new()
      .write(true)
      .create(true)
      .truncate(true)
      .mode(0o600)
      .open(&next)
      .and_then(|mut file| file.write_all(body.as_bytes()))
      .and_then(|()| std::fs::rename(&next, &path));

    written.map_err(|why| VerbError::Refused(format!("{}: {why}", path.display())))?;
  }

  let state = match (&outcome.home, outcome.missing.is_empty()) {
    (None, _) => "unreachable",
    (Some(_), true) => "ready",
    (Some(_), false) => "missing",
  };
  let detail = (!outcome.missing.is_empty()).then(|| outcome.missing.join("\n"));
  let at = now_seconds();

  run(
    db,
    r#"UPDATE "node_workplaces" SET "state" = $1, "detail" = $2, "home" = COALESCE($3, "home"), "checked_at" = $4, "updated_at" = $4 WHERE "node" = $5"#,
    vec![state.into(), detail.into(), outcome.home.clone().into(), at.into(), node.clone().into()],
  )
  .await?;

  // A copy or a build moved what the link's far end runs, so the link is stood again on it.
  if copy_code {
    let _ = tokio::process::Command::new("systemctl")
      .args(["--user", "stop", &format!("{}.service", link_unit(node))])
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status()
      .await;
  }

  tell_server("/workers/stand", json!({})).await;

  Ok(if outcome.missing.is_empty() {
    Answer::from(vec![format!("{node}: ready ({})", outcome.root.unwrap_or_default())])
  } else {
    let mut lines = vec![format!("{node}: not ready")];

    lines.extend(outcome.missing.iter().map(|line| format!("  - {line}")));
    Answer { lines, code: 1 }
  })
}

/// One run of `bmo-agency work-node`.
pub async fn work_node(data_root: &Path, verb: Option<WorkNodeVerb>) -> Result<Answer, VerbError> {
  let service = service(data_root).await?;

  work_node_on(&service.db, service.peers.node.as_ref(), data_root, verb).await
}

/// One run of `bmo-agency work-node` on a store already open.
pub async fn work_node_on(
  db: &DatabaseConnection,
  nodes: &dyn NodeService,
  data_root: &Path,
  verb: Option<WorkNodeVerb>,
) -> Result<Answer, VerbError> {
  match verb {
    None => listing(db, data_root).await.map(Answer::from),
    Some(WorkNodeVerb::Runs { node, switched }) => {
      let changed = run(
        db,
        r#"UPDATE "node_workplaces" SET "runs" = $1, "updated_at" = $2 WHERE "node" = $3"#,
        vec![i64::from(switched == "on").into(), now_seconds().into(), node.clone().into()],
      )
      .await?;

      if changed == 0 {
        return Err(VerbError::Refused(format!("{node} has no workplace; set it up first")));
      }

      Ok(Answer::from(vec![format!("{node}: heavy runs {switched}")]))
    }
    Some(WorkNodeVerb::Remove { node }) => {
      if run(db, r#"DELETE FROM "node_workplaces" WHERE "node" = $1"#, vec![node.clone().into()]).await?
        == 0
      {
        return Err(VerbError::Refused(format!("{node} has no workplace")));
      }

      tell_server("/workers/stand", json!({})).await;

      Ok(Answer::from(vec![format!(
        "{node}: its workplace is no longer one; what stands on the node is left as it is"
      )]))
    }
    Some(verb) => stand(db, nodes, data_root, &verb).await,
  }
}
