//! An account's isolation (docs/account-isolation.md, 「Where it is checked」): a user made by the setup's own first
//! step, and a program run as it in the sandbox the policy describes, is refused a read and a write outside its
//! home, the owner's files, the hub's settings file, sudo and docker, a port of this machine's loopback or of another
//! of its addresses and a sight of another user's process, and is allowed its own home and the tool host's port; while
//! a program of the owner's user still reads and writes them. The user is real and is taken away again with its
//! firewall table, so the test runs where this machine's user holds passwordless sudo, as the deployment's does. A
//! copy into the node whose writing side ends early fails at once, which needs no user.

use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use agency_server::account_isolation::Isolation;
use agency_server::verbs::work_node_isolated::IsolatedDoor;
use agency_server::verbs::work_node_setup::SetupShell;

/// The account's user, taken away with its home and its firewall table however the test ends.
struct MadeUser(String);

impl Drop for MadeUser {
  fn drop(&mut self) {
    remove_user(&self.0);
  }
}

// The user and the firewall table its sandbox stood, which is named after it.
fn remove_user(user: &str) {
  for line in [&["userdel", "--remove", user][..], &["nft", "delete", "table", "inet", user]] {
    let _ = Command::new("sudo")
      .arg("-n")
      .args(line)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status();
  }
}

/// The tool port the link's test hands the sandbox, where no tool host is asked.
const NO_TOOL_HOST: u16 = 1;

fn owner_runs(line: &str) -> bool {
  Command::new("/bin/sh")
    .args(["-c", line])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .is_ok_and(|status| status.success())
}

/// Whether this machine can stand an isolation node (docs/account-isolation.md, 「Where it is checked」); where it
/// cannot, what it lacks is said on a line.
fn can_stand_one_here() -> bool {
  let mut wants = agency_server::account_isolation::machine_wants();

  if !owner_runs("sudo -n true") {
    wants.push("this machine's user holds no passwordless sudo".to_owned());
  }

  if !wants.is_empty() {
    eprintln!("this machine cannot stand an isolation node, so it proves none: {}", wants.join("; "));
  }

  wants.is_empty()
}

fn shown(path: &Path) -> String {
  path.display().to_string()
}

#[tokio::test]
async fn a_copy_whose_writer_ends_at_once_fails_at_once_rather_than_holding_its_reader() {
  // More than a pipe holds, so the reader is still printing once the writer has gone.
  let source = tempfile::NamedTempFile::new().expect("a source");

  std::fs::write(source.path(), vec![b'x'; 4 << 20]).expect("its bytes");

  let mut reading = tokio::process::Command::new("cat");
  let mut writing = tokio::process::Command::new("/bin/sh");

  reading.arg(source.path()).kill_on_drop(true);
  writing.args(["-c", "echo the sandbox ran nothing >&2; exit 3"]).kill_on_drop(true);

  let (copied, said) = tokio::time::timeout(
    std::time::Duration::from_secs(60),
    agency_server::verbs::work_node_isolated::carried(reading, writing),
  )
  .await
  .expect("the copy ends once its writer has");

  assert!(!copied);
  assert!(said.contains("the sandbox ran nothing"), "{said}");
}

#[test]
fn the_sandbox_hides_other_users_processes_and_stands_its_user_s_network_rule_before_its_program() {
  let isolation = Isolation::of_account("7").expect("a login id");
  let words = isolation.sandbox("/usr/bin", 4321);
  let firewall = isolation.firewall(4321);
  let before_program = &words[..words.iter().position(|word| word == "--").expect("the program's place")];
  let stood = format!(
    "--property=ExecStartPre=+{} \"{firewall}\"",
    policy::account_isolation::ACCOUNT_ISOLATION.user_programs.nft
  );

  for property in ["--property=ProtectProc=invisible", "--property=ProcSubset=pid", stood.as_str()] {
    assert!(before_program.iter().any(|word| word == property), "{property} in {words:?}");
  }

  // One word to the system manager, read by it as it stands.
  assert!(!firewall.contains(['"', '\\', '$', '%']), "{firewall}");

  for rule in [
    "delete table inet bmo-account-7; table inet bmo-account-7 {",
    "meta skuid bmo-account-7 jump account",
    "ip daddr 127.0.0.1 tcp dport 4321 accept",
    "meta oif lo goto refuse",
    "192.168.0.0/16",
  ] {
    assert!(firewall.contains(rule), "{rule} in {firewall}");
  }
}

