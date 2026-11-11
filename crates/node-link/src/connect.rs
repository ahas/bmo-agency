//! The near end of a node's link (docs/work-nodes.md, 「The link」), the standing process of its unit:
//! it holds one SSH connection to the node carrying the herdr socket, the tool host's port and the home
//! bridge, keeps the mirror both ways over the session's standing input and output, answers the bot on
//! its control socket, and makes the connection again whenever it drops.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::bridge;
use crate::frame::{Frame, decode, encode};
use crate::rewrite::{NodeEnv, rewrite_json};
use crate::serve::write_whole;
use crate::spec::Spec;

/// How often the pushed files are looked at, how often the spec is, and the bounds of the wait before
/// the connection is made again.
const PUSH_EVERY: Duration = Duration::from_millis(300);
const SPEC_EVERY: Duration = Duration::from_secs(2);
const FIRST_WAIT: Duration = Duration::from_secs(1);
const LONGEST_WAIT: Duration = Duration::from_secs(60);
/// How often the state file is written while frames keep arriving.
const HEARD_EVERY: Duration = Duration::from_secs(2);

fn now_ms() -> u128 {
  SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis())
}

/// What the bot reads of the link: whether it is up, since when, when it last heard the node, the
/// node's home, and why it last dropped.
#[derive(Default)]
pub struct State {
  connected_since: Option<u128>,
  heard: Option<u128>,
  home: Option<String>,
  dropped: Option<String>,
  written: Option<u128>,
}

impl State {
  fn write(&mut self, spec: &Spec) {
    let body = json!({
      "node": spec.node,
      "state": if self.connected_since.is_some() { "connected" } else { "down" },
      "since": self.connected_since,
      "heard": self.heard,
      "home": self.home,
      "dropped": self.dropped,
      "pid": std::process::id(),
    });

    let _ = write_whole(&spec.state, format!("{body}\n").as_bytes(), 0o600);
    self.written = Some(now_ms());
  }
}

/// The sender of the connection standing now, which the control socket hands a signal to.
type Current = Arc<Mutex<Option<mpsc::UnboundedSender<Frame>>>>;

/// A path under a pull's landing, refused where it would step out of it.
pub fn landed(root: &Path, relative: &str) -> Option<PathBuf> {
  let path = Path::new(relative);

  path
    .components()
    .all(|part| matches!(part, Component::Normal(_)))
    .then(|| root.join(path))
}

/// The SSH command for one connection: the policy's login with the forwards and the far end's command.
fn ssh_command(spec: &Spec, remote_bridge: &str) -> Command {
  let (destination, options) = spec
    .ssh
    .split_last()
    .map_or((String::new(), &[][..]), |(last, rest)| (last.clone(), rest));
  // A test names a stand-in for OpenSSH that runs the far end here.
  let mut command = Command::new(std::env::var("NODE_LINK_SSH").unwrap_or_else(|_| "ssh".to_owned()));

  command.args(options).args([
    "-o",
    "ExitOnForwardFailure=yes",
    "-o",
    "ServerAliveInterval=10",
    "-o",
    "ServerAliveCountMax=3",
    "-o",
    "StreamLocalBindUnlink=yes",
  ]);
  command.arg("-L").arg(format!("{}:{}", spec.herdr.local.display(), spec.herdr.remote));
  command.arg("-R").arg(format!("{remote_bridge}:{}", spec.bridge.display()));

  for port in &spec.ports {
    command.arg("-R").arg(format!("127.0.0.1:{port}:127.0.0.1:{port}"));
  }

  command.arg(destination).args([
    spec.binary.as_str(),
    "serve",
    "--root",
    spec.root.as_str(),
    "--bridge",
    remote_bridge,
    "--herdr-socket",
    spec.herdr.remote.as_str(),
  ]);

  if let Some(session) = &spec.herdr.session {
    command.args(["--herdr-session", session.as_str()]);
  }
  command
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);

  command
}

// A connection to a port on a person's PC: the hub's connect program, its words ahead of the agent and the port.
fn jake_command(jake: &crate::spec::Jake, port: u16) -> Command {
  let (program, words) =
    jake.connect.split_first().map_or(("", &[][..]), |(one, rest)| (one.as_str(), rest));
  let mut command = Command::new(program);

  command
    .args(words)
    .args([jake.agent.as_str(), &port.to_string()])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);
  command
}

