//! The home bridge's near end (docs/work-nodes.md, 「The home bridge」): a script that only this machine
//! can run — it reads the archive or the data root — asked for from a node, run here out of the deploy
//! tree the spec names, the CLI's, its output, errors and exit carried back.
//!
//! One connection is one run. The asker writes one JSON line naming the script, its arguments, the run's
//! own variables and the directory it was asked from, then its standing input as frames of a channel
//! byte and a length — 1 for a piece of input, 0 for its end — since an asker that half-closed its
//! socket could no longer read the answer. The answer is frames the same way: 1 for output, 2 for
//! errors, and 3 for the exit code, which ends it.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::sync::mpsc;

/// A piece of the asker's input; a frame on any other channel, the end frame among them, ends it.
pub const IN: u8 = 1;
pub const OUT: u8 = 1;
pub const ERR: u8 = 2;
pub const EXIT: u8 = 3;

#[derive(Debug, Deserialize)]
pub struct Asked {
  pub script: String,
  #[serde(default)]
  pub args: Vec<String>,
  #[serde(default)]
  pub env: BTreeMap<String, String>,
  #[serde(default)]
  pub cwd: Option<String>,
}

/// The CLI's verbs' binary under the deploy tree, which the bridge runs as it runs a script.
pub const VERBS_BINARY: &str = "bin/bmo";

/// The script asked for, where it is one of the deploy tree's own: a path under its `scripts/` with no
/// step out of it, or the verbs' binary.
pub fn script_in(deploy: &Path, asked: &str) -> Option<PathBuf> {
  if asked == VERBS_BINARY {
    return Some(deploy.join(VERBS_BINARY));
  }

  let path = Path::new(asked);

  if path.is_absolute() || path.components().any(|part| !matches!(part, Component::Normal(_))) {
    return None;
  }

  let mut parts = path.components();

  if parts.next() != Some(Component::Normal("scripts".as_ref())) || !asked.ends_with(".ts") {
    return None;
  }

  Some(deploy.join(path))
}

/// The variables a run hands over that this machine reads as the node meant them: the run's own and
/// its place, every path in them read back as this machine's.
pub fn carried_env(
  asked: &BTreeMap<String, String>,
  from_node: impl Fn(&str) -> String,
) -> BTreeMap<String, String> {
  asked
    .iter()
    .filter(|(name, _)| name.starts_with("BMO_") || name.starts_with("CLAUDE"))
    .filter(|(name, _)| {
      !matches!(
        name.as_str(),
        "BMO_HOME_BRIDGE"
          | "BMO_DATA_DIR"
          | "BMO_SOURCE_DIR"
          | "BMO_DB_PATH"
          | "BMO_ARCHIVE_URL"
          | "CLAUDE_CONFIG_DIR"
      )
    })
    .map(|(name, value)| (name.clone(), from_node(value)))
    .collect()
}

async fn frame(out: &mpsc::UnboundedSender<(u8, Vec<u8>)>, channel: u8, mut from: impl AsyncRead + Unpin) {
  let mut buffer = vec![0; 64 * 1024];

  loop {
    match from.read(&mut buffer).await {
      Ok(0) | Err(_) => return,
      Ok(read) => {
        if out.send((channel, buffer[..read].to_vec())).is_err() {
          return;
        }
      }
    }
  }
}

pub async fn write_frame(
  to: &mut (impl AsyncWrite + Unpin),
  channel: u8,
  bytes: &[u8],
) -> std::io::Result<()> {
  to.write_all(&[channel]).await?;
  to.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
  to.write_all(bytes).await
}

/// One run asked over the bridge, answered on the same connection.
pub async fn answer(
  stream: UnixStream,
  deploy: PathBuf,
  node: String,
  from_node: impl Fn(&str) -> String,
) -> std::io::Result<()> {
  let (reading, mut writing) = stream.into_split();
  let mut reading = BufReader::new(reading);
  let mut header = String::new();

  reading.read_line(&mut header).await?;

  let asked: Asked = match serde_json::from_str(header.trim_end()) {
    Ok(asked) => asked,
    Err(error) => {
      write_frame(&mut writing, ERR, format!("node-link bridge: unreadable request: {error}\n").as_bytes())
        .await?;
      return write_frame(&mut writing, EXIT, &2i32.to_be_bytes()).await;
    }
  };

  let Some(script) = script_in(&deploy, &asked.script) else {
    write_frame(
      &mut writing,
      ERR,
      format!("node-link bridge: {} is no script of the deploy tree\n", asked.script).as_bytes(),
    )
    .await?;
    return write_frame(&mut writing, EXIT, &2i32.to_be_bytes()).await;
  };

  // The verbs' binary runs on its own; a script, on bun.
  let mut command = if asked.script == VERBS_BINARY {
    Command::new(&script)
  } else {
    let mut on_bun = Command::new("bun");

    on_bun.arg(&script);
    on_bun
  };

  command
    // A path of the node among the arguments — the place a hook names — is read as this machine's.
    .args(asked.args.iter().map(|arg| from_node(arg)))
    .current_dir(&deploy)
    .envs(carried_env(&asked.env, &from_node))
    .env("BMO_NODE", &node)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);

  if let Some(cwd) = &asked.cwd {
    command.env("BMO_NODE_CWD", cwd);
  }

  let mut child = match command.spawn() {
    Ok(child) => child,
    Err(error) => {
      write_frame(
        &mut writing,
        ERR,
        format!("node-link bridge: {}: {error}\n", script.display()).as_bytes(),
      )
      .await?;
      return write_frame(&mut writing, EXIT, &127i32.to_be_bytes()).await;
    }
  };

  let (send, mut frames) = mpsc::unbounded_channel();

  if let Some(mut input) = child.stdin.take() {
    tokio::spawn(async move {
      // Each piece of input as it comes, until the asker says it ended or goes away.
      loop {
        let mut head = [0u8; 5];

        if reading.read_exact(&mut head).await.is_err() || head[0] != IN {
          break;
        }

        let mut piece = vec![0; u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize];

        if reading.read_exact(&mut piece).await.is_err() || input.write_all(&piece).await.is_err() {
          break;
        }
      }

      let _ = input.shutdown().await;
    });
  }

  let outputs = [
    child.stdout.take().map(|out| tokio::spawn(frame_of(send.clone(), OUT, out))),
    child.stderr.take().map(|err| tokio::spawn(frame_of(send.clone(), ERR, err))),
  ];

  drop(send);

  while let Some((channel, bytes)) = frames.recv().await {
    if write_frame(&mut writing, channel, &bytes).await.is_err() {
      let _ = child.kill().await;
      return Ok(());
    }
  }

  for output in outputs.into_iter().flatten() {
    let _ = output.await;
  }

  let code = child.wait().await.ok().and_then(|status| status.code()).unwrap_or(1);

  write_frame(&mut writing, EXIT, &code.to_be_bytes()).await
}

async fn frame_of(send: mpsc::UnboundedSender<(u8, Vec<u8>)>, channel: u8, from: impl AsyncRead + Unpin) {
  frame(&send, channel, from).await;
}
