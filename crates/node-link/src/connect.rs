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
use crate::frame::{Frame, Home, Refused, decode, encode};
use crate::home::{DIAL_WAIT, Stream};
use crate::rewrite::{NodeEnv, rewrite_json};
use crate::serve::write_whole;
use crate::spec::{Local, Spec};

/// How often the pushed files are looked at, how often the spec is, and the bounds of the wait before
/// the connection is made again.
const PUSH_EVERY: Duration = Duration::from_millis(300);
const SPEC_EVERY: Duration = Duration::from_secs(2);
const FIRST_WAIT: Duration = Duration::from_secs(1);
const LONGEST_WAIT: Duration = Duration::from_secs(60);
/// How often the state file is written while frames keep arriving.
const HEARD_EVERY: Duration = Duration::from_secs(2);
/// How long a connection may go without a word from the node before it is dropped and made again: the far end
/// speaks every few seconds while it stands, so a connection silent this long is a dead one.
const UNHEARD_BOUND: Duration = Duration::from_secs(120);

/// A bound the environment names in milliseconds under `name`, which a test sets short, else the one given.
fn bound_named(name: &str, given: Duration) -> Duration {
  std::env::var(name)
    .ok()
    .and_then(|ms| ms.parse().ok())
    .map_or(given, Duration::from_millis)
}

fn now_ms() -> u128 {
  SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis())
}

/// What the bot reads of the link: whether it is up, since when, when it last heard the node, the
/// node's home, why it last dropped, and on a person's PC each port its far end could not listen on.
#[derive(Default)]
pub struct State {
  connected_since: Option<u128>,
  heard: Option<u128>,
  home: Option<String>,
  dropped: Option<String>,
  refused: Vec<Refused>,
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
      "refused": self.refused,
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

// The far end of a node on this machine, started under the words that run it as the node's own user: the bridge
// socket the near end holds as its bridge, and the socket it joins to its herdr session. Its end is the end of its
// standard input, so no signal need reach it across the user it runs as.
fn local_command(spec: &Spec, local: &Local) -> Command {
  let mut line: Vec<String> = local.run_as.clone();

  line.extend([
    spec.binary.clone(),
    "serve".into(),
    "--root".into(),
    spec.root.clone(),
    "--bridge".into(),
    spec.bridge.display().to_string(),
    "--herdr-socket".into(),
    spec.herdr.remote.clone(),
    "--herdr-unix".into(),
    local.herdr.display().to_string(),
  ]);

  if let Some(session) = &spec.herdr.session {
    line.extend(["--herdr-session".into(), session.clone()]);
  }

  let mut command = Command::new(&line[0]);

  command
    .args(&line[1..])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);

  command
}

// What a link to a node on this machine holds beside its mirror: the herdr socket here joined to the socket the far
// end joins to the node's herdr session.
fn local_sides(spec: &Spec, local: &Local) -> Vec<tokio::task::JoinHandle<()>> {
  let _ = std::fs::remove_file(&spec.herdr.local);

  match UnixListener::bind(&spec.herdr.local) {
    Ok(listener) => {
      let far = local.herdr.clone();

      vec![tokio::spawn(async move {
        while let Ok((mut client, _)) = listener.accept().await {
          let far = far.clone();

          tokio::spawn(async move {
            if let Ok(mut far) = tokio::net::UnixStream::connect(&far).await {
              let _ = tokio::io::copy_bidirectional(&mut client, &mut far).await;
            }
          });
        }
      })]
    }
    Err(error) => {
      eprintln!("{}: {}: {error}", spec.node, spec.herdr.local.display());
      Vec::new()
    }
  }
}

// A connection to a port on a person's PC: the hub's connect program, its words ahead of the agent and the port,
// presenting a credential that reaches that agent alone.
fn jake_command(jake: &crate::spec::Jake, port: u16) -> Result<Command, String> {
  let credential = crate::hub::agent_credential(&jake.hub_env, &jake.agent)?;
  let (program, words) =
    jake.connect.split_first().map_or(("", &[][..]), |(one, rest)| (one.as_str(), rest));
  let mut command = Command::new(program);

  command
    .args(words)
    .args([jake.agent.as_str(), &port.to_string()])
    .env(crate::hub::CREDENTIAL_VARIABLE, credential)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);
  Ok(command)
}

// What a link to a person's PC holds beside its mirror, for as long as the connection stands: the herdr socket
// here joined to the far end's herdr port, and the hub let go of every listen it keeps for the agent
// (docs/person-nodes.md, 「The link over jake」).
fn jake_sides(spec: &Spec, jake: &crate::spec::Jake) -> Vec<tokio::task::JoinHandle<()>> {
  let mut sides = Vec::new();
  let (agent, hub_env, node) = (jake.agent.clone(), jake.hub_env.clone(), spec.node.clone());

  sides.push(tokio::spawn(async move {
    match crate::hub::drop_listens(&hub_env, &agent).await {
      Ok(dropped) => {
        for port in dropped {
          eprintln!("{node}: the hub let go of its listen on port {port}");
        }
      }
      Err(why) => eprintln!("{node}: the hub was not asked to let go of its listens: {why}"),
    }
  }));
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

  sides
}

