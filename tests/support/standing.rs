//! Agency's own server as a test stands it: started on the store and a data root of the test's own, under a
//! home of the test's own so it reaches no herdr of the machine's, its control port and its quota source
//! each on a port the machine picks; and its control port once it listens. It is stopped when the guard
//! goes.

#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::binary;

/// A server standing, and the port its control routes listen on.
pub struct Standing {
  child: Child,
  pub port: u16,
}

impl Drop for Standing {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

/// The server started under `home` on the data root `data` and the store `store`, under `token`.
pub fn stand_server(home: &Path, data: &Path, store: &str, token: &str) -> Standing {
  let mut child = Command::new(binary::built("agency-server", "agency-server"))
    .arg("--data-root")
    .arg(data)
    .current_dir(home)
    .env("HOME", home)
    .env("XDG_CONFIG_HOME", home.join(".config"))
    .env("BMO_DEPLOY_DIR", env!("CARGO_MANIFEST_DIR"))
    .env_remove("CLAUDE_CONFIG_DIR")
    .env("BMO_ARCHIVE_URL", store)
    .env("AGENCY_CONTROL_PORT", "0")
    .env("AGENCY_QUOTA_PORT", "0")
    .env("AGENCY_CONTROL_TOKEN", token)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()
    .unwrap();
  let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
  let port = loop {
    let line = lines.next().expect("the server said where its control routes are").unwrap();

    if let Some(address) = line.strip_prefix("agency: control routes on http://") {
      break address.rsplit(':').next().unwrap().trim().parse().unwrap();
    }
  };

  std::thread::spawn(move || for _ in lines {});

  Standing { child, port }
}
