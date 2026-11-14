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
  /// On the node: a script of this machine's CLI tree asked of the home bridge the run's `BMO_HOME_BRIDGE`
  /// names, its output, errors and exit code the run's own.
  Home {
    script: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
  },
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
    /// On a person's PC: the loopback address the mirror is spoken on, one session after another.
    #[arg(long)]
    listen: Option<String>,
    /// On a person's PC: the loopback address the herdr socket is reached at.
    #[arg(long)]
    herdr_listen: Option<String>,
    /// On a person's PC: the loopback port the stream back to the home bridge stands on.
    #[arg(long)]
    bridge_port: Option<u16>,
    /// On a node of this machine running as a user of its own: the socket the herdr socket is reached at.
    #[arg(long)]
    herdr_unix: Option<PathBuf>,
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
    Verb::Home { script, args } => {
      let socket = std::env::var_os("BMO_HOME_BRIDGE").filter(|named| !named.is_empty());
      let Some(socket) = socket else {
        eprintln!("node-link home: no home bridge is named: BMO_HOME_BRIDGE is not set");
        return ExitCode::from(2);
      };
      let code = node_link::bridge::ask(
        std::path::Path::new(&socket),
        &script,
        &args,
        tokio::io::stdin(),
        tokio::io::stdout(),
        tokio::io::stderr(),
      )
      .await;

      ExitCode::from(u8::try_from(code).unwrap_or(1))
    }
    Verb::Serve {
      root,
      bridge,
      herdr_session,
      herdr_socket,
      listen,
      herdr_listen,
      bridge_port,
      herdr_unix,
    } => match serve::serve(serve::Serve {
      root,
      bridge,
      herdr_session,
      herdr_socket,
      listen,
      herdr_listen,
      bridge_port,
      herdr_unix,
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
