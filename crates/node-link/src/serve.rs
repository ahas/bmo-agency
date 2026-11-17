//! The far end of a node's link (docs/work-nodes.md, 「The link」): started by the SSH session on the
//! node, it watches what the near end asks for and sends it back as it changes, writes what the near end
//! sends, tells which processes of a place are alive, carries a signal to one, and reads the node's
//! load and memory. It ends with the session.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use crate::frame::{Frame, Identity, Pull, decode, encode};
use crate::home::{Carrier, listen_again, take_bridge};

/// How often what is watched is looked at, how often a place's processes are told without a change,
/// and how often the node's load is read.
const LOOK_EVERY: Duration = Duration::from_millis(500);
const ALIVE_EVERY: Duration = Duration::from_secs(5);
const PROC_EVERY: Duration = Duration::from_secs(5);
/// The most of a file one frame carries.
const CHUNK: u64 = 512 * 1024;
/// The most files one tree is walked for.
const TREE_MOST: usize = 5000;

/// The proc files a machine's standing is read from (docs/agent-control.md, 「Room on the machine」).
const PROC_FILES: [&str; 5] = ["loadavg", "meminfo", "stat", "pressure/memory", "pressure/cpu"];

#[derive(Clone, Copy, PartialEq, Eq)]
struct Seen {
  len: u64,
  modified: i64,
  modified_ns: i64,
  inode: u64,
}

fn seen(meta: &fs::Metadata) -> Seen {
  Seen {
    len: meta.len(),
    modified: meta.mtime(),
    modified_ns: meta.mtime_nsec(),
    inode: meta.ino(),
  }
}

pub struct Serve {
  pub root: PathBuf,
  pub bridge: Option<PathBuf>,
  /// The node's herdr session, by its name and its socket: started where its socket does not answer.
  pub herdr_session: Option<String>,
  pub herdr_socket: Option<PathBuf>,
  /// On a person's PC (docs/person-nodes.md): the loopback address the mirror is spoken on in place of standard
  /// input and output, one session after another, over which the road home is dialed too; and the loopback address
  /// the herdr socket is reached at.
  pub listen: Option<String>,
  pub herdr_listen: Option<String>,
  /// On a node of this machine running as a user of its own (docs/account-isolation.md): the socket the herdr
  /// socket is reached at by the owner's user, in the folder the two share.
  pub herdr_unix: Option<PathBuf>,
}

/// The mode of the socket the owner's user reaches a node's herdr session at, on a node of this machine running as a
/// user of its own: open to every user that reaches its folder, which the folder's own mode leaves to the two.
const SHARED_SOCKET_MODE: u32 = 0o666;

/// How often what must stand on the node is made sure of.
const STANDING_EVERY: Duration = Duration::from_secs(30);

/// A program started apart from this session, so it outlives it: in a session of its own, its output
/// into a log under the data root.
fn start_apart(program: &Path, args: &[&str], log: &Path) {
  let Ok(out) = fs::OpenOptions::new().create(true).append(true).open(log) else {
    return;
  };
  let Ok(err) = out.try_clone() else { return };
  let mut command = std::process::Command::new(program);

  command.args(args).stdin(std::process::Stdio::null()).stdout(out).stderr(err);

  // SAFETY: setsid touches nothing of the parent's memory and is async-signal-safe.
  unsafe {
    std::os::unix::process::CommandExt::pre_exec(&mut command, || {
      libc::setsid();
      Ok(())
    });
  }

  let _ = command.spawn();
}

#[derive(Deserialize)]
struct Claimed {
  pid: i32,
  birth: String,
}

/// What must stand on the node whether or not a link holds it: the agent host of its data root, which
/// the heads there ask, and its herdr session. Each is started only where it is not standing, and a
/// second agent host stands down by itself where one already holds the root.
fn make_standing(options: &Serve) {
  let data = options.root.join("data");
  let claim = data.join("executions").join("agent-host").join("claim");
  let held = fs::read_to_string(&claim)
    .ok()
    .and_then(|text| serde_json::from_str::<Claimed>(&text).ok());

  if !held.is_some_and(|one| process_birth(one.pid).as_deref() == Some(one.birth.as_str())) {
    let host = options.root.join("deploy").join("bin").join("agent-host");
    let data_arg = data.to_string_lossy().into_owned();
    let code_arg = options.root.join("deploy").to_string_lossy().into_owned();

    start_apart(
      &host,
      &["serve", "--data-root", &data_arg, "--code-root", &code_arg],
      &data.join("agent-host.log"),
    );
  }

  if let (Some(session), Some(socket)) = (&options.herdr_session, &options.herdr_socket)
    && std::os::unix::net::UnixStream::connect(socket).is_err()
  {
    start_apart(Path::new("herdr"), &["--session", session, "server"], &data.join("herdr-server.log"));
  }
}

