//! What the two ends of a node's link say to each other over the SSH session's standing input and
//! output (docs/work-nodes.md, 「The link」): one JSON object per line, file bytes in base64.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

/// One file or tree the far end watches and sends back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pull {
  /// The path on the node.
  pub remote: String,
  /// Only these names under it, where the pull is a directory of named files; none for every file.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub files: Vec<String>,
  /// Every file under it, at any depth, rather than the files directly in it.
  #[serde(default, skip_serializing_if = "is_false")]
  pub tree: bool,
  /// Files that only grow, sent as what they grew by.
  #[serde(default, skip_serializing_if = "is_false")]
  pub append: bool,
  /// The place's running record, whose processes are told alive or not.
  #[serde(default, skip_serializing_if = "is_false")]
  pub alive: bool,
}

// A switch left off is left out, as a spec's writer leaves it out.
fn is_false(on: &bool) -> bool {
  !on
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "kebab-case")]
pub enum Frame {
  /// The far end, up: the node's home, its own process and the search path its login carries.
  Hello { home: String, pid: u32, path: String },
  /// What the far end is to watch, replacing whatever it watched before.
  Watch { pulls: Vec<Pull> },
  /// A file written whole, at its path on the receiving end's side of the pull or push it belongs to.
  Put {
    pull: Option<usize>,
    path: String,
    data: String,
    mode: Option<u32>,
  },
  /// Bytes a growing file grew by, from the offset they start at.
  Append {
    pull: usize,
    path: String,
    offset: u64,
    data: String,
  },
  /// A file that is no longer there.
  Gone { pull: Option<usize>, path: String },
  /// Which processes a place's running record names are alive, by the pull that watches it.
  Alive { pull: usize, alive: Vec<Identity> },
  /// The node's load, memory and pressure files, by their names under the proc root.
  Proc { files: Vec<(String, String)> },
  /// A signal for a process on the node, sent only where it is still the process recorded.
  Signal {
    pid: i32,
    birth: Option<String>,
    signal: i32,
  },
  /// The far end's answer to anything it could not do, said once.
  Trouble { said: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
  pub pid: i32,
  pub birth: String,
}

pub fn encode(bytes: &[u8]) -> String {
  STANDARD.encode(bytes)
}

pub fn decode(text: &str) -> Option<Vec<u8>> {
  STANDARD.decode(text).ok()
}

impl Frame {
  pub fn line(&self) -> String {
    let mut line = serde_json::to_string(self).unwrap_or_default();

    line.push('\n');
    line
  }

  pub fn read(line: &str) -> Option<Frame> {
    serde_json::from_str(line.trim_end()).ok()
  }
}