#[tokio::test]
async fn an_isolated_account_reads_and_writes_inside_its_home_alone_while_the_owner_s_user_keeps_the_machine()
 {
  if !can_stand_one_here() {
    return;
  }

  let account = format!("99{}", std::process::id());
  let isolation = Isolation::of_account(&account).expect("a login id");

  remove_user(&isolation.user);

  let _made = MadeUser(isolation.user.clone());
  // The tool host's port, another port of this machine's loopback, and a port on another address of this machine,
  // which no private range names, so only the rule on this machine's own addresses refuses it.
  let tool_host = TcpListener::bind("127.0.0.1:0").expect("a tool host");
  let other = TcpListener::bind("127.0.0.1:0").expect("another port");
  let tool_port = tool_host.local_addr().expect("its port").port();
  let other_port = other.local_addr().expect("its port").port();
  let elsewhere = TcpListener::bind("127.0.0.2:0").expect("a port on another address");
  let elsewhere_port = elsewhere.local_addr().expect("its port").port();

  assert!(IsolatedDoor::open(&isolation, false, tool_port).await.is_err(), "a check makes no user");

  let door = IsolatedDoor::open(&isolation, true, tool_port).await.expect("the user is made");

  assert_eq!(door.wanting().await, Vec::<String>::new(), "the user holds its own group alone");

  // The owner's files: a folder under the owner's home whose own mode lets every user in, with a file anyone may
  // read, so only the isolation keeps the account from them.
  let home = PathBuf::from(std::env::var("HOME").expect("a home"));
  let owners = tempfile::Builder::new()
    .prefix(".isolation-check-")
    .tempdir_in(&home)
    .expect("a folder");
  let secret = owners.path().join("secret");

  std::fs::set_permissions(owners.path(), std::fs::Permissions::from_mode(0o777)).expect("open");
  std::fs::write(&secret, "the owner's\n").expect("a file");
  std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644)).expect("readable");

  let data = home.join(".local/share/bmo/data");
  let homes = PathBuf::from(&policy::account_isolation::ACCOUNT_ISOLATION.home_base);
  let hub_settings = home.join(".local/share/jake/hub.env");
  let written = owners.path().join("written");
  let refused = [
    ("a read of the owner's file", format!("cat {}", shown(&secret))),
    ("a read of the owner's home", format!("ls {}", shown(&home))),
    ("a read of the hub's settings file", format!("cat {}", shown(&hub_settings))),
    ("a read of the data root's tokens", format!("ls {}", shown(&data.join("env")))),
    ("a read of the system's secrets", "cat /etc/shadow".to_owned()),
    ("a write into the owner's folder", format!("touch {}", shown(&written))),
    ("a write into the system", "touch /etc/isolation-check".to_owned()),
    ("a write beside the homes", format!("touch {}", shown(&homes.join("isolation-check")))),
    ("a write into /root", "touch /root/isolation-check".to_owned()),
    ("sudo", "sudo -n true".to_owned()),
    ("docker", "test -w /var/run/docker.sock".to_owned()),
    (
      "a connection to a port of this machine's loopback",
      format!("exec 3<>/dev/tcp/127.0.0.1/{other_port}"),
    ),
    (
      "a connection to another address of this machine",
      format!("exec 3<>/dev/tcp/127.0.0.2/{elsewhere_port}"),
    ),
    ("a sight of another user's process", "ls /proc/1".to_owned()),
  ];

  for (what, line) in &refused {
    let ran = door.run(line).await;

    assert_ne!(ran.code, 0, "{what} is refused the account, yet it ran: {}", ran.stdout);
  }

  assert!(!written.exists(), "nothing the account wrote stands in the owner's folder");
  assert!(!Path::new("/etc/isolation-check").exists());

  // Its own home is its to read and write, its temporary folders are its own scratch folder, and no other home is
  // there to see.
  let inside = door.run("echo inside >\"$HOME/inside\" && cat \"$HOME/inside\"").await;

  assert_eq!((inside.code, inside.stdout.trim()), (0, "inside"), "{}", inside.stderr);

  let reached = door.run(&format!("exec 3<>/dev/tcp/127.0.0.1/{tool_port}")).await;

  assert_eq!(reached.code, 0, "the tool host's port is the account's to reach: {}", reached.stderr);

  let scratch_mark = format!("isolation-check-{account}");
  let scratch = door
    .run(&format!(
      "touch /tmp/{scratch_mark} /var/tmp/{scratch_mark}-v && test -e {}/{scratch_mark} && test -e {}/{scratch_mark}-v",
      shown(&isolation.scratch()),
      shown(&isolation.scratch())
    ))
    .await;

  assert_eq!(scratch.code, 0, "{}", scratch.stderr);
  assert!(
    !Path::new("/tmp").join(&scratch_mark).exists(),
    "this machine's /tmp holds nothing of the account's"
  );

  let listed = door.run(&format!("ls {}", shown(&homes))).await;

  assert_eq!(listed.stdout.split_whitespace().collect::<Vec<_>>(), [isolation.user.as_str()]);

  // The owner's user keeps the machine: its sudo, its files and the tokens the account was refused.
  assert!(owner_runs("sudo -n true"));
  assert!(
    TcpStream::connect(("127.0.0.1", other_port)).is_ok(),
    "the owner's user reaches the port"
  );
  assert!(
    TcpStream::connect(("127.0.0.2", elsewhere_port)).is_ok(),
    "the owner's user reaches the other address"
  );
  assert!(owner_runs(&format!("cat {} && touch {}", shown(&secret), shown(&written))));

  for kept in [&hub_settings, &data.join("env")] {
    if kept.exists() {
      assert!(
        owner_runs(&format!("ls {}", shown(kept))),
        "{} stays the owner's to read",
        kept.display()
      );
    }
  }
}