// What a link to a person's PC holds beside its mirror, for as long as the connection stands: the herdr socket
// here joined to the far end's herdr port, the bridge's stream back joined to the bridge socket here, and the hub
// asked to listen on the PC for the tool host and the bridge (docs/person-nodes.md, 「The link over jake」).
fn jake_sides(spec: &Spec, jake: &crate::spec::Jake) -> Vec<tokio::task::JoinHandle<()>> {
  let mut sides = Vec::new();
  let _ = std::fs::remove_file(&spec.herdr.local);

  match UnixListener::bind(&spec.herdr.local) {
    Ok(listener) => {
      let jake = jake.clone();
      sides.push(tokio::spawn(async move {
        while let Ok((client, _)) = listener.accept().await {
          let jake = jake.clone();
          tokio::spawn(async move { carry_over_jake(client, &jake, jake.herdr_port).await });
        }
      }));
    }
    Err(error) => eprintln!("{}: {}: {error}", spec.node, spec.herdr.local.display()),
  }

  let bridge = spec.bridge.clone();
  let relay = jake.bridge_relay;
  sides.push(tokio::spawn(async move {
    let Ok(listener) = tokio::net::TcpListener::bind(("127.0.0.1", relay)).await else {
      return;
    };
    while let Ok((mut client, _)) = listener.accept().await {
      let bridge = bridge.clone();
      tokio::spawn(async move {
        if let Ok(mut far) = tokio::net::UnixStream::connect(&bridge).await {
          let _ = tokio::io::copy_bidirectional(&mut client, &mut far).await;
        }
      });
    }
  }));

  let mut wanted: Vec<(u16, u16)> = spec.ports.iter().map(|port| (*port, *port)).collect();
  wanted.push((jake.bridge_port, jake.bridge_relay));
  let (agent, hub_env, node) = (jake.agent.clone(), jake.hub_env.clone(), spec.node.clone());
  sides.push(tokio::spawn(async move {
    if let Err(error) = ask_listens(&agent, &hub_env, &wanted).await {
      eprintln!("{node}: the hub was not asked to listen: {error}");
    }
  }));

  sides
}

// One connection here carried to a port on the PC through the hub's connect program.
async fn carry_over_jake(client: tokio::net::UnixStream, jake: &crate::spec::Jake, port: u16) {
  let Ok(mut child) = jake_command(jake, port).stderr(Stdio::null()).spawn() else {
    return;
  };
  let (Some(mut input), Some(mut output)) = (child.stdin.take(), child.stdout.take()) else {
    return;
  };
  let (mut reader, mut writer) = client.into_split();

  tokio::select! {
    _ = tokio::io::copy(&mut reader, &mut input) => {}
    _ = tokio::io::copy(&mut output, &mut writer) => {}
  }
  let _ = child.start_kill();
}

// The hub asked to have the agent listen on each PC port, joined to the port here, with the control port and
// token its own file names.
async fn ask_listens(agent: &str, hub_env: &Path, wanted: &[(u16, u16)]) -> Result<(), String> {
  let mut port = 8791u16;
  let mut token = String::new();

  for item in dotenvy::from_path_iter(hub_env).map_err(|error| format!("{}: {error}", hub_env.display()))? {
    let (key, value) = item.map_err(|error| format!("{}: {error}", hub_env.display()))?;
    match key.as_str() {
      "JAKE_CONTROL_PORT" => {
        port = value.parse().map_err(|_| format!("JAKE_CONTROL_PORT {value} is no port"))?
      }
      "JAKE_CONTROL_TOKEN" => token = value,
      _ => {}
    }
  }

  let client = reqwest::Client::new();
  for (pc, here) in wanted {
    let answer = client
      .post(format!("http://127.0.0.1:{port}/listen"))
      .query(&[("agent", agent), ("port", &pc.to_string())])
      .bearer_auth(&token)
      .json(&json!({ "target": format!("127.0.0.1:{here}") }))
      .send()
      .await
      .map_err(|error| error.to_string())?;
    if !answer.status().is_success() {
      let said = answer.text().await.unwrap_or_default();
      return Err(format!("port {pc}: {said}"));
    }
  }
  Ok(())
}

/// Whether two specs make the same connection, so a change between them needs no new one.
fn same_transport(one: &Spec, other: &Spec) -> bool {
  one.ssh == other.ssh
    && one.jake == other.jake
    && one.herdr.local == other.herdr.local
    && one.herdr.remote == other.herdr.remote
    && one.herdr.session == other.herdr.session
    && one.ports == other.ports
    && one.root == other.root
    && one.binary == other.binary
}

fn spec_stamp(path: &Path) -> Option<(i64, i64, u64)> {
  fs::metadata(path).ok().map(|meta| (meta.mtime(), meta.mtime_nsec(), meta.len()))
}

