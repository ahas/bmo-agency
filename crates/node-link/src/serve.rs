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
use tokio::sync::mpsc;

use crate::frame::{Frame, Identity, Pull, decode, encode};

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
  /// input and output, one session after another; the loopback address the herdr socket is reached at; and the
  /// loopback port the stream back to the home bridge stands on, which the node's bridge socket is joined to.
  pub listen: Option<String>,
  pub herdr_listen: Option<String>,
  pub bridge_port: Option<u16>,
}

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

  // On a person's PC the bridge socket is the far end's own, each connection joined to the stream back.
  let bridging = match options.bridge_port {
    Some(port) => {
      let socket = options.root.join("data").join("run").join("bridge-jake.sock");
      if let Some(dir) = socket.parent() {
        fs::create_dir_all(dir)?;
      }
      let _ = fs::remove_file(&socket);
      let listener = tokio::net::UnixListener::bind(&socket)?;
      point_bridge(&options.root, &socket);
      Some(tokio::spawn(join_unix_to_tcp(listener, format!("127.0.0.1:{port}"))))
    }
    None => None,
  };
  let herdr = match (&options.herdr_listen, &options.herdr_socket) {
    (Some(address), Some(socket)) => {
      let listener = tokio::net::TcpListener::bind(address.as_str()).await?;
      Some(tokio::spawn(join_tcp_to_unix(listener, socket.clone())))
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
    bridge_port: None,
  };
  let standing = tokio::spawn(async move {
    loop {
      make_standing(&standing_options);
      tokio::time::sleep(STANDING_EVERY).await;
    }
  });

  match &options.listen {
    // One near end at a time, each speaking the mirror over its own connection.
    Some(address) => {
      let listener = tokio::net::TcpListener::bind(address.as_str()).await?;
      loop {
        let (socket, _) = listener.accept().await?;
        let (reader, writer) = socket.into_split();
        session(reader, writer).await;
      }
    }
    None => session(tokio::io::stdin(), tokio::io::stdout()).await,
  }

  standing.abort();
  if let Some(bridging) = bridging {
    bridging.abort();
  }
  if let Some(herdr) = herdr {
    herdr.abort();
  }

  Ok(())
}

// One mirror session over a reader and a writer: the near end's frames done, and what is watched sent.
async fn session(
  reader: impl tokio::io::AsyncRead + Unpin + Send + 'static,
  writer: impl tokio::io::AsyncWrite + Unpin + Send + 'static,
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

  let _ = send.send(Frame::Hello {
    home: std::env::var("HOME").unwrap_or_default(),
    pid: std::process::id(),
    path: std::env::var("PATH").unwrap_or_default(),
  });

  let watching = {
    let send = send.clone();
    let pulls = pulls.clone();

    tokio::spawn(async move { watch(pulls, send).await })
  };

  let mut lines = BufReader::new(reader).lines();

  while let Ok(Some(line)) = lines.next_line().await {
    let Some(frame) = Frame::read(&line) else { continue };

    if let Err(said) = take(frame, &pulls) {
      let _ = send.send(Frame::Trouble { said });
    }
  }

  watching.abort();
  drop(send);
  let _ = writer.await;
}

// Each connection to a unix socket here joined to a fresh TCP connection to `target`.
async fn join_unix_to_tcp(listener: tokio::net::UnixListener, target: String) {
  while let Ok((client, _)) = listener.accept().await {
    let target = target.clone();
    tokio::spawn(async move {
      if let Ok(mut far) = tokio::net::TcpStream::connect(target.as_str()).await {
        let mut client = client;
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
