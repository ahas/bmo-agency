//! The model gateway the unit `bmo-llm-gateway` runs (docs/own-llm.md, 「The gateway」).
//!
//!   llm-gateway [--port <port>]
//!
//! The port is 18030 unless named; a check stands a launcher of its own on another. The store is the one
//! `BMO_ARCHIVE_URL` names, else the one the data root's `env/archive-url` holds.

use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use llm_gateway::child::{Place, start, write_private};
use llm_gateway::front::{Front, Status, router};
use llm_gateway::keys::{read_key, usable_secret_file};
use llm_gateway::table::{HOOKS, Table, gateway_table, table_digest};
use sqlx::Connection;
use store::open_read;
use store::registry::registrations;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};

fn home() -> PathBuf {
  PathBuf::from(std::env::var_os("HOME").filter(|home| !home.is_empty()).unwrap_or_else(|| "/".into()))
}

fn named(variable: &str) -> Option<String> {
  std::env::var(variable)
    .ok()
    .map(|value| value.trim().to_owned())
    .filter(|value| !value.is_empty())
}

// The tree a deployed binary stands in, `<tree>/bin/llm-gateway`, unless `BMO_CODE_ROOT` names one.
fn code_root() -> PathBuf {
  named("BMO_CODE_ROOT").map(PathBuf::from).unwrap_or_else(|| {
    std::env::current_exe()
      .ok()
      .and_then(|exe| Some(exe.parent()?.parent()?.to_owned()))
      .unwrap_or_else(|| PathBuf::from("."))
  })
}

/// The port named after `--port`, or 18030; anything else named is no port. A port of 0 is one the
/// system chooses, which the launcher names as it starts serving.
fn port_of(args: &[String]) -> Option<u16> {
  match args.iter().position(|arg| arg == "--port") {
    None => Some(18_030),
    Some(at) => args.get(at + 1)?.parse::<u16>().ok(),
  }
}

/// The store's address: the one `BMO_ARCHIVE_URL` names, else the one the data root's
/// `env/archive-url` holds.
fn store_address(data_root: &Path) -> Option<String> {
  named("BMO_ARCHIVE_URL").or_else(|| {
    std::fs::read_to_string(data_root.join("env/archive-url"))
      .ok()
      .map(|held| held.trim().to_owned())
      .filter(|held| !held.is_empty())
  })
}

/// The registry as it stands, read without a write: the store is opened read-only each time, so the
/// gateway never holds a lock the bot waits on. A registry not settled yet — the gateway coming up
/// before the bot — reads as nothing to serve for now.
async fn registry_table(address: &str) -> Option<Table> {
  let read = async {
    let mut store = open_read(address).await?;
    let read = registrations(&mut store).await;
    let _ = store.close().await;
    read
  };
  match read.await {
    Ok(registry) => Some(gateway_table(&registry)),
    Err(fault) => {
      eprintln!("llm-gateway: the registry could not be read: {fault}");
      None
    }
  }
}

// What the console reads of the gateway, written whole each time it changes.
fn write_status(path: &Path, status: &Status) {
  let at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64);
  let mut written = serde_json::to_value(status).expect("the status is plain data");
  written["at"] = at.into();
  let _ = write_private(path, &format!("{written}\n"));
}

async fn refresh(front: &Front, place: &Place, store: &str) {
  let Some(table) = registry_table(store).await else {
    return;
  };
  let digest = table_digest(&table, |key_file| read_key(&place.env_dir, key_file), &place.hook_texts());
  if let Err(fault) = front.take(&digest, start(place, &table, &digest)).await {
    eprintln!("llm-gateway: the table {digest} did not start: {fault}");
  }
}

#[tokio::main]
async fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let Some(port) = port_of(&args) else {
    eprintln!("llm-gateway: --port takes a port number.");
    return ExitCode::FAILURE;
  };
  let data_root = named("BMO_DATA_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| home().join(".local/share/bmo/data"));
  let Some(store) = store_address(&data_root) else {
    eprintln!(
      "llm-gateway: no store is named, neither by BMO_ARCHIVE_URL nor in {}",
      data_root.join("env/archive-url").display()
    );
    return ExitCode::FAILURE;
  };
  let hooks_dir = code_root().join("crates/llm-gateway/hooks");
  let place = Arc::new(Place {
    folder: data_root.join("llm-gateway"),
    env_dir: data_root.join("env"),
    hooks: HOOKS.iter().map(|hook| hooks_dir.join(format!("{hook}.py"))).collect(),
  });
  let refresh_every = Duration::from_millis(
    named("LLM_GATEWAY_REFRESH_MS")
      .and_then(|ms| ms.parse().ok())
      .filter(|ms| *ms > 0)
      .unwrap_or(5_000),
  );

  // Without its own key the gateway would answer anyone on this machine, so it does not stand at all.
  let gateway_key = place.env_dir.join("llm-gateway.key");
  if !usable_secret_file(&gateway_key) {
    eprintln!(
      "llm-gateway: {} is missing or not 0600, so the gateway does not start",
      gateway_key.display()
    );
    return ExitCode::FAILURE;
  }
  if let Err(fault) = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&place.folder) {
    eprintln!("llm-gateway: {} could not be made: {fault}", place.folder.display());
    return ExitCode::FAILURE;
  }
  place.clear_leftovers();

  let status_path = place.folder.join("status.json");
  let front = Front::new({
    let path = status_path.clone();
    move |status| write_status(&path, status)
  });
  let listener = match TcpListener::bind(("127.0.0.1", port)).await {
    Ok(listener) => listener,
    Err(fault) => {
      eprintln!("llm-gateway: 127.0.0.1:{port} could not be listened on: {fault}");
      return ExitCode::FAILURE;
    }
  };
  let port = listener.local_addr().map_or(port, |at| at.port());
  println!("llm-gateway: serving on http://127.0.0.1:{port}");
  write_status(&status_path, &front.status());

  // The registry read again on a beat; a reading still under way — a child starting — lets the beats
  // it spans go by.
  let refreshing = tokio::spawn({
    let (front, place) = (Arc::clone(&front), Arc::clone(&place));
    async move {
      let mut beat = tokio::time::interval(refresh_every);
      beat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
      loop {
        beat.tick().await;
        refresh(&front, &place, &store).await;
      }
    }
  });

  let (Ok(mut terminate), Ok(mut interrupt)) =
    (signal(SignalKind::terminate()), signal(SignalKind::interrupt()))
  else {
    eprintln!("llm-gateway: the stop signals could not be listened for");
    return ExitCode::FAILURE;
  };
  tokio::select! {
    served = axum::serve(listener, router(Arc::clone(&front))) => {
      if let Err(fault) = served {
        eprintln!("llm-gateway: {fault}");
      }
    }
    _ = terminate.recv() => {}
    _ = interrupt.recv() => {}
  }

  refreshing.abort();
  front.close().await;
  let _ = std::fs::remove_file(&status_path);
  ExitCode::SUCCESS
}