/// Where the home bridge stands on the node: one name every program is handed, pointing at the socket
/// this session's forward made, which carries the session's own name so no earlier session's leftover
/// stands in its way.
fn point_bridge(root: &Path, bridge: &Path) {
  let link = root.join("data").join("bridge.sock");
  let _ = fs::remove_file(&link);
  let _ = symlink(bridge, &link);

  if let (Some(dir), Some(own)) = (bridge.parent(), bridge.file_name())
    && let Ok(entries) = fs::read_dir(dir)
  {
    for entry in entries.flatten() {
      let name = entry.file_name();

      if name != own && name.to_string_lossy().starts_with("bridge-") {
        let _ = fs::remove_file(entry.path());
      }
    }
  }
}

pub async fn serve(options: Serve) -> std::io::Result<()> {
  if let Some(bridge) = &options.bridge {
    point_bridge(&options.root, bridge);
  }

  // On a person's PC the far end carries home what the programs reach there: the ports the near end names, at the
  // loopback address the mirror is spoken on, and the node's bridge socket, which is its own.
  let carrier = options
    .listen
    .as_deref()
    .map(|address| Carrier::new(address.rsplit_once(':').map_or(address, |(host, _)| host)));
  let bridging = match &carrier {
    Some(carrier) => {
      let socket = options.root.join("data").join("run").join("bridge-jake.sock");
      if let Some(dir) = socket.parent() {
        fs::create_dir_all(dir)?;
      }
      let _ = fs::remove_file(&socket);
      let listener = tokio::net::UnixListener::bind(&socket)?;
      point_bridge(&options.root, &socket);
      let again = tokio::spawn(listen_again(carrier.clone()));
      Some((tokio::spawn(take_bridge(listener, carrier.clone())), again))
    }
    None => None,
  };
  let herdr = match (&options.herdr_listen, &options.herdr_unix, &options.herdr_socket) {
    (Some(address), _, Some(socket)) => {
      let listener = tokio::net::TcpListener::bind(address.as_str()).await?;
      Some(tokio::spawn(join_tcp_to_unix(listener, socket.clone())))
    }
    (None, Some(shared), Some(socket)) => {
      let listener = crate::connect::bind_mode(shared, SHARED_SOCKET_MODE)?;
      Some(tokio::spawn(join_unix_to_unix(listener, socket.clone())))
    }
    _ => None,
  };

  let standing_options = Serve {
    root: options.root.clone(),
    bridge: options.bridge.clone(),
    herdr_session: options.herdr_session.clone(),
    herdr_socket: options.herdr_socket.clone(),
    listen: None,
    herdr_listen: None,
    herdr_unix: None,
  };
  let standing = tokio::spawn(async move {
    loop {
      make_standing(&standing_options);
      tokio::time::sleep(STANDING_EVERY).await;
    }
  });

  match (&options.listen, &carrier) {
    // Each connection a mirror session, the newest near end's alone, or a stream dialing a held connection home.
    (Some(address), Some(carrier)) => {
      let listener = tokio::net::TcpListener::bind(address.as_str()).await?;
      let mirror = Arc::new(Mirror::default());
      loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(connection(socket, carrier.clone(), mirror.clone()));
      }
    }
    _ => {
      // Over standard input and output one near end stands for the whole of the far end's life.
      let (_standing, superseded) = oneshot::channel();

      session(
        BufReader::new(tokio::io::stdin()),
        tokio::io::stdout(),
        Greeting::Owed,
        None,
        superseded,
      )
      .await
    }
  }

  standing.abort();
  if let Some((bridging, again)) = bridging {
    bridging.abort();
    again.abort();
  }
  if let Some(herdr) = herdr {
    herdr.abort();
  }

  Ok(())
}

// The far end's greeting: who it is, its home and its search path.
fn hello() -> Frame {
  Frame::Hello {
    home: std::env::var("HOME").unwrap_or_default(),
    pid: std::process::id(),
    path: std::env::var("PATH").unwrap_or_default(),
  }
}