fn file_stamp(path: &Path) -> Option<(i64, i64, u64, u64)> {
  fs::metadata(path)
    .ok()
    .filter(fs::Metadata::is_file)
    .map(|meta| (meta.mtime(), meta.mtime_nsec(), meta.len(), meta.ino()))
}

/// The pushed files of a spec and what was last sent of each, kept for one connection.
struct Pushing {
  sent: HashMap<PathBuf, (i64, i64, u64, u64)>,
  // Files a pushed program named under the data root, which go to the node beside it.
  followers: BTreeSet<PathBuf>,
}

impl Pushing {
  fn new() -> Self {
    Self {
      sent: HashMap::new(),
      followers: BTreeSet::new(),
    }
  }

  /// Whether a file named by a pushed one may go along: under a root that follows, under none that is
  /// withheld, and not one the node itself sends back.
  fn may_follow(spec: &Spec, path: &Path) -> bool {
    spec.follow.iter().any(|root| path.starts_with(root))
      && !spec.withheld.iter().any(|root| path.starts_with(root))
      && !spec.pull.iter().any(|landing| path.starts_with(&landing.local))
  }

  fn body(
    spec: &Spec,
    node_path: Option<&str>,
    path: &Path,
    rewrite: bool,
    followers: &mut BTreeSet<PathBuf>,
  ) -> Option<Vec<u8>> {
    let bytes = fs::read(path).ok()?;

    if !rewrite || path.extension().is_none_or(|extension| extension != "json") {
      return Some(bytes);
    }

    let Ok(mut value) = serde_json::from_slice::<Value>(&bytes) else { return Some(bytes) };
    let mut named = Vec::new();

    let env = NodeEnv {
      keep: &spec.env_keep,
      path: node_path,
      add: &spec.env_add,
    };

    rewrite_json(&mut value, &spec.rewrite, &env, &spec.follow, &mut named);

    for one in named {
      let path = PathBuf::from(one);

      if Self::may_follow(spec, &path) && path.is_file() {
        followers.insert(path);
      }
    }

    Some(format!("{value}\n").into_bytes())
  }

  /// Every pushed file that changed since it was last sent, sent; every one gone, said gone.
  fn push(&mut self, spec: &Spec, node_path: Option<&str>, send: &mpsc::UnboundedSender<Frame>) {
    let mut wanted: Vec<(PathBuf, bool)> = spec
      .push
      .iter()
      .flat_map(|push| push.files.iter().map(move |name| (push.local.join(name), push.rewrite)))
      .collect();

    wanted.extend(self.followers.iter().map(|path| (path.clone(), true)));

    for (path, rewrite) in wanted {
      let remote = spec.to_node(&path.to_string_lossy());

      match file_stamp(&path) {
        None => {
          if self.sent.remove(&path).is_some() {
            let _ = send.send(Frame::Gone {
              pull: None,
              path: remote,
            });
          }
        }
        Some(stamp) if self.sent.get(&path) == Some(&stamp) => {}
        Some(stamp) => {
          let Some(bytes) = Self::body(spec, node_path, &path, rewrite, &mut self.followers) else {
            continue;
          };

          let _ = send.send(Frame::Put {
            pull: None,
            path: remote,
            data: encode(&bytes),
            mode: Some(0o600),
          });
          self.sent.insert(path, stamp);
        }
      }
    }
  }
}

/// A frame the node sent, put where it lands here.
pub fn apply(spec: &Spec, frame: Frame, state: &mut State) {
  match frame {
    Frame::Hello { home, .. } => {
      state.home = Some(home);
    }
    Frame::Put {
      pull: Some(index),
      path,
      data,
      mode,
    } => {
      let (Some(landing), Some(bytes)) = (spec.pull.get(index), decode(&data)) else {
        return;
      };
      let Some(local) = landed(&landing.local, &path) else { return };

      let _ = write_whole(&local, &bytes, mode.unwrap_or(0o600) | 0o600);
    }
    Frame::Append {
      pull,
      path,
      offset,
      data,
    } => {
      let (Some(landing), Some(bytes)) = (spec.pull.get(pull), decode(&data)) else { return };
      let Some(local) = landed(&landing.local, &path) else { return };

      if let Some(dir) = local.parent() {
        let _ = fs::create_dir_all(dir);
      }

      let Ok(mut file) = fs::OpenOptions::new().create(true).truncate(false).write(true).open(&local)
      else {
        return;
      };

      // What the node holds from this offset on is what this file holds from it: anything after it
      // here was written by an earlier run of the same file that the node has since replaced.
      if file.set_len(offset).is_ok() && file.seek(SeekFrom::Start(offset)).is_ok() {
        let _ = file.write_all(&bytes);
      }
    }
    Frame::Gone {
      pull: Some(index),
      path,
    } => {
      if let Some(local) = spec.pull.get(index).and_then(|landing| landed(&landing.local, &path)) {
        let _ = fs::remove_file(local);
      }
    }
    Frame::Alive { pull, alive } => {
      if let Some(landing) = spec.pull.get(pull) {
        let body = json!({ "at": now_ms(), "alive": alive });

        let _ = write_whole(&landing.local.join("node-alive.json"), format!("{body}\n").as_bytes(), 0o600);
      }
    }
    Frame::Proc { files } => {
      for (name, text) in files {
        if let Some(local) = landed(&spec.proc, &name) {
          let _ = write_whole(&local, text.as_bytes(), 0o644);
        }
      }
    }
    Frame::Trouble { said } => eprintln!("{}: the node said: {said}", spec.node),
    _ => {}
  }
}

