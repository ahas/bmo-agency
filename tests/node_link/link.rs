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
  let binary = crate::support::binary::path("node-link");
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

#[test]
fn a_link_to_a_persons_pc_mirrors_over_the_hubs_connect_program_with_no_ssh() {
  let binary = crate::support::binary::path("node-link");
  let dir = tempfile::tempdir().expect("dir");
  let here = dir.path().join("here");
  let there = dir.path().join("there");
  let place_here = here.join("agent-places").join("w");
  let place_there = there.join("data").join("agent-places").join("w");

  for made in [&place_here, &place_there, &there.join("data").join("run")] {
    fs::create_dir_all(made).expect("made");
  }

  // The far end stands on the PC as its own service, listening at the loopback for the mirror.
  let mirror = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
  let mirror_port = mirror.local_addr().expect("its address").port();
  drop(mirror);
  let _far = Link(
    Command::new(&binary)
      .args(["serve", "--root"])
      .arg(&there)
      .args(["--listen", &format!("127.0.0.1:{mirror_port}")])
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .expect("serve"),
  );

  // The hub's connect program standing in: its agent and port, joined to that port here, and only for a
  // control credential scoped to that agent alone.
  let connect = dir.path().join("jake-hub");
  let hub_env = dir.path().join("hub.env");

  fs::write(&hub_env, "JAKE_CONTROL_TOKEN=t\nJAKE_CONTROL_KEY=k\n").expect("hub settings");
  fs::write(
    &connect,
    "#!/bin/bash\n[ \"$1\" = connect ] || exit 2\n[ \"${JAKE_CONTROL_CREDENTIAL%%.*}\" = \"$2\" ] || exit 3\nexec 3<>/dev/tcp/127.0.0.1/$3\ncat <&3 &\ncat >&3\n",
  )
  .expect("connect");
  fs::set_permissions(&connect, fs::Permissions::from_mode(0o755)).expect("runnable");

  let spec = dir.path().join("link.json");
  let state = dir.path().join("state.json");

  fs::write(
    &spec,
    json!({
      "node": "u7",
      "ssh": [],
      "root": there,
      "binary": binary,
      "herdr": { "local": dir.path().join("h.sock"), "remote": dir.path().join("rh.sock") },
      "ports": [],
      "bridge": dir.path().join("b.sock"),
      "deploy": dir.path(),
      "control": dir.path().join("c.sock"),
      "rewrite": [[here.to_string_lossy(), there.join("data").to_string_lossy()]],
      "push": [{ "local": place_here, "files": ["program.json"], "rewrite": true }],
      "pull": [{ "remote": place_there, "local": place_here, "files": ["running.json"] }],
      "proc": dir.path().join("proc"),
      "state": state,
      "follow": [here],
      "withheld": [],
      "jake": {
        "agent": "u7",
        "connect": [connect, "connect"],
        "hub_env": hub_env,
        "mirror_port": mirror_port,
        "herdr_port": 1,
        "bridge_port": 2,
        "bridge_relay": 0
      }
    })
    .to_string(),
  )
  .expect("spec");

  let _link = Link(
    Command::new(&binary)
      .arg("connect")
      .arg(&spec)
      .env("NODE_LINK_FIRST_WAIT_MS", "50")
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .expect("connect"),
  );

  wait_for("the connection", || read_json(&state).is_some_and(|read| read["state"] == "connected"));

  // Both ways over the hub's connect program: the program there, its running record here.
  fs::write(place_here.join("program.json"), json!({ "command": { "args": ["x"] } }).to_string())
    .expect("program");
  wait_for("the program on the PC", || place_there.join("program.json").exists());
  fs::write(place_there.join("running.json"), json!({ "pids": [] }).to_string()).expect("running");
  wait_for("the running record here", || place_here.join("running.json").exists());
}

/// One call of the CLI's verbs asked of a home bridge at `socket`, as a node asks it, its input ended at once
/// or held open for as long as the call runs: its exit code, or none where the bridge cannot be reached or the
/// call ended with no exit code.
fn ask_bridge(socket: &Path, args: &[&str], input_ends: bool) -> Option<i32> {
  use std::io::{Read, Write};

  let mut stream = std::os::unix::net::UnixStream::connect(socket).ok()?;

  stream.set_read_timeout(Some(Duration::from_secs(10))).expect("a bounded read");
  writeln!(stream, "{}", json!({ "script": "bin/bmo", "args": args })).ok()?;

  if input_ends {
    stream.write_all(&[0, 0, 0, 0, 0]).ok()?;
  }

  loop {
    let mut head = [0u8; 5];

    stream.read_exact(&mut head).ok()?;

    let mut body = vec![0; u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize];

    stream.read_exact(&mut body).ok()?;

    if head[0] == node_link::bridge::EXIT {
      return Some(i32::from_be_bytes(body.try_into().expect("an exit code")));
    }
  }
}

