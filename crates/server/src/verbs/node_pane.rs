//! What a node's workspace in herdr runs (docs/work-nodes.md, 「In herdr」): the node as the node service
//! lists it, kept current, and where the service names an SSH login to it, that login on Enter. It ends
//! with the head's stop once the node is gone, which ends the loop its pane runs it under.

use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Duration;

use policy::fill_marks;
use policy::verb_words::VERB_WORDS;

use super::{Answer, VerbError, service};
use crate::peers::node::{NodeRow, NodeService};
use crate::places::home::HEAD_STOPPED;

// How often the pane draws the node again while nobody is logged in from it.
const REDRAW: Duration = Duration::from_secs(2);

/// What a node's pane shows: the node as the service lists it, and what Enter does there.
pub fn node_pane_text(node: &NodeRow, logs_in: bool) -> String {
  let words = &VERB_WORDS.node_pane;
  let detail = node
    .detail
    .as_deref()
    .map(|detail| fill_marks(&words.detail, &[("detail", detail)]))
    .unwrap_or_default();

  [
    format!("{} ({})", node.label.as_deref().unwrap_or(&node.node), node.node),
    String::new(),
    fill_marks(&words.state, &[("state", &node.state), ("detail", &detail)]),
    String::new(),
    if logs_in { words.ssh_enter.clone() } else { words.no_login.clone() },
  ]
  .join("\n")
}

// The node as it stands now, or none where it left the list.
async fn current(nodes: &dyn NodeService, name: &str) -> Result<Option<NodeRow>, VerbError> {
  let rows = nodes.node_rows().await.map_err(|fault| VerbError::Refused(fault.to_string()))?;

  Ok(rows.into_iter().find(|row| row.node == name))
}

// The login the node service names to the node, or none.
async fn login(nodes: &dyn NodeService, name: &str) -> Option<Vec<String>> {
  nodes.ssh_login(name).await.ok().flatten()
}

fn draw(node: &NodeRow, logs_in: bool) {
  println!("\x1b[2J\x1b[H{}", node_pane_text(node, logs_in));
  let _ = std::io::stdout().flush();
}

/// The pane run until its node leaves the list.
pub async fn node_pane(data_root: &Path, name: &str) -> Result<Answer, VerbError> {
  let service = service(data_root).await?;
  let nodes = service.peers.node.clone();
  let gone = || Answer {
    lines: vec![fill_marks(&VERB_WORDS.node_pane.gone, &[("name", name)])],
    code: u8::try_from(HEAD_STOPPED).unwrap_or(1),
  };

  // Ctrl-C in this pane is for the login it starts: the login stops on it, and this screen stays. A
  // handler rather than an ignore, so what the pane starts still stops on one.
  if let Ok(mut interrupts) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()) {
    tokio::spawn(async move { while interrupts.recv().await.is_some() {} });
  }

  // Enter, a line at a time, read only while no login holds the terminal: the reader waits for its turn
  // after each line, so a login started on one is the only reader of what is typed next.
  let (lines, entered) = tokio::sync::mpsc::unbounded_channel::<()>();
  let mut entered = Some(entered);
  let (turn, turns) = std::sync::mpsc::channel::<()>();

  std::thread::spawn(move || {
    let mut input = std::io::stdin().lock();
    let mut line = String::new();

    loop {
      line.clear();

      if input.read_line(&mut line).unwrap_or(0) == 0 || lines.send(()).is_err() || turns.recv().is_err() {
        return;
      }
    }
  });

  let Some(node) = current(nodes.as_ref(), name).await? else { return Ok(gone()) };

  draw(&node, login(nodes.as_ref(), name).await.is_some());

  let mut redraw = tokio::time::interval(REDRAW);

  loop {
    tokio::select! {
      _ = redraw.tick() => {
        let Some(node) = current(nodes.as_ref(), name).await? else { return Ok(gone()) };

        draw(&node, login(nodes.as_ref(), name).await.is_some());
      }
      typed = async {
        match entered.as_mut() {
          Some(entered) => entered.recv().await,
          None => std::future::pending().await,
        }
      } => {
        if typed.is_none() {
          // The standing input ended; the pane keeps drawing.
          entered = None;
          continue;
        }

        if let Some(args) = login(nodes.as_ref(), name).await {
          print!("\x1b[2J\x1b[H");
          let _ = std::io::stdout().flush();
          let _ = tokio::process::Command::new("ssh").args(args).status().await;

          redraw.reset();
        }

        let Some(node) = current(nodes.as_ref(), name).await? else { return Ok(gone()) };

        draw(&node, login(nodes.as_ref(), name).await.is_some());
        let _ = turn.send(());
      }
    }
  }
}