/// One connection, from its start to its end; answers why it ended.
async fn run_once(spec_path: &Path, spec: &mut Spec, current: &Current, state: &mut State) -> String {
  let remote_bridge = format!("{}/data/run/bridge-{}-{}.sock", spec.root, std::process::id(), now_ms());
  let (command, sides) = match &spec.jake {
    Some(jake) => (jake_command(jake, jake.mirror_port), jake_sides(spec, jake)),
    None => (ssh_command(spec, &remote_bridge), Vec::new()),
  };
  // The sides stand as long as this connection does.
  let _sides = Sides(sides);
  let mut command = command;
  let mut child = match command.spawn() {
    Ok(child) => child,
    Err(error) => return format!("the connection could not start: {error}"),
  };

  let (send, mut sending) = mpsc::unbounded_channel::<Frame>();
  let Some(mut input) = child.stdin.take() else { return "ssh gave no input".into() };
  let Some(output) = child.stdout.take() else { return "ssh gave no output".into() };
  let errors = child.stderr.take();

  let writer = tokio::spawn(async move {
    while let Some(frame) = sending.recv().await {
      if input.write_all(frame.line().as_bytes()).await.is_err() || input.flush().await.is_err() {
        break;
      }
    }
  });
  // OpenSSH's own last words, which say why a connection ended where the node said nothing.
  let last_said = Arc::new(Mutex::new(String::new()));
  let said = last_said.clone();
  let errors = tokio::spawn(async move {
    let Some(errors) = errors else { return };
    let mut lines = BufReader::new(errors).lines();

    while let Ok(Some(line)) = lines.next_line().await {
      if !line.trim().is_empty()
        && let Ok(mut held) = said.lock()
      {
        *held = line;
      }
    }
  });

  if let Ok(mut held) = current.lock() {
    *held = Some(send.clone());
  }

  let mut lines = BufReader::new(output).lines();
  let mut pushing = Pushing::new();
  let mut node_path: Option<String> = None;
  let mut push_tick = tokio::time::interval(PUSH_EVERY);
  let mut spec_tick = tokio::time::interval(SPEC_EVERY);
  let mut stamp = spec_stamp(spec_path);

  let reason = loop {
    tokio::select! {
      line = lines.next_line() => {
        let Ok(Some(line)) = line else { break "the session ended".to_owned() };
        let Some(frame) = Frame::read(&line) else { continue };
        let now = now_ms();

        // A connection made is said at once; while it stands, the state is written every few
        // seconds of frames.
        let hello = matches!(frame, Frame::Hello { .. });

        if let Frame::Hello { path, .. } = &frame {
          let head = spec.path_head.join(":");

          node_path = Some(if head.is_empty() { path.clone() } else { format!("{head}:{path}") });
          state.connected_since = Some(now);
          state.dropped = None;
          let _ = send.send(Frame::Watch { pulls: spec.pulls() });
          pushing = Pushing::new();
        }

        apply(spec, frame, state);
        state.heard = Some(now);

        if hello || state.written.is_none_or(|at| now.saturating_sub(at) >= HEARD_EVERY.as_millis()) {
          state.write(spec);
        }
      }
      _ = push_tick.tick() => {
        if node_path.is_some() {
          pushing.push(spec, node_path.as_deref(), &send);
        }
      }
      _ = spec_tick.tick() => {
        let now = spec_stamp(spec_path);

        if now != stamp {
          stamp = now;

          match Spec::read(spec_path) {
            Ok(read) if same_transport(spec, &read) => {
              *spec = read;
              let _ = send.send(Frame::Watch { pulls: spec.pulls() });
            }
            Ok(read) => {
              *spec = read;
              break "the node's connection was changed".to_owned();
            }
            Err(error) => eprintln!("{error}"),
          }
        }
      }
    }
  };

  if let Ok(mut held) = current.lock() {
    *held = None;
  }

  drop(send);
  let _ = child.start_kill();
  let _ = child.wait().await;
  writer.abort();
  let _ = errors.await;

  let said = last_said.lock().map(|held| held.clone()).unwrap_or_default();

  if said.is_empty() { reason } else { format!("{reason}: {said}") }
}

