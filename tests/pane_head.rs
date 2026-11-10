//! `pane-head` as a pane's loop starts it, against the agent host's answers on a scratch data root
//! (docs/agent-control.md, 「One standing pane per agent」). The answering side runs in this test's
//! process, through the same handler and socket the host serves; the program is a stand-in shell.

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_host::ask::{Answer, Asks, Request, ask, listen};
use agent_host::identity::ProcessIdentity;
use agent_host::layout::Layout;
use serde_json::{Value, json};
use tempfile::TempDir;

const SH: &str = "/bin/sh";

// The head's binary, built once for this test binary.
fn head_binary() -> &'static str {
  static HEAD: OnceLock<String> = OnceLock::new();

  HEAD.get_or_init(|| crate::support::binary::built("pane-head", "pane-head").to_str().unwrap().to_owned())
}

fn now_ms() -> u64 {
  SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

fn within(ms: u64, mut holds: impl FnMut() -> bool) -> bool {
  let until = Instant::now() + Duration::from_millis(ms);

  while Instant::now() < until {
    if holds() {
      return true;
    }

    thread::sleep(Duration::from_millis(20));
  }

  holds()
}

fn alive(pid: i32) -> bool {
  ProcessIdentity::of(pid).is_some_and(|one| one.alive())
}

fn signal(pid: i32, signal: i32) {
  // SAFETY: a signal to a process this test started.
  unsafe {
    libc::kill(pid, signal);
  }
}

// A process that stands until it is ended, to hold a place.
struct Holder(Child);

impl Holder {
  fn new() -> Self {
    Self(Command::new("sleep").arg("60").spawn().unwrap())
  }

  fn identity(&self) -> Value {
    serde_json::to_value(ProcessIdentity::of(self.0.id() as i32).unwrap()).unwrap()
  }
}

impl Drop for Holder {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

// A process the test ends whatever happens.
struct Reaped(Child);

impl Reaped {
  fn pid(&self) -> i32 {
    self.0.id() as i32
  }

  fn ended(&mut self, ms: u64) -> Option<std::process::ExitStatus> {
    let mut status = None;

    within(ms, || {
      status = self.0.try_wait().ok().flatten();
      status.is_some()
    });

    status
  }
}

impl Drop for Reaped {
  fn drop(&mut self) {
    // The loop's children are in its process group only where the test started it so.
    signal(-self.pid(), libc::SIGKILL);
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

struct Root {
  data: TempDir,
  asks: Arc<Mutex<Asks>>,
}

impl Root {
  /// A data root whose host answers on its socket.
  fn new() -> Self {
    let data = tempfile::tempdir().unwrap();
    let layout = Layout::new(data.path());

    fs::create_dir_all(&layout.executions).unwrap();

    let asks = Arc::new(Mutex::new(Asks::new(data.path())));

    listen(&layout.socket(), asks.clone()).unwrap();
    Self { data, asks }
  }

  /// The host's own loop ending followers, as it does every half second.
  fn giving_way(&self) {
    let asks = self.asks.clone();

    thread::spawn(move || {
      loop {
        asks.lock().unwrap().give_way();
        thread::sleep(Duration::from_millis(200));
      }
    });
  }

  fn place(&self, agent: &str) -> PathBuf {
    let place = self.data.path().join("agent-places").join(agent);

    fs::create_dir_all(&place).unwrap();
    place
  }

  fn socket(&self) -> PathBuf {
    Layout::new(self.data.path()).socket()
  }

  fn write(&self, agent: &str, file: &str, value: Value) {
    fs::write(self.place(agent).join(file), value.to_string()).unwrap();
  }

  /// A file of the place, or null where it is not written yet.
  fn read(&self, agent: &str, file: &str) -> Value {
    fs::read(self.place(agent).join(file))
      .ok()
      .and_then(|bytes| serde_json::from_slice(&bytes).ok())
      .unwrap_or(Value::Null)
  }

  fn program(&self, agent: &str, script: &str, more: Value) {
    let mut program = json!({
      "identity": "one",
      "command": { "args": [SH, "-c", script], "cwd": self.data.path(), "env": { "PATH": std::env::var("PATH").unwrap(), "OWN": "program" } },
      "restart": null,
    });

    for (key, value) in more.as_object().unwrap() {
      program[key] = value.clone();
    }

    self.write(agent, "program.json", program);
  }

  /// A live process recorded as holding the agent's program.
  fn held_by(&self, agent: &str, holder: &Holder) {
    let identity = holder.identity();

    self.write(
      agent,
      "running.json",
      json!({ "identity": "one", "pid": identity["pid"], "birth": identity["birth"], "loop": identity, "starts": 1, "startedAt": now_ms(), "recent": [now_ms()] }),
    );
  }

  fn head(&self, agent: &str, pane: &str, more: &[(&str, &str)]) -> Command {
    let mut head = Command::new(head_binary());

    head
      .arg(agent)
      .env("BMO_DATA_DIR", self.data.path())
      .env("HERDR_PANE_ID", pane)
      .env_remove("BMO_HOST_HEAD")
      .envs(more.iter().copied())
      .stdin(Stdio::null());
    head
  }

  fn run(&self, agent: &str, pane: &str, more: &[(&str, &str)]) -> Output {
    self.head(agent, pane, more).output().unwrap()
  }

  fn spawn(&self, agent: &str, pane: &str, more: &[(&str, &str)]) -> Reaped {
    Reaped(self.head(agent, pane, more).stdout(Stdio::null()).spawn().unwrap())
  }

  fn head_record(&self, agent: &str, pane: &str) -> Option<Value> {
    let path = Layout::new(self.data.path()).heads().join(agent).join(format!("{pane}.json"));

    serde_json::from_slice(&fs::read(path).ok()?).ok()
  }

  fn ask(&self, agent: &str, pid: i32) -> Answer {
    ask(
      &self.socket(),
      &Request::Ask {
        agent: agent.to_owned(),
        pane: None,
        pid,
        host_head: false,
      },
    )
    .unwrap()
  }
}

fn cmdline(pid: i32) -> String {
  fs::read(format!("/proc/{pid}/cmdline"))
    .map(|bytes| String::from_utf8_lossy(&bytes).replace('\0', " "))
    .unwrap_or_default()
}

#[test]
fn the_program_takes_the_heads_own_process_with_the_panes_terminal_over_its_environment() {
  let root = Root::new();
  let said = root.data.path().join("said");

  root.program(
    "chat",
    &format!(
      "echo $$ $TERM $OWN ${{LEAKED:-none}} $HERDR_PANE_ID $HERDR_SOCKET_PATH > {}; sleep 30",
      said.display()
    ),
    json!({}),
  );

  let head = root.spawn(
    "chat",
    "p1",
    &[("TERM", "xterm-test"), ("LEAKED", "pane"), ("HERDR_SOCKET_PATH", "/run/herdr.sock")],
  );

  // The pane's terminal and herdr's variables for the pane are handed on; nothing else of the pane's.
  assert!(within(3_000, || fs::read_to_string(&said).is_ok_and(|text| text.ends_with('\n'))));
  assert_eq!(
    fs::read_to_string(&said).unwrap(),
    format!("{} xterm-test program none p1 /run/herdr.sock\n", head.pid())
  );

  let running = root.read("chat", "running.json");

  assert_eq!(running["pid"], head.pid());
  assert_eq!(running["loop"]["pid"], head.pid());
  assert_eq!((running["identity"].as_str(), running["starts"].as_u64()), (Some("one"), Some(1)));
  assert_eq!(running["paneId"], "p1");
  assert_eq!(root.head_record("chat", "p1").unwrap()["holds"], "program");
}

#[test]
fn a_program_whose_command_names_an_error_file_writes_its_error_stream_there_and_is_not_handed_the_name() {
  let root = Root::new();
  let errors = root.data.path().join("stderr.log");
  let said = root.data.path().join("said");

  fs::write(&errors, "an earlier start\n").unwrap();
  root.program(
    "chat",
    &format!("echo ${{BMO_PANE_STDERR:-unnamed}} > {}; echo the cause >&2", said.display()),
    json!({ "command": { "args": [SH, "-c", format!("echo ${{BMO_PANE_STDERR:-unnamed}} > {}; echo the cause >&2", said.display())], "cwd": root.data.path(), "env": { "PATH": std::env::var("PATH").unwrap(), "BMO_PANE_STDERR": errors } } }),
  );

  let ended = root.run("chat", "p1", &[]);

  assert!(ended.status.success(), "{ended:?}");
  assert!(String::from_utf8_lossy(&ended.stderr).is_empty(), "{ended:?}");
  assert_eq!(fs::read_to_string(&said).unwrap(), "unnamed\n");
  assert_eq!(fs::read_to_string(&errors).unwrap(), "an earlier start\nthe cause\n");
}

#[test]
fn a_head_record_and_a_start_left_by_a_process_that_is_gone_read_as_absent() {
  let root = Root::new();
  let mut gone = Command::new("true").spawn().unwrap();
  let pid = gone.id() as i32;
  let birth = ProcessIdentity::of(pid).map(|one| one.birth).unwrap_or_default();

  gone.wait().unwrap();
  root.program("chat", "sleep 30", json!({}));

  let stale = json!({ "pid": pid, "birth": birth, "holds": "program", "paneId": "old", "hostHead": false });
  let heads = Layout::new(root.data.path()).heads().join("chat");

  fs::create_dir_all(&heads).unwrap();
  fs::write(heads.join("old.json"), stale.to_string()).unwrap();
  root.write(
    "chat",
    "running.json",
    json!({ "identity": "one", "pid": pid, "birth": birth, "loop": { "pid": pid, "birth": birth }, "starts": 1, "startedAt": now_ms() }),
  );

  let head = root.spawn("chat", "p2", &[]);

  assert!(within(3_000, || root.read("chat", "running.json")["pid"] == head.pid()));
  assert!(root.head_record("chat", "old").is_none());
  assert_eq!(root.read("chat", "running.json")["starts"], 2);
}

#[test]
fn a_program_started_often_waits_its_bound_and_holds_the_place_meanwhile() {
  let root = Root::new();
  let said = root.data.path().join("started");
  let now = now_ms();

  root.program("chat", &format!("touch {}; sleep 30", said.display()), json!({}));
  root.write(
    "chat",
    "running.json",
    json!({ "identity": "one", "pid": 0, "birth": null, "loop": { "pid": 0, "birth": "" }, "starts": 3, "startedAt": now - 3_000, "recent": [now - 30_000, now - 20_000, now - 10_000] }),
  );

  let began = Instant::now();
  let head = root.spawn("chat", "p1", &[]);

  // The head writes its wait the moment it decides on it, however long a loaded machine takes to start it, and
  // the program waits its bound from then: the wait is read as soon as it stands, before the bound has passed.
  assert!(within(30_000, || root.read("chat", "running.json")["pendingUntil"].is_u64()));

  let running = root.read("chat", "running.json");

  assert!(running["pendingUntil"].as_u64().unwrap() >= now + 2_000);
  assert_eq!(running["starts"], 4);
  // Waiting, it holds the place: a second head is shown the view.
  assert!(matches!(root.ask("chat", std::process::id() as i32), Answer::View { .. }));
  assert!(!said.exists());
  assert!(within(4_000, || said.exists()));
  assert!(began.elapsed() >= Duration::from_millis(1_900));
  assert_eq!(root.read("chat", "running.json")["pid"], head.pid());
}

#[test]
fn a_session_is_opened_until_its_record_exists_and_resumed_after() {
  let root = Root::new();
  let opened = root.data.path().join("session.jsonl");
  let restart = json!({ "args": [SH, "-c", "resume"], "cwd": root.data.path(), "env": {} });
  // Each start is a head of its own, whose program has ended by the next one.
  let start = || {
    let head = Holder::new();

    match root.ask("chat", head.0.id() as i32) {
      Answer::Program { args, .. } => args[2].clone(),
      other => panic!("{other:?}"),
    }
  };

  root.program("chat", "open", json!({ "restart": restart, "opened": opened }));

  // The first start opens it; a start after it with no record behind the session opens it again.
  assert_eq!(start(), "open");
  assert_eq!(start(), "open");
  fs::write(&opened, "").unwrap();
  assert_eq!(start(), "resume");
  assert_eq!(root.read("chat", "running.json")["starts"], 3);
}

#[test]
fn a_program_that_cannot_start_is_written_down_and_the_head_ends_for_the_loop() {
  let root = Root::new();

  root.write(
    "chat",
    "program.json",
    json!({ "identity": "broken", "command": { "args": ["no-such-program-anywhere"], "cwd": root.data.path(), "env": { "PATH": "/nowhere" } }, "restart": null }),
  );

  let ended = root.run("chat", "p1", &[]);

  assert!(ended.status.success());
  assert!(String::from_utf8_lossy(&ended.stdout).contains("could not be started"));

  let failed = root.read("chat", "start-failed.json");

  assert_eq!(failed["identity"], "broken");
  assert_eq!(failed["reason"], "no-such-program-anywhere: no such executable on PATH=/nowhere");

  let running = root.read("chat", "running.json");

  assert_eq!((running["pid"].as_i64(), running["starts"].as_u64()), (Some(0), Some(1)));
}

#[test]
fn a_program_held_elsewhere_gets_the_view_and_the_view_gives_way_once_it_is_not() {
  let root = Root::new();
  let holder = Holder::new();

  root.giving_way();
  root.program("chat", "sleep 30", json!({}));
  root.held_by("chat", &holder);

  let mut follower = root.spawn("chat", "p2", &[]);

  assert!(within(3_000, || cmdline(follower.pid()).starts_with("tail -n 200 -F ")));
  assert!(cmdline(follower.pid()).ends_with("agent-host/views/chat "));
  assert_eq!(root.head_record("chat", "p2").unwrap()["holds"], "view");

  drop(holder);

  // Nobody holds the program now, and the follower ends so its loop can start the program there.
  assert!(follower.ended(6_000).is_some(), "the follower stood in the program's way");
}

#[test]
fn a_head_opened_for_a_host_beside_the_program_lets_go_and_no_host_is_a_line_and_a_clean_end() {
  let root = Root::new();
  let holder = Holder::new();

  root.program("chat", "sleep 30", json!({}));
  root.held_by("chat", &holder);

  let let_go = root.run("chat", "p3", &[("BMO_HOST_HEAD", "1")]);

  assert_eq!(let_go.status.code(), Some(3));

  // A host agent's pane, with nothing written, is its view whatever opened it.
  let host = Root::new();
  let view = host.spawn("gate", "p4", &[("BMO_HOST_HEAD", "1")]);

  assert!(within(3_000, || cmdline(view.pid()).starts_with("tail ")));

  let nowhere = tempfile::tempdir().unwrap();
  let unanswered = Command::new(head_binary())
    .arg("chat")
    .env("BMO_DATA_DIR", nowhere.path())
    .output()
    .unwrap();

  assert!(unanswered.status.success());
  assert!(String::from_utf8_lossy(&unanswered.stdout).contains("does not answer"));
}

// The pane's loop, as the launcher writes it: a program ended by a signal is started again, and only
// `HEAD_STOPPED` ends the loop.
#[test]
fn the_loop_starts_the_head_again_after_a_signal_and_stops_only_on_head_stopped() {
  let root = Root::new();
  let looped = |more: &[(&str, &str)]| {
    let mut shell = Command::new(SH);

    shell
      .args([
        "-c",
        "while :; do \"$1\" \"$2\"; case $? in 3) exit 0;; esac; sleep 1; done",
        "bmo-head-chat",
        head_binary(),
        "chat",
      ])
      .env("BMO_DATA_DIR", root.data.path())
      .env("HERDR_PANE_ID", "p1")
      .env_remove("BMO_HOST_HEAD")
      .envs(more.iter().copied())
      .stdin(Stdio::null())
      .stdout(Stdio::null());
    // SAFETY: setpgid in the child before exec, so the test can end the loop's whole group.
    unsafe {
      std::os::unix::process::CommandExt::pre_exec(&mut shell, || {
        libc::setpgid(0, 0);
        Ok(())
      });
    }
    Reaped(shell.spawn().unwrap())
  };

  root.program("chat", "sleep 30", json!({}));

  let mut pane = looped(&[]);
  let mut first = 0;

  assert!(
    within(3_000, || {
      first = root.read("chat", "running.json")["pid"].as_i64().unwrap_or(0) as i32;
      first > 0 && cmdline(first).starts_with("/bin/sh -c sleep 30")
    }),
    "the program never stood"
  );
  signal(first, libc::SIGTERM);

  assert!(within(4_000, || {
    let running = root.read("chat", "running.json");
    running["starts"] == 2 && running["pid"] != first && alive(running["pid"].as_i64().unwrap() as i32)
  }));
  assert!(pane.ended(0).is_none(), "the loop ended on a signal");

  // A head opened for a host beside the program is told to let go, and its loop ends with it.
  let mut beside = looped(&[("BMO_HOST_HEAD", "1"), ("HERDR_PANE_ID", "p9")]);

  assert!(beside.ended(3_000).is_some_and(|status| status.success()));
  assert!(pane.ended(0).is_none());
}
