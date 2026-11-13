//! A job turn's tool calls held to a clock (docs/agent-control.md, 「A job turn's tool calls are held to a
//! clock」): a call blocked on a pipe nothing writes to is cut after it stops burning processor time, a call
//! that keeps burning it is left alone, and a stop cuts a blocked call so its words can be read. The lines
//! the watcher writes name the call, its pid, its duration and its outcome.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agency_server::door::execution::Identity;
use agency_server::door::looks::GoneConfirm;
use agency_server::door::stand::Interrupt;
use agency_server::door::tool_clock::{CallVerdict, SilentClock, ToolClock, Watch as ToolWatch};
use agency_server::door::turn::{Ending, Seat, Timings, Turn, Watch};
use agency_server::places::Ground;
use harness_providers::Machine;
use harness_providers::claude_code::ClaudeCode;
use serde_json::{Value, json};

use crate::integration::common;
use crate::integration::common::Scratch;
use crate::integration::common::herdr::HerdrModel;

// A program of the test's own, in a process group of its own so every process it starts is taken down with
// it.
struct Program {
  child: Child,
  pid: i32,
}

impl Program {
  fn spawn(script: &str) -> Self {
    Self::spawn_args(&["bash", "-c", script])
  }

  fn spawn_args(args: &[&str]) -> Self {
    let child = Command::new(args[0])
      .args(&args[1..])
      .process_group(0)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .unwrap();
    let pid = child.id() as i32;

    Self { child, pid }
  }

  // A program making a shell call as the turn's program makes one: the call a shell of its own under it,
  // its standard output and standard error on a file of its own.
  fn spawn_writing(script: &str, output: &Path) -> Self {
    let file = std::fs::File::create(output).unwrap();
    let child = Command::new("bash")
      .args(["-c", "bash -c \"$0\" & wait", script])
      .process_group(0)
      .stdin(Stdio::null())
      .stdout(file.try_clone().unwrap())
      .stderr(file)
      .spawn()
      .unwrap();
    let pid = child.id() as i32;

    Self { child, pid }
  }
}

impl Drop for Program {
  fn drop(&mut self) {
    // SAFETY: a signal sent to the process group this test started.
    unsafe {
      libc::kill(-self.pid, libc::SIGKILL);
    }

    let _ = self.child.wait();
  }
}

fn clock(min_ms: i64, stall_ms: i64, stop_stall_ms: i64) -> ToolClock {
  ToolClock {
    min_ms,
    stall_ms,
    look_ms: 20,
    in_process_ms: 400,
    in_process_grace_ms: 2_000,
    in_process_cut: "The tool call {tool} was cut after {seconds} seconds; go on without it.".to_owned(),
    in_process_grace_cut: "cut at its bound after a tool call did not take its cut".to_owned(),
    log_after_ms: 100,
    still_every_ms: 100,
    stop_stall_ms,
    stop_bound_ms: 5_000,
    silent: unasked(),
  }
}

// A silent rule whose window no test's call reaches, for the tests of the first rule.
fn unasked() -> SilentClock {
  SilentClock {
    window_ms: i64::MAX / 4,
    idle_cpu_percent: 5,
    ask_chars: 400,
    cut: "The tool call {tool} was cut on the supervisor's call after {seconds} seconds: {why}.".to_owned(),
  }
}

fn stamp(ms: i64) -> String {
  chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms).unwrap().to_rfc3339()
}

fn used(id: &str, command: &str, at: i64) -> Value {
  json!({
    "type": "assistant", "timestamp": stamp(at),
    "message": { "content": [
      { "type": "tool_use", "id": id, "name": "Bash", "input": { "command": command } }
    ] }
  })
}

fn answered(id: &str, at: i64, is_error: bool) -> Value {
  json!({
    "type": "user", "timestamp": stamp(at),
    "message": { "content": [
      { "type": "tool_result", "tool_use_id": id, "is_error": is_error, "content": "killed" }
    ] }
  })
}