/// Waits, bounded, until the condition holds.
async fn wait_for(what: &str, mut holds: impl FnMut() -> bool) {
  for _ in 0..400 {
    if holds() {
      return;
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
  }

  panic!("{what} never came");
}

/// The near end of the link, taken down with the far end it started however the test ends.
struct NearEnd(std::process::Child);

impl Drop for NearEnd {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

#[tokio::test]
async fn an_isolation_node_s_link_runs_its_far_end_as_the_account_s_user_in_the_sandbox() {
  use agency_server::verbs::work_node_isolated::IsolatedHands;
  use agency_server::verbs::work_node_setup::SetupHands;
  use serde_json::json;

  if !can_stand_one_here() {
    return;
  }

  let account = format!("98{}", std::process::id());
  let isolation = Isolation::of_account(&account).expect("a login id");

  remove_user(&isolation.user);

  let _made = MadeUser(isolation.user.clone());
  let door = IsolatedDoor::open(&isolation, true, NO_TOOL_HOST).await.expect("the user is made");
  let root = isolation.home.join("node");
  let binary = root.join("bin/node-link");
  let place_there = root.join("data/agent-places/w");

  // The far end's binary copied as the setup copies one: read here, written by the account's user.
  let hands = IsolatedHands {
    install: false,
    copy_code: true,
    door: &door,
  };
  let (copied, said) =
    hands.copy(&shown(&crate::support::binary::path("node-link")), &shown(&binary)).await;

  assert!(copied, "{said}");
  assert_eq!(door.run(&format!("mkdir -p {}", shown(&place_there))).await.code, 0);

  // This machine's side: the place, and the CLI's tree whose verb the bridge runs, saying whose it ran as.
  let here = tempfile::tempdir().expect("here");
  let place_here = here.path().join("data/agent-places/w");
  let deploy = here.path().join("cli");

  std::fs::create_dir_all(&place_here).expect("place");
  std::fs::create_dir_all(deploy.join("bin")).expect("cli");
  std::fs::write(deploy.join("bin/bmo"), "#!/bin/sh\necho \"$1 ran as $(id -un)\"\n").expect("bmo");
  std::fs::set_permissions(deploy.join("bin/bmo"), std::fs::Permissions::from_mode(0o755))
    .expect("runnable");

  let spec = here.path().join("link.json");
  let state = here.path().join("state.json");

  std::fs::write(
    &spec,
    json!({
      "node": isolation.node,
      "ssh": [],
      "root": root,
      "binary": binary,
      "herdr": { "local": here.path().join("h.sock"), "remote": root.join("data/run/herdr.sock") },
      "ports": [],
      "bridge": isolation.bridge_socket(),
      "deploy": deploy,
      "control": here.path().join("c.sock"),
      "rewrite": [[shown(&here.path().join("data")), shown(&root.join("data"))]],
      "push": [{ "local": place_here, "files": ["program.json"], "rewrite": true }],
      "pull": [{ "remote": place_there, "local": place_here, "files": ["running.json"] }],
      "proc": here.path().join("proc"),
      "state": state,
      "follow": [],
      "withheld": [],
      "bridge_verbs": ["agent-status"],
      "local": { "run_as": isolation.sandbox("/usr/local/bin:/usr/bin:/bin", NO_TOOL_HOST), "herdr": isolation.herdr_socket() }
    })
    .to_string(),
  )
  .expect("spec");

  let near = NearEnd(
    Command::new(crate::support::binary::path("node-link"))
      .arg("connect")
      .arg(&spec)
      .env("NODE_LINK_FIRST_WAIT_MS", "50")
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .expect("connect"),
  );
  let read =
    |path: &Path| std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok());

  wait_for("the connection", || {
    read(&state).is_some_and(|state: serde_json::Value| state["state"] == "connected")
  })
  .await;

  // The far end is the account's user's, with no new privileges to gain.
  let far = Command::new("pgrep")
    .args(["-u", &isolation.user, "node-link"])
    .output()
    .expect("pgrep");
  let pid = String::from_utf8_lossy(&far.stdout)
    .split_whitespace()
    .next()
    .expect("the far end")
    .to_owned();
  let status = std::fs::read_to_string(format!("/proc/{pid}/status")).expect("its status");
  let uid = status.lines().find(|line| line.starts_with("Uid:")).expect("its user");
  let own = Command::new("id").args(["-u", &isolation.user]).output().expect("id");

  assert!(
    uid.split_whitespace().nth(1) == Some(String::from_utf8_lossy(&own.stdout).trim()),
    "{uid}"
  );
  assert!(
    status.lines().any(|line| line == "NoNewPrivs:\t1"),
    "the far end runs with no new privileges"
  );

  // The mirror both ways: the program written into the account's home by its own user, its record brought back.
  std::fs::write(place_here.join("program.json"), json!({ "command": { "args": ["x"] } }).to_string())
    .expect("program");
  wait_for("the program on the node", || {
    Command::new("sudo")
      .args(["-n", "test", "-e", &shown(&place_there.join("program.json"))])
      .status()
      .is_ok_and(|status| status.success())
  })
  .await;
  assert_eq!(
    door
      .run(&format!("echo '{{\"pids\": []}}' >{}", shown(&place_there.join("running.json"))))
      .await
      .code,
    0
  );
  wait_for("the running record here", || place_here.join("running.json").exists()).await;

  // A program of the node reaches the home bridge in the shared folder: the policy's verb runs as this machine's
  // user, and any other is refused.
  let ask = |verb: &str| {
    format!(
      "BMO_HOME_BRIDGE={} {} home bin/bmo {verb}",
      shown(&isolation.bridge_socket()),
      shown(&binary)
    )
  };
  let asked = door.run(&ask("agent-status")).await;
  let owner = Command::new("id").arg("-un").output().expect("id");

  assert_eq!(
    (asked.code, asked.stdout.trim()),
    (
      0,
      format!("agent-status ran as {}", String::from_utf8_lossy(&owner.stdout).trim()).as_str()
    ),
    "{}",
    asked.stderr
  );
  assert_eq!(door.run(&ask("node")).await.code, 2, "a verb the policy does not name is refused");

  // The far end ends with its link, so nothing of the account's outlives it.
  drop(near);
  wait_for("the far end gone", || {
    !Command::new("pgrep")
      .args(["-u", &isolation.user])
      .stdout(Stdio::null())
      .status()
      .is_ok_and(|status| status.success())
  })
  .await;
}