// What a connection holds beside its mirror, ended with it.
struct Sides(Vec<tokio::task::JoinHandle<()>>);

impl Drop for Sides {
  fn drop(&mut self) {
    for side in &self.0 {
      side.abort();
    }
  }
}

/// The bot's questions on the control socket: a signal for a process on the node, and the link's own
/// state. One JSON line in, one out.
async fn control(listener: UnixListener, current: Current, state_path: PathBuf) {
  loop {
    let Ok((stream, _)) = listener.accept().await else { continue };
    let current = current.clone();
    let state_path = state_path.clone();

    tokio::spawn(async move {
      let (reading, mut writing) = stream.into_split();
      let mut line = String::new();

      if BufReader::new(reading).read_line(&mut line).await.is_err() {
        return;
      }

      let asked: Value = serde_json::from_str(line.trim_end()).unwrap_or(Value::Null);
      let answer = if let Some(signal) = asked.get("signal") {
        let frame = Frame::Signal {
          pid: signal.get("pid").and_then(Value::as_i64).unwrap_or(0) as i32,
          birth: signal.get("birth").and_then(Value::as_str).map(str::to_owned),
          signal: signal.get("signal").and_then(Value::as_i64).unwrap_or(i64::from(libc::SIGTERM)) as i32,
        };
        let sender = current.lock().ok().and_then(|held| held.clone());

        match sender {
          Some(sender) if sender.send(frame).is_ok() => json!({ "ok": true }),
          _ => json!({ "ok": false, "error": "the link to the node is down" }),
        }
      } else {
        fs::read_to_string(&state_path)
          .ok()
          .and_then(|text| serde_json::from_str(&text).ok())
          .unwrap_or(json!({ "state": "unknown" }))
      };

      let _ = writing.write_all(format!("{answer}\n").as_bytes()).await;
    });
  }
}

pub fn bind(path: &Path) -> std::io::Result<UnixListener> {
  if let Some(dir) = path.parent() {
    fs::create_dir_all(dir)?;
  }

  let _ = fs::remove_file(path);
  let listener = UnixListener::bind(path)?;

  fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;

  Ok(listener)
}

pub async fn connect(spec_path: PathBuf) -> Result<(), String> {
  let mut spec = Spec::read(&spec_path)?;
  let current: Current = Arc::new(Mutex::new(None));
  let control_listener =
    bind(&spec.control).map_err(|error| format!("{}: {error}", spec.control.display()))?;
  let bridge_listener =
    bind(&spec.bridge).map_err(|error| format!("{}: {error}", spec.bridge.display()))?;

  tokio::spawn(control(control_listener, current.clone(), spec.state.clone()));

  {
    let spec_path = spec_path.clone();

    tokio::spawn(async move {
      loop {
        let Ok((stream, _)) = bridge_listener.accept().await else { continue };
        let Ok(spec) = Spec::read(&spec_path) else { continue };

        tokio::spawn(async move {
          let deploy = spec.deploy.clone();
          let node = spec.node.clone();

          let _ = bridge::answer(stream, deploy, node, move |path| spec.here(path)).await;
        });
      }
    });
  }

  let mut state = State::default();
  // A test names a shorter first wait.
  let first_wait = std::env::var("NODE_LINK_FIRST_WAIT_MS")
    .ok()
    .and_then(|ms| ms.parse().ok())
    .map_or(FIRST_WAIT, Duration::from_millis);
  let mut wait = first_wait;

  loop {
    state.connected_since = None;
    state.write(&spec);

    let began = std::time::Instant::now();
    let reason = run_once(&spec_path, &mut spec, &current, &mut state).await;

    eprintln!("{}: link dropped: {reason}", spec.node);
    state.connected_since = None;
    state.dropped = Some(reason);
    state.write(&spec);

    // A connection that stood a while was a good one, and the next try starts from the shortest wait.
    wait = if began.elapsed() > LONGEST_WAIT { first_wait } else { (wait * 2).min(LONGEST_WAIT) };
    tokio::time::sleep(wait).await;

    if let Ok(read) = Spec::read(&spec_path) {
      spec = read;
    }
  }
}