/// A connection the far end on a person's PC holds dialed home (docs/person-nodes.md, 「The link over jake」): what
/// it reaches here joined first, a port the link carries or the bridge's socket and nothing else, then a new stream
/// opened to the far end's mirror port, its greeting read, the connection named on it, and the two joined.
pub async fn dial_home(
  jake: &crate::spec::Jake,
  id: u64,
  to: Home,
  ports: &[u16],
  bridge: &Path,
) -> Result<(), String> {
  let mut here: Box<dyn Stream> = match to {
    Home::Port(port) if ports.contains(&port) => Box::new(
      tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .map_err(|why| format!("port {port} here: {why}"))?,
    ),
    Home::Port(port) => return Err(format!("port {port} is not one the link carries")),
    Home::Bridge => Box::new(
      tokio::net::UnixStream::connect(bridge)
        .await
        .map_err(|why| format!("{}: {why}", bridge.display()))?,
    ),
  };
  let mut command = jake_command(jake, jake.mirror_port)?;
  let mut child = command.stderr(Stdio::null()).spawn().map_err(|why| {
    let program = command.as_std().get_program().to_string_lossy().into_owned();

    format!("{program}: {why}")
  })?;
  let (Some(mut input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
    return Err("the hub's connect program gave no input or output".into());
  };
  let mut output = BufReader::new(output);
  let mut greeting = String::new();

  match tokio::time::timeout(DIAL_WAIT, output.read_line(&mut greeting)).await {
    Ok(Ok(read)) if read > 0 && matches!(Frame::read(&greeting), Some(Frame::Hello { .. })) => {}
    _ => return Err("the far end did not greet the stream".into()),
  }

  input
    .write_all(Frame::Back { id }.line().as_bytes())
    .await
    .map_err(|why| format!("the stream: {why}"))?;

  let mut stream = tokio::io::join(output, input);
  let _ = tokio::io::copy_bidirectional(&mut stream, &mut here).await;
  let _ = child.start_kill();

  Ok(())
}

// One connection here carried to a port on the PC through the hub's connect program.
async fn carry_over_jake(client: tokio::net::UnixStream, jake: &crate::spec::Jake, port: u16) {
  let mut command = match jake_command(jake, port) {
    Ok(command) => command,
    Err(why) => return eprintln!("{}: port {port} could not be reached: {why}", jake.agent),
  };
  let mut child = match command.stderr(Stdio::null()).spawn() {
    Ok(child) => child,
    Err(error) => {
      let program = command.as_std().get_program().to_string_lossy().into_owned();

      return eprintln!("{}: port {port} could not be reached: {program}: {error}", jake.agent);
    }
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

/// Whether two specs make the same connection, so a change between them needs no new one.
fn same_transport(one: &Spec, other: &Spec) -> bool {
  one.ssh == other.ssh
    && one.jake == other.jake
    && one.local == other.local
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
  /// withheld unless it is one the spec hands all the same, and not one the node itself sends back.
  fn may_follow(spec: &Spec, path: &Path) -> bool {
    spec.follow.iter().any(|root| path.starts_with(root))
      && (spec.handed.iter().any(|handed| path == handed)
        || !spec.withheld.iter().any(|root| path.starts_with(root)))
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
    Frame::Carried { refused } => {
      if refused != state.refused {
        for one in &refused {
          eprintln!("{}: the far end could not listen on port {}: {}", spec.node, one.port, one.why);
        }

        if refused.is_empty() {
          eprintln!("{}: the far end listens on every port it carries", spec.node);
        }
      }

      state.refused = refused;
      state.written = None;
    }
    _ => {}
  }
}

// The ports here a program on a person's PC reaches, named to its far end to carry home; nothing for a node
// reached otherwise, whose connection forwards them itself.
fn carry(spec: &Spec, send: &mpsc::UnboundedSender<Frame>) {
  if spec.jake.is_some() {
    let _ = send.send(Frame::Carry {
      ports: spec.ports.clone(),
    });
  }
}

/// One connection, from its start to its end; answers why it ended.
async fn run_once(spec_path: &Path, spec: &mut Spec, current: &Current, state: &mut State) -> String {
  let remote_bridge = format!("{}/data/run/bridge-{}-{}.sock", spec.root, std::process::id(), now_ms());
  let (command, sides) = match (&spec.jake, &spec.local) {
    (Some(jake), _) => match jake_command(jake, jake.mirror_port) {
      Ok(command) => (command, jake_sides(spec, jake)),
      Err(why) => return format!("the connection could not start: {why}"),
    },
    (None, Some(local)) => (local_command(spec, local), local_sides(spec, local)),
    (None, None) => (ssh_command(spec, &remote_bridge), Vec::new()),
  };
  // The sides stand as long as this connection does.
  let _sides = Sides(sides);
  let mut command = command;
  let mut child = match command.spawn() {
    Ok(child) => child,
    Err(error) => {
      let program = command.as_std().get_program().to_string_lossy().into_owned();

      return format!("the connection could not start: {program}: {error}");
    }
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
  let unheard = bound_named("NODE_LINK_UNHEARD_MS", UNHEARD_BOUND);
  let mut heard_at = tokio::time::Instant::now();

  let reason = loop {
    tokio::select! {
      () = tokio::time::sleep_until(heard_at + unheard) => {
        break format!("nothing was heard from the node in {} seconds", unheard.as_secs_f64());
      }
      line = lines.next_line() => {
        let Ok(Some(line)) = line else { break "the session ended".to_owned() };
        heard_at = tokio::time::Instant::now();
        let Some(frame) = Frame::read(&line) else { continue };
        let now = now_ms();

        // A connection made is said at once; while it stands, the state is written every few
        // seconds of frames.
        let hello = matches!(frame, Frame::Hello { .. });

        if let Frame::Hello { path, .. } = &frame {
          let head = spec.path_head.join(":");

          node_path = Some(if head.is_empty() { path.clone() } else { format!("{head}:{path}") });

          if state.connected_since.is_none() {
            eprintln!("{}: link up", spec.node);
          }

          state.connected_since = Some(now);
          state.dropped = None;
          let _ = send.send(Frame::Near { node: spec.node.clone() });
          let _ = send.send(Frame::Watch { pulls: spec.pulls() });
          let _ = send.send(Frame::Folders { paths: spec.folders.clone() });
          carry(spec, &send);
          pushing = Pushing::new();
        }

        if let (Frame::Dial { id, to }, Some(jake)) = (&frame, &spec.jake) {
          let (id, to, jake, ports, bridge) = (*id, *to, jake.clone(), spec.ports.clone(), spec.bridge.clone());
          let node = spec.node.clone();

          tokio::spawn(async move {
            if let Err(why) = dial_home(&jake, id, to, &ports, &bridge).await {
              eprintln!("{node}: a connection on the node was not dialed home: {why}");
            }
          });
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

              // Before the far end greets, what it is asked waits for the greeting, which asks it whole and after
              // the node's name.
              if node_path.is_some() {
                let _ = send.send(Frame::Watch { pulls: spec.pulls() });
                let _ = send.send(Frame::Folders { paths: spec.folders.clone() });
                carry(spec, &send);
              }
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
  bind_mode(path, 0o600)
}

/// A socket bound at `path` with the mode given, its folder made where it is not.
pub fn bind_mode(path: &Path, mode: u32) -> std::io::Result<UnixListener> {
  if let Some(dir) = path.parent() {
    fs::create_dir_all(dir)?;
  }

  let _ = fs::remove_file(path);
  let listener = UnixListener::bind(path)?;

  fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(mode))?;

  Ok(listener)
}

/// The mode of the bridge socket a node on this machine reaches as its own user: open to every user that reaches
/// its folder, which the folder's own mode leaves to the owner's user and the node's.
const SHARED_SOCKET_MODE: u32 = 0o666;

pub async fn connect(spec_path: PathBuf) -> Result<(), String> {
  let mut spec = Spec::read(&spec_path)?;
  let current: Current = Arc::new(Mutex::new(None));
  let control_listener =
    bind(&spec.control).map_err(|error| format!("{}: {error}", spec.control.display()))?;
  let bridge_mode = if spec.local.is_some() { SHARED_SOCKET_MODE } else { 0o600 };
  let bridge_listener =
    bind_mode(&spec.bridge, bridge_mode).map_err(|error| format!("{}: {error}", spec.bridge.display()))?;

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
          let verbs = spec.bridge_verbs.clone();

          let _ = bridge::answer(stream, deploy, node, verbs, move |path| spec.here(path)).await;
        });
      }
    });
  }

  let mut state = State::default();
  // A test names a shorter first wait.
  let first_wait = bound_named("NODE_LINK_FIRST_WAIT_MS", FIRST_WAIT);
  let mut wait = first_wait;

  loop {
    state.connected_since = None;
    state.write(&spec);

    let began = std::time::Instant::now();
    let reason = run_once(&spec_path, &mut spec, &current, &mut state).await;

    match state.connected_since {
      Some(since) => eprintln!(
        "{}: link down after {} seconds: {reason}",
        spec.node,
        now_ms().saturating_sub(since) / 1000
      ),
      None => eprintln!("{}: link not made: {reason}", spec.node),
    }
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
