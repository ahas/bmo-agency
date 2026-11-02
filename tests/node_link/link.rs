//! The link run whole on this machine (docs/work-nodes.md, 「The link」): `connect` with an OpenSSH
//! stand-in that runs the far end here, so the mirror is read both ways through the real frames, and a
//! dropped connection is made again.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::Duration;

use serde_json::{Value, json};

/// Waits, bounded, until the condition holds.
fn wait_for(what: &str, mut holds: impl FnMut() -> bool) {
  for _ in 0..200 {
    if holds() {
      return;
    }

    sleep(Duration::from_millis(50));
  }

  panic!("{what} never came");
}

fn read_json(path: &Path) -> Option<Value> {
  serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// This process's start time, as a running record names a process.
fn own_birth() -> String {
  let stat = fs::read_to_string("/proc/self/stat").expect("stat");
  let (_, rest) = stat.rsplit_once(") ").expect("fields");

  rest.split(' ').nth(19).expect("start time").to_owned()
}

struct Link(Child);

impl Drop for Link {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

#[test]
fn the_link_mirrors_both_ways_and_is_made_again_after_its_connection_drops() {
  let binary = crate::support::binary::built("node-link", "node-link");
  let dir = tempfile::tempdir().expect("dir");
  let here = dir.path().join("here");
  let there = dir.path().join("there");
  let place_here = here.join("agent-places").join("w");
  let place_there = there.join("data").join("agent-places").join("w");

  for made in [&place_here, &place_there, &there.join("data").join("run")] {
    fs::create_dir_all(made).expect("made");
  }

  // OpenSSH standing in: the options and the destination passed over, the far end run here, and the
  // process it runs as written down so the test can cut the connection.
  let ssh = dir.path().join("ssh");

  fs::write(
    &ssh,
    "#!/bin/sh\necho $$ > \"$LINK_PID_FILE\"\nwhile [ \"$#\" -gt 0 ] && [ \"$1\" != serve ]; do binary=\"$1\"; shift; done\nshift\nexec \"$binary\" serve \"$@\"\n",
  )
  .expect("ssh");
  fs::set_permissions(&ssh, fs::Permissions::from_mode(0o755)).expect("runnable");

  let spec = dir.path().join("link.json");
  let state = dir.path().join("state.json");

  fs::write(
    &spec,
    json!({
      "node": "t",
      "ssh": ["-o", "BatchMode=yes", "tester@node"],
      "root": there,
      "binary": binary,
      "herdr": { "local": dir.path().join("h.sock"), "remote": dir.path().join("rh.sock") },
      "ports": [],
      "bridge": dir.path().join("b.sock"),
      "deploy": dir.path(),
      "control": dir.path().join("c.sock"),
      "rewrite": [[here.to_string_lossy(), there.join("data").to_string_lossy()]],
      "push": [{ "local": place_here, "files": ["program.json"], "rewrite": true }],
      "pull": [
        { "remote": place_there, "local": place_here, "files": ["running.json"], "alive": true },
        { "remote": there.join("records"), "local": here.join("records"), "tree": true, "append": true }
      ],
      "proc": dir.path().join("proc"),
      "state": state,
      "follow": [here],
      "withheld": [],
      "path_head": ["/node/bin"],
      "env_keep": ["BMO_AGENT_PLACE"],
      "env_add": [["BMO_NODE", "t"]]
    })
    .to_string(),
  )
  .expect("spec");

  let pid_file = dir.path().join("far.pid");
  let _link = Link(
    Command::new(&binary)
      .arg("connect")
      .arg(&spec)
      .env("NODE_LINK_SSH", &ssh)
      .env("NODE_LINK_FIRST_WAIT_MS", "50")
      .env("LINK_PID_FILE", &pid_file)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .expect("connect"),
  );
  let connected = || read_json(&state).is_some_and(|read| read["state"] == "connected");

  wait_for("the connection", connected);

  // Here to there: the program, its paths read as the node's, the bot's token left here.
  let program = json!({ "command": { "args": ["x"], "cwd": "/w", "env": { "BMO_AGENT_PLACE": place_here, "BMO_CONTROL_TOKEN": "secret" } } });

  fs::write(place_here.join("program.json"), program.to_string()).expect("program");
  wait_for("the program on the node", || place_there.join("program.json").exists());

  let pushed = read_json(&place_there.join("program.json")).expect("pushed");

  assert_eq!(pushed["command"]["env"]["BMO_AGENT_PLACE"], json!(place_there));
  assert_eq!(pushed["command"]["env"]["BMO_NODE"], "t");
  assert!(pushed["command"]["env"]["BMO_CONTROL_TOKEN"].is_null());
  assert!(
    pushed["command"]["env"]["PATH"]
      .as_str()
      .is_some_and(|path| path.starts_with("/node/bin:"))
  );

  // There to here: the running record and which of its processes live, a record as it grows, and
  // the node's load and memory.
  let own = std::process::id();

  fs::write(
    place_there.join("running.json"),
    json!({ "pid": own, "birth": own_birth(), "loop": { "pid": 1, "birth": "0" } }).to_string(),
  )
  .expect("running");
  wait_for("the running record here", || place_here.join("running.json").exists());
  wait_for("this process told alive", || {
    read_json(&place_here.join("node-alive.json")).is_some_and(|alive| {
      alive["alive"].as_array().is_some_and(|all| all.iter().any(|one| one["pid"] == own))
    })
  });

  fs::create_dir_all(there.join("records").join("s1")).expect("records");
  fs::write(there.join("records").join("s1").join("a.jsonl"), "one\n").expect("record");
  wait_for("the record here", || {
    fs::read_to_string(here.join("records/s1/a.jsonl")).is_ok_and(|text| text == "one\n")
  });
  fs::OpenOptions::new()
    .append(true)
    .open(there.join("records/s1/a.jsonl"))
    .and_then(|mut file| std::io::Write::write_all(&mut file, b"two\n"))
    .expect("grown");
  wait_for("the grown record here", || {
    fs::read_to_string(here.join("records/s1/a.jsonl")).is_ok_and(|text| text == "one\ntwo\n")
  });
  wait_for("the node's memory", || dir.path().join("proc").join("meminfo").exists());

  // The connection cut: the link says it dropped, makes it again, and carries on where it was.
  let first = fs::read_to_string(&pid_file).expect("pid").trim().to_owned();

  Command::new("kill").args(["-KILL", &first]).status().expect("kill");
  wait_for("a new connection", || {
    fs::read_to_string(&pid_file).is_ok_and(|pid| pid.trim() != first) && connected()
  });

  fs::write(
    place_here.join("program.json"),
    json!({ "command": { "args": ["y"], "cwd": "/w", "env": {} } }).to_string(),
  )
  .expect("program again");
  wait_for("the new program on the node", || {
    read_json(&place_there.join("program.json")).is_some_and(|read| read["command"]["args"][0] == "y")
  });
}