/// The link program for `spec`, its far end never reached, so only its home bridge is at work.
fn bridge_only_link(binary: &Path, spec: &Path, ssh: &Path) -> Link {
  Link(
    Command::new(binary)
      .arg("connect")
      .arg(spec)
      .env("NODE_LINK_SSH", ssh)
      .env("NODE_LINK_FIRST_WAIT_MS", "50")
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .expect("connect"),
  )
}

// The home bridge's verbs held by the link program itself (docs/person-nodes.md, 「Who reaches a node」): a
// spec that comes to name verbs while the link runs holds the next call to them; the program killed in the
// middle of a call leaves no run behind; and started again on the same spec it refuses what it refused.
#[test]
fn the_bridge_s_verbs_hold_from_the_next_call_and_across_the_link_killed_mid_call_and_started_again() {
  let binary = crate::support::binary::path("node-link");
  let dir = tempfile::tempdir().expect("dir");
  let deploy = dir.path().join("deploy");
  let ran = dir.path().join("ran");
  let held = dir.path().join("held.pid");
  let bmo = deploy.join(node_link::bridge::VERBS_BINARY);
  let ssh = dir.path().join("ssh");

  fs::create_dir_all(bmo.parent().expect("bin")).expect("bin");
  // The CLI standing in: each call written down, and a held call standing on its input until it ends.
  fs::write(
    &bmo,
    format!(
      "#!/bin/sh\necho \"$@\" >> {}\nif [ \"$2\" = --hold ]; then echo $$ > {}; exec cat > /dev/null; fi\n",
      ran.display(),
      held.display()
    ),
  )
  .expect("bmo");
  fs::write(&ssh, "#!/bin/sh\nexit 1\n").expect("ssh");

  for runnable in [&bmo, &ssh] {
    fs::set_permissions(runnable, fs::Permissions::from_mode(0o755)).expect("runnable");
  }

  let spec = dir.path().join("link.json");
  let socket = dir.path().join("b.sock");
  let write_spec = |verbs: Option<&[&str]>| {
    let moving = dir.path().join("link.json.new");

    fs::write(
      &moving,
      json!({
        "node": "pc",
        "ssh": ["tester@pc"],
        "root": dir.path().join("there"),
        "binary": binary,
        "herdr": { "local": dir.path().join("h.sock"), "remote": dir.path().join("rh.sock") },
        "ports": [],
        "bridge": socket,
        "deploy": deploy,
        "control": dir.path().join("c.sock"),
        "rewrite": [],
        "push": [],
        "pull": [],
        "proc": dir.path().join("proc"),
        "state": dir.path().join("state.json"),
        "follow": [],
        "withheld": [],
        "bridge_verbs": verbs
      })
      .to_string(),
    )
    .expect("spec");
    fs::rename(&moving, &spec).expect("spec in place");
  };

  // A node of the deployment's own asks for any verb.
  write_spec(None);

  let link = bridge_only_link(&binary, &spec, &ssh);

  wait_for("the bridge", || socket.exists());
  assert_eq!(ask_bridge(&socket, &["node", "work"], true), Some(0));

  // An account's record comes to name the node while the link runs: its next call is held to the verbs.
  write_spec(Some(&["agent-status"]));
  assert_eq!(ask_bridge(&socket, &["node", "work"], true), Some(2));
  assert_eq!(ask_bridge(&socket, &["agent-status"], true), Some(0));

  // The link program killed in the middle of a call: the asker gets no exit code and the run ends with it.
  let asker = {
    let socket = socket.clone();

    std::thread::spawn(move || ask_bridge(&socket, &["agent-status", "--hold"], false))
  };

  wait_for("the held run", || held.exists());

  let run = fs::read_to_string(&held).expect("held pid").trim().to_owned();

  drop(link);
  assert_eq!(asker.join().expect("asker"), None);
  wait_for("the held run's end", || !Path::new("/proc").join(&run).exists());

  // Started again on the same spec, it refuses what it refused and runs what it ran.
  let _link = bridge_only_link(&binary, &spec, &ssh);

  wait_for("the bridge again", || ask_bridge(&socket, &["agent-status"], true).is_some());
  assert_eq!(ask_bridge(&socket, &["node", "work"], true), Some(2));
  assert_eq!(
    fs::read_to_string(&ran).expect("ran"),
    "node work\nagent-status\nagent-status --hold\nagent-status\n",
    "a refused call runs nothing"
  );
}