// One connection on a person's PC, greeted, then told apart by the near end's first words: a stream dialing a held
// connection home, joined to it, or a mirror session, which ends the one before it and takes its turn once it has.
async fn connection(socket: tokio::net::TcpStream, carrier: Arc<Carrier>, mirror: Arc<Mirror>) {
  let (reader, mut writer) = socket.into_split();

  if writer.write_all(hello().line().as_bytes()).await.is_err() {
    return;
  }

  let mut reader = BufReader::new(reader);
  let mut first = String::new();

  if !matches!(reader.read_line(&mut first).await, Ok(read) if read > 0) {
    return;
  }

  match Frame::read(&first) {
    Some(Frame::Back { id }) => {
      if let Some(mut held) = carrier.take(id) {
        let mut stream = tokio::io::join(reader, writer);
        let _ = tokio::io::copy_bidirectional(&mut stream, &mut held).await;
      }
    }
    first => {
      let superseded = mirror.newest();
      let _turn = mirror.turn.lock().await;

      session(reader, writer, Greeting::Said(first), Some(carrier), superseded).await;
    }
  }
}

/// The mirror sessions of one far end (docs/person-nodes.md, 「The link over jake」): one at a time, the newest near
/// end's. A near end that went away may leave its stream standing here with nothing to close it, so the session
/// behind it is ended by the next connection rather than waited for.
#[derive(Default)]
pub struct Mirror {
  /// Held by the session speaking now.
  turn: tokio::sync::Mutex<()>,
  /// What ends the newest session, once a newer one comes.
  newest: Mutex<Option<oneshot::Sender<()>>>,
}

impl Mirror {
  /// A new session made the newest: the one before it told to end, and what tells this one, in turn.
  pub fn newest(&self) -> oneshot::Receiver<()> {
    let (ends, superseded) = oneshot::channel();
    let older = self.newest.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).replace(ends);

    if let Some(older) = older {
      let _ = older.send(());
    }

    superseded
  }
}

// Whether a session still owes its greeting, or said it already and read the near end's first frame.
enum Greeting {
  Owed,
  Said(Option<Frame>),
}

// One mirror session over a reader and a writer: the near end's frames done, and what is watched sent. On a person's
// PC the session dials home what the carrier holds for as long as it stands.
async fn session(
  reader: BufReader<impl tokio::io::AsyncRead + Unpin + Send + 'static>,
  writer: impl tokio::io::AsyncWrite + Unpin + Send + 'static,
  greeting: Greeting,
  carrier: Option<Arc<Carrier>>,
  mut superseded: oneshot::Receiver<()>,
) {
  let (send, mut sending) = mpsc::unbounded_channel::<Frame>();
  let pulls: Arc<Mutex<Vec<Pull>>> = Arc::new(Mutex::new(Vec::new()));

  let writer = tokio::spawn(async move {
    let mut out = writer;

    while let Some(frame) = sending.recv().await {
      if out.write_all(frame.line().as_bytes()).await.is_err() || out.flush().await.is_err() {
        break;
      }
    }
  });

  let first = match greeting {
    Greeting::Owed => {
      let _ = send.send(hello());
      None
    }
    Greeting::Said(first) => first,
  };

  if let Some(carrier) = &carrier {
    carrier.attach(send.clone());
  }

  let watching = {
    let send = send.clone();
    let pulls = pulls.clone();

    tokio::spawn(async move { watch(pulls, send).await })
  };

  let mut lines = reader.lines();
  let mut first = first;
  let mut overtaken = false;

  loop {
    let frame = match first.take() {
      Some(frame) => frame,
      None => tokio::select! {
        line = lines.next_line() => match line {
          Ok(Some(line)) => match Frame::read(&line) {
            Some(frame) => frame,
            None => continue,
          },
          _ => break,
        },
        _ = &mut superseded => {
          overtaken = true;
          break;
        }
      },
    };

    if let Frame::Carry { ports } = frame {
      if let Some(carrier) = &carrier {
        carrier.carry(Some(&ports), true).await;
      }
      continue;
    }

    if let Err(said) = take(frame, &pulls) {
      let _ = send.send(Frame::Trouble { said });
    }
  }

  if let Some(carrier) = &carrier {
    carrier.detach(&send);
  }

  watching.abort();
  drop(send);

  // A session a newer near end overtook may be writing into a stream nobody reads, so its writer is ended rather
  // than waited on.
  if overtaken {
    writer.abort();
  }

  let _ = writer.await;
}