fn pid_of(line: &str) -> i32 {
  line
    .split("pid=")
    .nth(1)
    .and_then(|rest| rest.split_whitespace().next())
    .and_then(|pid| pid.parse().ok())
    .unwrap_or(0)
}

async fn gone(pid: i32) -> bool {
  for _ in 0..40 {
    if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
      return true;
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
  }

  false
}

// A child of the program blocked reading a pipe nothing writes to: the script opens a fifo read-write and
// hands it to `cat`, which then waits on a read that never comes.
const BLOCKED: &str = "f=$(mktemp -u); mkfifo \"$f\"; exec 3<>\"$f\"; cat <&3";

#[tokio::test]
async fn a_call_blocked_on_a_pipe_is_cut_and_its_lines_carry_the_pid_and_duration() {
  let program = Program::spawn(BLOCKED);
  let mut watch = ToolWatch::new(clock(300, 400, 100));
  let began = agency_server::now_ms();
  let mut records = vec![used("t1", "cat", began)];
  let mut said = vec![];
  let mut cut = None;

  for step in 0..40 {
    let now = began + step * 100;

    said.extend(watch.look("job-41", &records, 0, program.pid, now).await);

    if said.iter().any(|line| line.contains("cut after")) {
      cut = Some(now);

      break;
    }

    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  let cut = cut.unwrap_or_else(|| panic!("the blocked call was cut within its clock: {said:?}"));

  assert!(cut - began <= 4_000, "the cut is bounded: {}ms", cut - began);

  let start = said.iter().find(|line| line.contains("started pid=")).expect("a start line");
  let still = said.iter().find(|line| line.contains("still running")).expect("a still-running line");
  let cut_line = said.iter().find(|line| line.contains("cut after")).unwrap();

  assert!(
    start.contains("tool Bash") && start.contains("cat"),
    "the start names the call: {start}"
  );
  assert!(pid_of(start) > 1, "the start carries its pid: {start}");
  assert!(still.contains("pid="), "the still-running line carries its pid: {still}");
  assert!(cut_line.contains("pid="), "the cut line carries its pid: {cut_line}");
  assert!(gone(pid_of(cut_line)).await, "the call's process is gone");

  // The harness reports the killed call to the model, and its end is written with its duration and outcome.
  records.push(answered("t1", began + 2_000, true));
  said.extend(watch.look("job-41", &records, 0, program.pid, began + 2_000).await);

  let end = said.iter().find(|line| line.contains("ended pid=")).expect("an end line");

  assert!(end.contains("after 2.0s: failed"), "the end names its duration and outcome: {end}");
  assert!(pid_of(end) > 1, "the end carries its pid: {end}");
}

#[tokio::test]
async fn a_call_that_keeps_burning_processor_is_not_cut() {
  let program = Program::spawn("bash -c 'while :; do :; done' & wait");
  let mut watch = ToolWatch::new(clock(200, 200, 100));
  let began = agency_server::now_ms();
  let records = vec![used("t1", "loop", began)];
  let mut said = vec![];

  for step in 0..15 {
    said.extend(watch.look("job-41", &records, 0, program.pid, began + step * 100).await);
    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  assert!(
    !said.iter().any(|line| line.contains("cut after")),
    "a working call is left to finish: {said:?}"
  );
  assert!(said.iter().any(|line| line.contains("still running")), "it is still reported: {said:?}");
}

// A call waiting inside the program with no process of its own: the program has no children, so its
// call owns no tree. Past the call's own bound it is cut, and the words that end it are handed out.
#[tokio::test]
async fn a_call_with_no_process_of_its_own_is_cut_at_its_in_process_bound() {
  let program = Program::spawn("exec sleep 100000");
  let mut watch = ToolWatch::new(clock(300, 400, 100));
  let began = agency_server::now_ms();
  let mut records = vec![json!({
    "type": "assistant", "timestamp": stamp(began),
    "message": { "content": [
      { "type": "tool_use", "id": "t1", "name": "WebFetch", "input": { "url": "https://example.invalid/" } }
    ] }
  })];
  let mut said = vec![];
  let mut cut = None;

  for step in 0..40 {
    let now = began + step * 100;

    said.extend(watch.look("job-41", &records, 0, program.pid, now).await);

    if let Some(found) = watch.take_in_process_cut() {
      cut = Some((now, found));

      break;
    }

    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  let (at, cut) = cut.unwrap_or_else(|| panic!("the in-process call was cut: {said:?}"));

  assert!(at - began <= 1_500, "the cut is bounded: {}ms", at - began);
  assert!(
    said
      .iter()
      .any(|line| line.contains("cut after") && line.contains("no process of its own")),
    "the cut line says the call had no process: {said:?}"
  );
  assert!(cut.words.contains("WebFetch"), "the words name the call: {}", cut.words);
  assert!(watch.in_process_cut_still(), "the call stands until its result is written");

  // The program takes the words and answers the call, so the clock lets it go.
  records.push(answered("t1", at + 100, true));
  said.extend(watch.look("job-41", &records, 0, program.pid, at + 100).await);

  assert!(!watch.in_process_cut_still(), "the answered call is let go");
  assert!(
    said.iter().any(|line| line.contains("ended pid=-") && line.contains("failed")),
    "the end is written with the call's outcome: {said:?}"
  );
}

#[tokio::test]
async fn a_stop_cuts_a_call_blocked_on_a_pipe_within_its_bound() {
  let program = Program::spawn(BLOCKED);
  let mut watch = ToolWatch::new(clock(60_000, 60_000, 100));
  let began = agency_server::now_ms();
  let records = vec![used("t1", "cat", began)];

  // A look first, so the call's process is known.
  watch.look("job-41", &records, 0, program.pid, began).await;
  tokio::time::sleep(Duration::from_millis(200)).await;

  let said = watch.cut_for_stop("job-41", &records, 0, program.pid, began + 300).await;
  let cut = said.iter().find(|line| line.contains("cut for the stop")).unwrap_or_else(|| {
    panic!("the blocked call was cut for the stop: {said:?}");
  });

  assert!(pid_of(cut) > 1, "the cut carries its pid: {cut}");
  assert!(gone(pid_of(cut)).await, "the call's process is gone");
}

// A silent rule a test's call reaches within a second: its idle share is generous, since a loaded machine
// spends more on starting a process than the policy's share allows.
fn asking(min_ms: i64, window_ms: i64) -> ToolClock {
  ToolClock {
    silent: SilentClock {
      window_ms,
      idle_cpu_percent: 30,
      ..unasked()
    },
    ..clock(min_ms, 60_000, 100)
  }
}

// A wait that polls as a job's does: one line said, then a loop that sleeps and looks for a file that never
// comes, beginning a process on every pass and writing nothing.
const POLLING: &str =
  "echo waiting for the other job to close; until [ -e /nonexistent/closed ]; do sleep 0.1; done";

// The clock looked at every 100 ms for up to `steps` looks, until a call is handed over as silent.
async fn looked_until_silent(
  watch: &mut ToolWatch,
  records: &[Value],
  program: i32,
  began: i64,
  steps: i64,
  said: &mut Vec<String>,
) -> Option<(i64, agency_server::door::tool_clock::SilentCall)> {
  for step in 0..steps {
    let now = began + step * 100;

    said.extend(watch.look("job-41", records, 0, program, now).await);

    if let Some(silent) = watch.take_silent() {
      return Some((now, silent));
    }

    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  None
}

#[tokio::test]
async fn a_call_that_polls_with_no_new_output_is_asked_about_and_runs_on() {
  let scratch = tempfile::tempdir().unwrap();
  let program = Program::spawn_writing(POLLING, &scratch.path().join("call.output"));
  let mut watch = ToolWatch::new(asking(200, 400));
  let began = agency_server::now_ms();
  let records = vec![used("t1", POLLING, began)];
  let mut said = vec![];
  let (at, silent) = looked_until_silent(&mut watch, &records, program.pid, began, 30, &mut said)
    .await
    .unwrap_or_else(|| panic!("the polling call was handed over as silent: {said:?}"));

  assert!(at - began <= 2_000, "it is asked about within its window: {}ms", at - began);
  assert_eq!(silent.id, "t1");
  assert!(
    silent.tool.starts_with("Bash echo waiting"),
    "the ask carries the command: {}",
    silent.tool
  );
  assert!(
    silent.tool.contains("sleep 0.1"),
    "the whole command, not the line's cut: {}",
    silent.tool
  );
  assert_eq!(silent.started_at, began, "how long it has run is read from its start");
  assert!(silent.output_changed_at < at, "its output last changed before the window");
  assert_eq!(silent.last_line.as_deref(), Some("waiting for the other job to close"));
  assert!(silent.cpu_percent < 30.0, "its share is under the idle share: {}", silent.cpu_percent);
  assert_eq!(silent.window_ms, 400);
  assert!(
    !said.iter().any(|line| line.contains("cut after")),
    "nothing cuts a call that is asked about: {said:?}"
  );

  // An ask stands with no verdict: the call runs on, and the verdict is looked for on the next read.
  let answered = watch.answer("job-41", "t1", CallVerdict::Waiting, at).await;

  assert!(
    answered.lines.iter().any(|line| line.contains("asked about")),
    "the ask is written: {answered:?}"
  );
  assert!(answered.words.is_none());
  tokio::time::sleep(Duration::from_millis(100)).await;
  said.extend(watch.look("job-41", &records, 0, program.pid, at + 100).await);

  let polled = watch.take_silent().expect("an ask that stands is looked at again");

  assert_eq!(polled.id, "t1");
  assert!(std::path::Path::new(&format!("/proc/{}", program.pid)).exists(), "the call runs on");
  assert!(
    watch.answer("job-41", "t1", CallVerdict::Waiting, at + 100).await.lines.is_empty(),
    "a verdict looked for again writes no second ask"
  );
}

#[tokio::test]
async fn a_call_whose_output_keeps_growing_is_not_asked_about() {
  let scratch = tempfile::tempdir().unwrap();
  let script = "while :; do echo one more pass; sleep 0.1; done";
  let program = Program::spawn_writing(script, &scratch.path().join("call.output"));
  let mut watch = ToolWatch::new(asking(200, 400));
  let began = agency_server::now_ms();
  let records = vec![used("t1", script, began)];
  let mut said = vec![];

  assert!(
    looked_until_silent(&mut watch, &records, program.pid, began, 15, &mut said)
      .await
      .is_none(),
    "a call writing on every pass is left alone: {said:?}"
  );
  assert!(!said.iter().any(|line| line.contains("cut after")), "and is not cut: {said:?}");
}

#[tokio::test]
async fn a_kept_wait_goes_on_and_is_asked_about_again_once_its_keep_runs_out() {
  let scratch = tempfile::tempdir().unwrap();
  let program = Program::spawn_writing(POLLING, &scratch.path().join("call.output"));
  let mut watch = ToolWatch::new(asking(200, 400));
  let began = agency_server::now_ms();
  let records = vec![used("t1", POLLING, began)];
  let mut said = vec![];
  let (at, _) = looked_until_silent(&mut watch, &records, program.pid, began, 30, &mut said)
    .await
    .expect("the polling call was handed over");

  // The supervisor keeps it, with a time to look again.
  let until = at + 600;
  let kept = watch.answer("job-41", "t1", CallVerdict::Kept(until), at).await;

  assert!(kept.lines.iter().any(|line| line.contains("kept on the supervisor's call")), "{kept:?}");

  let mut again = None;

  for step in 1..20 {
    let now = at + step * 100;

    tokio::time::sleep(Duration::from_millis(100)).await;
    said.extend(watch.look("job-41", &records, 0, program.pid, now).await);

    if let Some(silent) = watch.take_silent() {
      again = Some((now, silent));

      break;
    }

    assert!(
      std::path::Path::new(&format!("/proc/{}", program.pid)).exists(),
      "the kept call runs on"
    );
  }

  let (again_at, again) = again.expect("still silent and idle, it is asked about again");

  assert!(again_at >= until, "not before its keep runs out: {}ms early", until - again_at);
  assert_eq!(again.last_line.as_deref(), Some("waiting for the other job to close"));

  // A cut stops its processes and gives the words for the turn with the supervisor's reason.
  let cut = watch
    .answer("job-41", "t1", CallVerdict::Cut("it waits on another job's close".into()), again_at)
    .await;
  let line = cut
    .lines
    .iter()
    .find(|line| line.contains("cut on the supervisor's call"))
    .expect("the cut line");
  let words = cut.words.expect("the words for the turn");

  assert!(words.contains("it waits on another job's close") && words.contains("Bash"), "{words}");
  assert!(gone(pid_of(line)).await, "the cut call's process is gone: {line}");
}

fn append(path: &Path, lines: &[Value]) {
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();

  let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();

  for line in lines {
    writeln!(file, "{line}").unwrap();
  }
}

// The lines a program writes: the words it was handed, a call it made, the answer to the call, and the end
// of its turn.
fn handed(words: &str) -> Value {
  json!({ "type": "user", "message": { "role": "user", "content": words } })
}

fn answer(words: &str) -> Value {
  json!({ "type": "assistant", "message": {
    "id": "m-1", "model": "claude-sonnet-5", "role": "assistant",
    "content": [{ "type": "text", "text": words }],
    "usage": { "input_tokens": 10, "output_tokens": 2 },
  }})
}

fn turn_end() -> Value {
  json!({ "type": "system", "subtype": "turn_duration" })
}

// The lines a test's watcher keeps, and the clock it hands the door.
#[derive(Clone)]
struct Notes(Arc<Mutex<Vec<String>>>, ToolClock);

impl Watch for Notes {
  async fn note(&mut self, text: &str) {
    self.0.lock().unwrap().push(text.to_owned());
  }

  fn tool_clock(&self) -> Option<ToolClock> {
    Some(self.1.clone())
  }
}

fn wrote(notes: &Arc<Mutex<Vec<String>>>, text: &str) -> bool {
  notes.lock().unwrap().iter().any(|line| line.contains(text))
}

// A job's turn whose program blocks on a pipe: the door cuts the call by its clock, the model is handed the
// failed result, and the turn ends with its answer rather than waiting for ever.
#[tokio::test(flavor = "multi_thread")]
async fn the_door_cuts_a_job_turn_s_blocked_call_and_the_turn_ends() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let config = home.path().join("config");
  let cwd = home.path().join("work");
  std::fs::create_dir_all(&cwd).unwrap();
  std::fs::create_dir_all(config.join("herdr")).unwrap();

  let herdr_model = HerdrModel::stand(config.join("herdr").join("herdr.sock"));
  let at = |path: &Path| path.to_string_lossy().into_owned();
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), at(home.path())),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
      ("XDG_CONFIG_HOME".to_owned(), at(&config)),
      ("BMO_DEPLOY_DIR".to_owned(), env!("CARGO_MANIFEST_DIR").to_owned()),
    ],
    home.path().join("data"),
  );
  let service = common::service(&scratch, &[("AGENCY_SOURCE_DIR", at(&home.path().join("source")))]);
  let ground = Ground::new(&service, &machine, Some(herdr::Herdr::there(herdr_model.socket.clone())));
  let session = "session-cut-1";
  let record = ClaudeCode::records_under(&config, &cwd, session).swap_remove(0);
  append(&record, &[]);

  let command = vec!["bash".to_owned(), "-c".to_owned(), "the work".to_owned()];
  // The blocked call begins only once the turn has made it, as a program's own call does: the script
  // waits for the trigger the test writes with the tool_use.
  let trigger = home.path().join("go");
  let blocked = format!(
    "while [ ! -e {} ]; do sleep 0.05; done; ( {} ) & while :; do sleep 0.05; done",
    trigger.display(),
    BLOCKED
  );
  let program = Program::spawn_args(&["bash", "-c", &blocked, "--session-id", session]);

  let turn = Turn {
    name: "job-41".into(),
    title: "the work".into(),
    seat: Seat::Own(herdr::Herdr::there(herdr_model.socket.clone())),
    cwd: at(&cwd),
    command: command.clone(),
    env: BTreeMap::from([
      ("CLAUDE_CONFIG_DIR".to_owned(), at(&config)),
      ("HOME".to_owned(), at(home.path())),
    ]),
    prompt: "the work".into(),
    session: session.into(),
    identity: Identity {
      agent: None,
      root: None,
      parent: None,
      role: Some("worker".into()),
      model: Some("claude-sonnet-5".into()),
      effort: None,
    },
    line: "worker".into(),
    provider: "claude-code".into(),
    lead: String::new(),
    trusts_folder: true,
    clock: None,
    interrupt: Interrupt::Keys(vec!["escape".into()]),
    timings: Timings {
      poll: Duration::from_millis(30),
      redraw: Duration::from_millis(500),
      submit: Duration::from_secs(5),
      gone: GoneConfirm::default(),
    },
  };
  let notes = Arc::new(Mutex::new(Vec::new()));
  let watch = Notes(
    notes.clone(),
    ToolClock {
      min_ms: 400,
      stall_ms: 500,
      look_ms: 20,
      in_process_ms: 400,
      in_process_grace_ms: 2_000,
      in_process_cut: "The tool call {tool} was cut after {seconds} seconds; go on without it.".to_owned(),
      in_process_grace_cut: "cut at its bound after a tool call did not take its cut".to_owned(),
      log_after_ms: 100,
      still_every_ms: 100,
      stop_stall_ms: 100,
      stop_bound_ms: 5_000,
      silent: unasked(),
    },
  );

  // The program the pane plays: the door's pane made working, the prompt seen, a call made, and once the
  // call is cut the result, answer and end written.
  let herdr_for_writer = herdr_model.clone();
  let record_for_writer = record.clone();
  let command_for_writer = command.clone();
  let trigger_for_writer = trigger.clone();
  let writing = tokio::spawn(async move {
    let mut pane = None;

    for _ in 0..400 {
      pane = herdr_for_writer
        .panes()
        .into_iter()
        .find(|pane| pane.command.as_ref() == Some(&command_for_writer))
        .map(|pane| pane.id);

      if pane.is_some() {
        break;
      }

      tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let pane = pane.expect("the door's pane");

    // The pane comes up at rest, so the door may put its prompt in; then it works.
    herdr_for_writer.agent_status(&pane, "idle");

    for _ in 0..400 {
      if !herdr_for_writer.typed().is_empty() {
        break;
      }

      tokio::time::sleep(Duration::from_millis(10)).await;
    }

    herdr_for_writer.agent_status(&pane, "working");
    append(&record_for_writer, &[handed("the work")]);
    tokio::time::sleep(Duration::from_millis(300)).await;
    append(
      &record_for_writer,
      &[json!({ "type": "assistant", "message": { "content": [
        { "type": "tool_use", "id": "t1", "name": "Bash", "input": { "command": "cat" } }
      ] } })],
    );
    std::fs::write(&trigger_for_writer, "").unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    append(
      &record_for_writer,
      &[
        json!({ "type": "user", "message": { "content": [
          { "type": "tool_result", "tool_use_id": "t1", "is_error": true, "content": "killed" }
        ] } }),
        answer("the call was cut and the work went on"),
        turn_end(),
      ],
    );
    herdr_for_writer.agent_status(&pane, "idle");
  });

  let checking = record.clone();
  let ended = tokio::time::timeout(Duration::from_secs(20), async {
    let mut watch = watch;

    agency_server::door::turn::put(&ground, &turn, &mut watch).await
  })
  .await
  .unwrap_or_else(|_| {
    panic!(
      "the turn ends within its bound; notes: {:?}; writer finished: {}; record:\n{}",
      notes.lock().unwrap(),
      writing.is_finished(),
      std::fs::read_to_string(&checking).unwrap_or_default()
    )
  })
  .expect("the turn is put in and ended");

  writing.await.unwrap();
  drop(program);

  assert!(
    matches!(ended, Ending::Answered { .. }),
    "the turn answered after the call was cut: {ended:?}"
  );
  assert!(wrote(&notes, "tool Bash started pid="), "the start line: {:?}", notes.lock().unwrap());
  assert!(wrote(&notes, "still running"), "the still-running line: {:?}", notes.lock().unwrap());
  assert!(wrote(&notes, "cut after"), "the cut line: {:?}", notes.lock().unwrap());
}

