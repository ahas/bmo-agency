//! `node-link connect <spec>` holds one node's link from this machine; `node-link serve` is its far end,
//! which the link's SSH session starts on the node (docs/work-nodes.md, 「The link」).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use node_link::{connect, serve};

#[derive(Parser)]
#[command(name = "node-link", about = "The link to a work node")]
struct Cli {
  #[command(subcommand)]
  verb: Verb,
}

#[derive(Subcommand)]
enum Verb {
  /// Hold the link the spec describes, making it again whenever it drops.
  Connect { spec: PathBuf },
  /// The far end, on the node: watch, send, write and signal over standard input and output.
  Serve {
    /// The workplace's root on the node.
    #[arg(long)]
    root: PathBuf,
    /// The socket this session's home bridge was forwarded to.
    #[arg(long)]
    bridge: Option<PathBuf>,
    /// The node's herdr session, by its name and its socket, started where it does not answer.
    #[arg(long)]
    herdr_session: Option<String>,
    #[arg(long)]
    herdr_socket: Option<PathBuf>,
  },
}

#[tokio::main]
async fn main() -> ExitCode {
  match Cli::parse().verb {
    Verb::Connect { spec } => match connect::connect(spec).await {
      Ok(()) => ExitCode::SUCCESS,
      Err(error) => {
        eprintln!("node-link: {error}");
        ExitCode::FAILURE
      }
    },
    Verb::Serve {
      root,
      bridge,
      herdr_session,
      herdr_socket,
    } => match serve::serve(serve::Serve {
      root,
      bridge,
      herdr_session,
      herdr_socket,
    })
    .await
    {
      Ok(()) => ExitCode::SUCCESS,
      Err(error) => {
        eprintln!("node-link serve: {error}");
        ExitCode::FAILURE
      }
    },
  }
}