// Each connection to a unix socket here joined to a fresh connection to the unix socket `socket`.
async fn join_unix_to_unix(listener: tokio::net::UnixListener, socket: PathBuf) {
  while let Ok((mut client, _)) = listener.accept().await {
    let socket = socket.clone();
    tokio::spawn(async move {
      if let Ok(mut far) = tokio::net::UnixStream::connect(&socket).await {
        let _ = tokio::io::copy_bidirectional(&mut client, &mut far).await;
      }
    });
  }
}

// Each TCP connection here joined to a fresh connection to the unix socket `socket`.
async fn join_tcp_to_unix(listener: tokio::net::TcpListener, socket: PathBuf) {
  while let Ok((client, _)) = listener.accept().await {
    let socket = socket.clone();
    tokio::spawn(async move {
      if let Ok(mut far) = tokio::net::UnixStream::connect(&socket).await {
        let mut client = client;
        let _ = tokio::io::copy_bidirectional(&mut client, &mut far).await;
      }
    });
  }
}

// What the near end sent, done.
/// The folders the programs placed on the node work in, each made where it is not, readable by the account alone.
pub fn make_folders(paths: &[String]) -> Result<(), String> {
  for path in paths {
    std::fs::DirBuilder::new()
      .recursive(true)
      .mode(0o700)
      .create(path)
      .map_err(|error| format!("{path}: {error}"))?;
  }

  Ok(())
}

fn take(frame: Frame, pulls: &Mutex<Vec<Pull>>) -> Result<(), String> {
  match frame {
    Frame::Watch { pulls: asked } => {
      *pulls.lock().map_err(|_| "the watch list is poisoned".to_owned())? = asked;
      Ok(())
    }
    Frame::Put {
      path, data, mode, ..
    } => {
      let bytes = decode(&data).ok_or_else(|| format!("{path}: not base64"))?;

      write_whole(Path::new(&path), &bytes, mode.unwrap_or(0o600))
        .map_err(|error| format!("{path}: {error}"))
    }
    Frame::Folders { paths } => make_folders(&paths),
    Frame::Gone { path, .. } => match fs::remove_file(&path) {
      Ok(()) => Ok(()),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
      Err(error) => Err(format!("{path}: {error}")),
    },
    Frame::Signal { pid, birth, signal } => {
      if pid <= 1
        || birth
          .as_ref()
          .is_some_and(|birth| process_birth(pid).as_deref() != Some(birth.as_str()))
      {
        return Ok(());
      }

      // SAFETY: kill takes a plain pid and signal number and touches no memory of ours.
      unsafe { libc::kill(pid, signal) };
      Ok(())
    }
    _ => Ok(()),
  }
}

/// A file written whole and put in place in one step, its directory made private where it is new.
pub fn write_whole(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
  if let Some(dir) = path.parent() {
    fs::create_dir_all(dir)?;
  }

  let temporary = path.with_extension(format!("link-{}", std::process::id()));

  fs::write(&temporary, bytes)?;
  fs::set_permissions(&temporary, fs::Permissions::from_mode(mode))?;
  fs::rename(&temporary, path)
}

/// A process's start time as the kernel counts it, field 22 of its stat line.
pub fn process_birth(pid: i32) -> Option<String> {
  let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
  let (_, rest) = stat.rsplit_once(") ")?;
  let fields: Vec<&str> = rest.split(' ').collect();

  if fields.first().is_some_and(|state| state.starts_with('Z')) {
    return None;
  }

  fields.get(19).map(|birth| (*birth).to_owned())
}

#[derive(Deserialize)]
struct Recorded {
  pid: i32,
  birth: Option<String>,
}

#[derive(Deserialize)]
struct Running {
  pid: i32,
  birth: Option<String>,
  #[serde(rename = "loop")]
  head: Option<Recorded>,
}

/// The processes a running record names that are alive: the program and the head that became it.
pub fn alive_in(dir: &Path) -> Vec<Identity> {
  let Ok(text) = fs::read_to_string(dir.join("running.json")) else { return Vec::new() };
  let Ok(running) = serde_json::from_str::<Running>(&text) else { return Vec::new() };
  let named = [
    Some(Recorded {
      pid: running.pid,
      birth: running.birth,
    }),
    running.head,
  ];

  named
    .into_iter()
    .flatten()
    .filter_map(|one| {
      let birth = one.birth?;

      (one.pid > 1 && process_birth(one.pid).as_deref() == Some(birth.as_str())).then_some(Identity {
        pid: one.pid,
        birth,
      })
    })
    .collect()
}

