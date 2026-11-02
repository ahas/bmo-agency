//! LiteLLM on one table: the table written beside the hooks it names, at `0600`, the keys in the
//! child's environment alone, and the child ready once it answers its liveliness path.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use http_body_util::Empty;
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::front::{BoxFuture, Child};
use crate::keys::read_key;
use crate::table::{GATEWAY_KEY_VARIABLE, Table};

/// How often a child coming up is asked whether it answers.
const READINESS_LOOK: Duration = Duration::from_millis(500);

/// Where a launcher keeps its tables and reads its keys, and the hooks it copies beside each table.
pub struct Place {
  /// The data root's `llm-gateway/`, private to the account.
  pub folder: PathBuf,
  /// The data root's `env/`.
  pub env_dir: PathBuf,
  pub hooks: Vec<PathBuf>,
}

impl Place {
  pub fn table_path(&self, digest: &str) -> PathBuf {
    self.folder.join(format!("config-{digest}.yaml"))
  }

  /// Each hook's name and what its file holds now, which the table's digest covers.
  pub fn hook_texts(&self) -> Vec<(String, Vec<u8>)> {
    self
      .hooks
      .iter()
      .map(|path| {
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        (name, std::fs::read(path).unwrap_or_default())
      })
      .collect()
  }

  /// A table left by a launcher that ended without stopping its children is nobody's.
  pub fn clear_leftovers(&self) {
    let Ok(entries) = std::fs::read_dir(&self.folder) else {
      return;
    };
    for entry in entries.flatten() {
      let name = entry.file_name().to_string_lossy().into_owned();
      if name.starts_with("config-") && name.contains(".yaml") {
        let _ = std::fs::remove_file(entry.path());
      }
    }
  }
}

// A file written private and renamed into place, so no reader sees half of it.
pub fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
  let staged = PathBuf::from(format!("{}.{}", path.display(), std::process::id()));
  std::fs::write(&staged, text)?;
  std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o600))?;
  std::fs::rename(&staged, path)
}

struct Litellm {
  port: u16,
  process: Mutex<Option<tokio::process::Child>>,
  table: PathBuf,
}

impl Child for Litellm {
  fn port(&self) -> u16 {
    self.port
  }

  fn stop(&self) -> BoxFuture<'_, ()> {
    Box::pin(async move {
      if let Some(mut process) = self.process.lock().await.take() {
        stop_process(&mut process).await;
      }
      let _ = std::fs::remove_file(&self.table);
    })
  }
}

// A child asked to end, and waited for until it has.
async fn stop_process(process: &mut tokio::process::Child) {
  if let Some(pid) = process.id() {
    unsafe {
      libc::kill(pid as i32, libc::SIGTERM);
    }
  }
  let _ = process.wait().await;
}

fn free_port() -> std::io::Result<u16> {
  Ok(std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

// Whether the child answers its liveliness path with a success: a refused or broken connection is no, and
// an answer is waited for.
async fn lively(port: u16) -> bool {
  let asked = async {
    let stream = TcpStream::connect(("127.0.0.1", port)).await.ok()?;
    let (mut sender, connection) =
      hyper::client::conn::http1::handshake(TokioIo::new(stream)).await.ok()?;
    tokio::spawn(connection);
    let request = hyper::Request::get("/health/liveliness")
      .header(hyper::header::HOST, format!("127.0.0.1:{port}"))
      .body(Empty::<Bytes>::new())
      .ok()?;
    Some(sender.send_request(request).await.ok()?.status().is_success())
  };
  asked.await.unwrap_or(false)
}

/// LiteLLM started on the table and waited for until it answers or ends, however long it takes.
pub async fn start(place: &Place, table: &Table, digest: &str) -> Result<Box<dyn Child>, String> {
  let table_path = place.table_path(digest);
  write_private(&table_path, &table.text)
    .map_err(|fault| format!("the table could not be written: {fault}"))?;
  // LiteLLM loads a callback from a file of its name beside the table.
  for hook in &place.hooks {
    if let Some(name) = hook.file_name() {
      std::fs::copy(hook, place.folder.join(name))
        .map_err(|fault| format!("{} could not be copied: {fault}", hook.display()))?;
    }
  }

  let mut env = BTreeMap::new();
  env.insert("PATH".to_owned(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned()));
  env.insert("HOME".to_owned(), std::env::var("HOME").unwrap_or_default());
  env.insert(
    GATEWAY_KEY_VARIABLE.to_owned(),
    read_key(&place.env_dir, "llm-gateway.key").unwrap_or_default(),
  );
  for (variable, key_file) in &table.key_files {
    let key = read_key(&place.env_dir, key_file);
    if key.is_none() {
      eprintln!(
        "llm-gateway: the key file {key_file} is missing or not 0600; its addresses will be refused"
      );
    }
    env.insert(variable.clone(), key.unwrap_or_default());
  }

  let port = free_port().map_err(|fault| format!("no free port: {fault}"))?;
  let spawned = Command::new("litellm")
    .args([
      "--config",
      &table_path.to_string_lossy(),
      "--host",
      "127.0.0.1",
      "--port",
      &port.to_string(),
    ])
    .args(["--num_workers", "1"])
    .current_dir(&place.folder)
    .env_clear()
    .envs(&env)
    .stdin(Stdio::null())
    .spawn();
  let mut process = match spawned {
    Ok(process) => process,
    Err(fault) => {
      let _ = std::fs::remove_file(&table_path);
      return Err(format!("LiteLLM could not be started: {fault}"));
    }
  };

  loop {
    if let Ok(Some(ended)) = process.try_wait() {
      let _ = std::fs::remove_file(&table_path);
      let code = ended.code().map_or_else(|| "a signal".to_owned(), |code| code.to_string());
      return Err(format!("LiteLLM ended with {code} before it answered"));
    }
    if lively(port).await {
      return Ok(Box::new(Litellm {
        port,
        process: Mutex::new(Some(process)),
        table: table_path,
      }));
    }
    tokio::time::sleep(READINESS_LOOK).await;
  }
}