// A job's turn whose program waits on a call with no process of its own: the door cuts the call at its
// own bound, puts the words into the working turn, and the model is handed the call's result and answers.
#[tokio::test(flavor = "multi_thread")]
async fn the_door_cuts_a_job_turn_s_in_process_call_and_the_model_gets_a_result() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let config = home.path().join("config");
  let cwd = home.path().join("work");
  std::fs::create_dir_all(&cwd).unwrap();
  std::fs::create_dir_all(config.join("herdr")).unwrap();

  let herdr_model = HerdrModel::stand(config.join("herdr").join("herdr.sock"));
  let at = |path: &Path| path.to_string_lossy().into_owned();
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), at(home.path())),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
      ("XDG_CONFIG_HOME".to_owned(), at(&config)),
      ("BMO_DEPLOY_DIR".to_owned(), env!("CARGO_MANIFEST_DIR").to_owned()),
    ],
    home.path().join("data"),
  );
  let service = common::service(&scratch, &[("AGENCY_SOURCE_DIR", at(&home.path().join("source")))]);
  let ground = Ground::new(&service, &machine, Some(herdr::Herdr::there(herdr_model.socket.clone())));
  let session = "session-cut-2";
  let record = ClaudeCode::records_under(&config, &cwd, session).swap_remove(0);
  append(&record, &[]);

  let command = vec!["bash".to_owned(), "-c".to_owned(), "the work".to_owned()];
  // The program has no child of its own, so the call it makes owns no process.
  let program = Program::spawn_args(&["bash", "-c", "while :; do :; done", "--session-id", session]);

  let turn = Turn {
    name: "job-42".into(),
    title: "the work".into(),
    seat: Seat::Own(herdr::Herdr::there(herdr_model.socket.clone())),
    cwd: at(&cwd),
    command: command.clone(),
    env: BTreeMap::from([
      ("CLAUDE_CONFIG_DIR".to_owned(), at(&config)),
      ("HOME".to_owned(), at(home.path())),
    ]),
    prompt: "the work".into(),
    session: session.into(),
    identity: Identity {
      agent: None,
      root: None,
      parent: None,
      role: Some("worker".into()),
      model: Some("claude-sonnet-5".into()),
      effort: None,
    },
    line: "worker".into(),
    provider: "claude-code".into(),
    lead: String::new(),
    trusts_folder: true,
    clock: None,
    interrupt: Interrupt::Keys(vec!["escape".into()]),
    timings: Timings {
      poll: Duration::from_millis(30),
      redraw: Duration::from_millis(500),
      submit: Duration::from_secs(5),
      gone: GoneConfirm::default(),
    },
  };
  let notes = Arc::new(Mutex::new(Vec::new()));
  let watch = Notes(
    notes.clone(),
    ToolClock {
      min_ms: 400,
      stall_ms: 500,
      look_ms: 20,
      in_process_ms: 400,
      in_process_grace_ms: 5_000,
      in_process_cut: "The tool call {tool} was cut after {seconds} seconds; go on without it.".to_owned(),
      in_process_grace_cut: "cut at its bound after a tool call did not take its cut".to_owned(),
      log_after_ms: 100,
      still_every_ms: 100,
      stop_stall_ms: 100,
      stop_bound_ms: 5_000,
      silent: unasked(),
    },
  );

  let herdr_for_writer = herdr_model.clone();
  let record_for_writer = record.clone();
  let command_for_writer = command.clone();
  let writing = tokio::spawn(async move {
    let mut pane = None;

    for _ in 0..400 {
      pane = herdr_for_writer
        .panes()
        .into_iter()
        .find(|pane| pane.command.as_ref() == Some(&command_for_writer))
        .map(|pane| pane.id);

      if pane.is_some() {
        break;
      }

      tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let pane = pane.expect("the door's pane");

    herdr_for_writer.agent_status(&pane, "idle");

    for _ in 0..400 {
      if !herdr_for_writer.prompts().is_empty() {
        break;
      }

      tokio::time::sleep(Duration::from_millis(10)).await;
    }

    herdr_for_writer.agent_status(&pane, "working");
    append(&record_for_writer, &[handed("the work")]);
    tokio::time::sleep(Duration::from_millis(300)).await;
    append(
      &record_for_writer,
      &[json!({ "type": "assistant", "message": { "content": [
        { "type": "tool_use", "id": "t1", "name": "WebFetch", "input": { "url": "https://example.invalid/" } }
      ] } })],
    );

    // The call has no process to signal, so the door cuts it by putting words into the turn.
    let mut cut = false;

    for _ in 0..600 {
      if herdr_for_writer.prompts().iter().any(|(_, text)| text.contains("go on without it")) {
        cut = true;

        break;
      }

      tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert!(cut, "the door put the cut's words into the working turn");

    // The program ends the waiting call on them and hands the model its result.
    append(
      &record_for_writer,
      &[
        json!({ "type": "user", "message": { "content": [
          { "type": "tool_result", "tool_use_id": "t1", "is_error": true, "content": "cut" }
        ] } }),
        answer("the call was cut and the work went on"),
        turn_end(),
      ],
    );
    herdr_for_writer.agent_status(&pane, "idle");
  });

  let checking = record.clone();
  let ended = tokio::time::timeout(Duration::from_secs(20), async {
    let mut watch = watch;

    agency_server::door::turn::put(&ground, &turn, &mut watch).await
  })
  .await
  .unwrap_or_else(|_| {
    panic!(
      "the turn ends within its bound; notes: {:?}; writer finished: {}; record:\n{}",
      notes.lock().unwrap(),
      writing.is_finished(),
      std::fs::read_to_string(&checking).unwrap_or_default()
    )
  })
  .expect("the turn is put in and ended");

  writing.await.unwrap();
  drop(program);

  assert!(
    matches!(ended, Ending::Answered { .. }),
    "the model answered after the in-process call was cut: {ended:?}"
  );
  assert!(
    wrote(&notes, "tool WebFetch started pid=-"),
    "the start line: {:?}",
    notes.lock().unwrap()
  );
  assert!(wrote(&notes, "cut after"), "the cut line: {:?}", notes.lock().unwrap());
  assert!(
    wrote(&notes, "no process of its own"),
    "the cut names the missing process: {:?}",
    notes.lock().unwrap()
  );
}