/// The files one pull covers, by their path under it.
pub fn files_of(pull: &Pull) -> Vec<String> {
  let root = Path::new(&pull.remote);

  if !pull.files.is_empty() {
    return pull.files.clone();
  }

  let mut found = Vec::new();
  let mut dirs = vec![PathBuf::new()];

  while let Some(dir) = dirs.pop() {
    let Ok(entries) = fs::read_dir(root.join(&dir)) else { continue };

    for entry in entries.flatten() {
      let Ok(kind) = entry.file_type() else { continue };
      let relative = dir.join(entry.file_name());

      if kind.is_dir() && pull.tree {
        dirs.push(relative);
      } else if kind.is_file() {
        found.push(relative.to_string_lossy().into_owned());
      }

      if found.len() >= TREE_MOST {
        return found;
      }
    }
  }

  found
}

fn read_range(path: &Path, from: u64, to: u64) -> std::io::Result<Vec<u8>> {
  let mut file = fs::File::open(path)?;
  let mut bytes = vec![0; (to - from) as usize];

  file.seek(SeekFrom::Start(from))?;
  file.read_exact(&mut bytes)?;

  Ok(bytes)
}

async fn watch(pulls: Arc<Mutex<Vec<Pull>>>, send: mpsc::UnboundedSender<Frame>) {
  // What was last sent of each file, by its pull and its path under it, and of each place's processes.
  let mut sent: HashMap<(usize, String), Seen> = HashMap::new();
  let mut told: HashMap<usize, (Vec<Identity>, Instant)> = HashMap::new();
  let mut watched: Vec<Pull> = Vec::new();
  let mut proc_at: Option<Instant> = None;

  loop {
    let now_watching = pulls.lock().map(|held| held.clone()).unwrap_or_default();

    // A list asked anew is a list sent anew from nothing, since its numbering may have moved.
    if now_watching != watched {
      sent.clear();
      told.clear();
      watched = now_watching;
    }

    for (index, pull) in watched.iter().enumerate() {
      let mut here: Vec<String> = Vec::new();

      for relative in files_of(pull) {
        let path = Path::new(&pull.remote).join(&relative);
        let Ok(meta) = fs::metadata(&path) else { continue };

        if !meta.is_file() {
          continue;
        }

        here.push(relative.clone());

        let now = seen(&meta);
        let before = sent.get(&(index, relative.clone())).copied();

        if before == Some(now) {
          continue;
        }

        let from = match before {
          Some(was) if pull.append && was.inode == now.inode && was.len <= now.len => was.len,
          _ => 0,
        };

        if pull.append {
          let mut offset = from;

          while offset < now.len {
            let to = (offset + CHUNK).min(now.len);
            let Ok(bytes) = read_range(&path, offset, to) else { break };

            let _ = send.send(Frame::Append {
              pull: index,
              path: relative.clone(),
              offset,
              data: encode(&bytes),
            });
            offset = to;
          }

          if offset == 0 && now.len == 0 {
            let _ = send.send(Frame::Append {
              pull: index,
              path: relative.clone(),
              offset: 0,
              data: String::new(),
            });
          }
        } else if let Ok(bytes) = fs::read(&path) {
          let _ = send.send(Frame::Put {
            pull: Some(index),
            path: relative.clone(),
            data: encode(&bytes),
            mode: Some(meta.mode() & 0o777),
          });
        }

        sent.insert((index, relative), now);
      }

      let gone: Vec<String> = sent
        .keys()
        .filter(|(pulled, relative)| *pulled == index && !here.contains(relative))
        .map(|(_, relative)| relative.clone())
        .collect();

      for relative in gone {
        sent.remove(&(index, relative.clone()));
        let _ = send.send(Frame::Gone {
          pull: Some(index),
          path: relative,
        });
      }

      if pull.alive {
        let alive = alive_in(Path::new(&pull.remote));
        let due = told.get(&index).is_none_or(|(was, at)| *was != alive || at.elapsed() >= ALIVE_EVERY);

        if due {
          let _ = send.send(Frame::Alive {
            pull: index,
            alive: alive.clone(),
          });
          told.insert(index, (alive, Instant::now()));
        }
      }
    }

    if proc_at.is_none_or(|at| at.elapsed() >= PROC_EVERY) {
      let files = PROC_FILES
        .iter()
        .filter_map(|name| {
          fs::read_to_string(Path::new("/proc").join(name))
            .ok()
            .map(|text| ((*name).to_owned(), text))
        })
        .collect();

      let _ = send.send(Frame::Proc { files });
      proc_at = Some(Instant::now());
    }

    if send.is_closed() {
      return;
    }

    tokio::time::sleep(LOOK_EVERY).await;
  }
}
