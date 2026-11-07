//! A job turn's tool calls held to a clock (docs/agent-control.md, 「A job turn's tool calls are held to a
//! clock」). A call is read off the turn's record: its `tool_use` block until the `tool_result` that answers
//! it. The process tree of the turn's program is read as the kernel names it, and what the call owns is the
//! part of that tree that began with the call.
//!
//! A call that has burned no processor time for its stall is stopped: its process tree is signalled, the
//! program's tool comes back to the model as a failed result, and the turn goes on. A call that keeps
//! burning processor time — a build, a test, a search — is never touched, however long it runs. On a stop,
//! a call with no processor time behind it is stopped the same way, so the stop's words can be read at the
//! next boundary instead of waiting behind a blocked read.
//!
//! A call that holds no process of its own waits inside the program — on a socket, on a pipe — and leaves
//! nothing in the tree to read. Past a bound of its own it is cut too, but not by a signal: words are put
//! into the working turn saying the call did not complete, and the program ends the waiting call and hands
//! the model a failed result. A program that does not take the words ends the turn at the grace.
//!
//! The lines this writes go to the run's log and to the unit's own log, so a hang is readable while it is
//! happening: the call's start with its pid and a one-line summary, a still-running line past a threshold,
//! and its end with its duration and outcome.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::time::Duration;

use agent_host::identity::{ProcessStat, boot_ms, clock_ticks, process_stat};
use policy::tool_clock::CallClock;
use serde_json::Value;

/// How far before a call's record line a process may have begun and still be the call's own: the kernel's
/// start time is kept to the second, and the record is written a moment before the process starts.
const START_SLACK_MS: i64 = 3_000;

/// How long a stopped call's processes are given to end on their own before they are killed.
const TERM_GRACE: Duration = Duration::from_millis(250);

/// The numbers a call is held to, from the policy or a test's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolClock {
  pub min_ms: i64,
  pub stall_ms: i64,
  pub look_ms: i64,
  pub in_process_ms: i64,
  pub in_process_grace_ms: i64,
  pub in_process_cut: String,
  pub in_process_grace_cut: String,
  pub log_after_ms: i64,
  pub still_every_ms: i64,
  pub stop_stall_ms: i64,
  pub stop_bound_ms: i64,
}

impl From<&CallClock> for ToolClock {
  fn from(call: &CallClock) -> Self {
    let ms = |seconds: u64| i64::try_from(seconds).unwrap_or(i64::MAX / 1_000).saturating_mul(1_000);

    Self {
      min_ms: ms(call.min_seconds),
      stall_ms: ms(call.stall_seconds),
      look_ms: ms(call.look_seconds),
      in_process_ms: ms(call.in_process_seconds),
      in_process_grace_ms: ms(call.in_process_grace_seconds),
      in_process_cut: call.in_process_cut.clone(),
      in_process_grace_cut: call.in_process_grace_cut.clone(),
      log_after_ms: ms(call.log_after_seconds),
      still_every_ms: ms(call.still_every_seconds),
      stop_stall_ms: ms(call.stop_stall_seconds),
      stop_bound_ms: ms(call.stop_bound_seconds),
    }
  }
}

/// One tool call as the record names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolUse {
  pub id: String,
  pub name: String,
  pub input: Value,
  pub started_at: Option<i64>,
}

impl ToolUse {
  /// A one-line summary of what the call was given: the command a shell was handed, else the description
  /// or the path, flattened and cut.
  pub fn summary(&self) -> String {
    let field = |name: &str| self.input.get(name).and_then(Value::as_str).filter(|one| !one.is_empty());
    let said = field("command")
      .or_else(|| field("description"))
      .or_else(|| field("file_path"))
      .or_else(|| field("path"))
      .or_else(|| field("pattern"))
      .or_else(|| field("url"))
      .or_else(|| field("query"))
      .unwrap_or_default();
    let flat = said.split_whitespace().collect::<Vec<_>>().join(" ");

    flat.chars().take(160).collect()
  }

  /// What the call is called in a line: its tool's name, and the summary where it has one.
  pub fn words(&self) -> String {
    let after = self.summary();

    if after.is_empty() { self.name.clone() } else { format!("{} {}", self.name, after) }
  }
}

// A process as the call's own tree: its pid, parent, processor time and when it began.
#[derive(Clone, Debug)]
struct Proc {
  pid: i32,
  ppid: i32,
  cpu: u64,
  started_at: i64,
  cmdline: String,
}

// One call the watcher is holding.
#[derive(Clone, Debug, Default)]
struct Call {
  name: String,
  summary: String,
  started_at: i64,
  pids: Vec<i32>,
  cpu: HashMap<i32, u64>,
  last_progress_at: i64,
  started_said: bool,
  still_said_at: i64,
  cut: bool,
  // A call cut while it held no process of its own: its words are put into the turn, and a program that
  // does not take them ends the turn at the grace.
  in_process: bool,
}

impl Call {
  fn words(&self) -> String {
    if self.summary.is_empty() {
      self.name.clone()
    } else {
      format!("{} {}", self.name, self.summary)
    }
  }
}

/// A call cut while it had no process of its own: the words to put into the working turn, and what the
/// call was. A program that takes the words ends the waiting call and hands the model a failed result;
/// one that does not is ended at the grace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InProcessCut {
  pub tool: String,
  pub at: i64,
  pub seconds: u64,
  pub words: String,
}

/// A turn's tool calls watched over a turn.
#[derive(Debug)]
pub struct Watch {
  clock: ToolClock,
  calls: BTreeMap<String, Call>,
  last_look: i64,
  in_process: VecDeque<InProcessCut>,
}

impl Watch {
  pub fn new(clock: ToolClock) -> Self {
    Self {
      clock,
      calls: BTreeMap::new(),
      last_look: i64::MIN / 2,
      in_process: VecDeque::new(),
    }
  }

  /// A call just cut because it held no process of its own, whose words are yet to go into the turn.
  pub fn take_in_process_cut(&mut self) -> Option<InProcessCut> {
    self.in_process.pop_front()
  }

  /// Whether a call cut with no process of its own still stands unanswered, so the words did not end it.
  pub fn in_process_cut_still(&self) -> bool {
    self.calls.values().any(|call| call.in_process)
  }

  /// Watch the record once: what began, what ended, what is still going, and what has stopped making
  /// progress. `program` is the pid of the turn's program, whose tree the call's processes are part of.
  pub async fn look(
    &mut self,
    name: &str,
    records: &[Value],
    from: usize,
    program: i32,
    now: i64,
  ) -> Vec<String> {
    let uses = tool_uses(records, from);
    let results = tool_results(records, from);
    let mut lines = vec![];

    for used in &uses {
      if !self.calls.contains_key(&used.id) {
        self.calls.insert(
          used.id.clone(),
          Call {
            name: used.name.clone(),
            summary: used.summary(),
            started_at: used.started_at.unwrap_or(now),
            last_progress_at: now,
            ..Call::default()
          },
        );
      }
    }

    // A call the record now answers is over. It is read before the process scan so its own last look sees
    // it; the line is the duration it stood and what the answer said.
    let inflight: HashSet<&str> = uses.iter().map(|used| used.id.as_str()).collect();

    self.calls.retain(|id, call| {
      if inflight.contains(id.as_str()) {
        return true;
      }

      let outcome = match results.get(id) {
        Some(true) => "failed",
        Some(false) => "completed",
        None => "ended with the turn",
      };
      let seconds = (now - call.started_at).max(0) as f64 / 1_000.0;

      lines.push(format!(
        "{name}: tool {} ended pid={} after {seconds:.1}s: {outcome}",
        call.words(),
        call.pids.first().map_or_else(|| "-".to_owned(), i32::to_string),
      ));

      false
    });

    let scanning = now - self.last_look >= self.clock.look_ms;
    let procs = scanning.then(scan);

    if scanning {
      self.last_look = now;

      for used in &uses {
        let Some(call) = self.calls.get_mut(&used.id) else { continue };
        let Some(procs) = &procs else { continue };
        let owned = owned_by(procs, program, call.started_at, &used.name, &used.input);

        let mut progress = false;

        for owned in &owned {
          match call.cpu.get(&owned.pid) {
            Some(cpu) if owned.cpu > *cpu => progress = true,
            None => progress = true,
            _ => {}
          }

          call.cpu.insert(owned.pid, owned.cpu);
        }

        if progress {
          call.last_progress_at = now;
        }

        // The pids are kept once a call had them: a call cut or ended leaves its last pid for its end line.
        if !owned.is_empty() {
          call.pids = owned.iter().map(|owned| owned.pid).collect();
        }
      }
    }

    let mut cutting: Vec<Call> = vec![];

    for used in &uses {
      let Some(call) = self.calls.get_mut(&used.id) else { continue };
      let elapsed = now - call.started_at;

      // A call with no process yet may have one on its way, so its start waits a moment — no longer than
      // its own bound, since a call that never gets one is cut there.
      let waiting_for_process = call.pids.is_empty() && elapsed < 2_000.min(self.clock.in_process_ms);

      if !call.started_said && waiting_for_process {
        continue;
      }

      if !call.started_said {
        call.started_said = true;
        call.still_said_at = now;
        lines.push(start_line(name, &used.name, &call.summary, call.pids.first(), elapsed));
      }

      // A call with no process of its own has nothing in the tree to read, so it is held to a bound of its
      // own; one that owns a tree is cut when that tree burns no processor time.
      let in_process = call.pids.is_empty();
      let due = if in_process {
        elapsed >= self.clock.in_process_ms.max(self.clock.min_ms)
      } else {
        elapsed >= self.clock.min_ms && now - call.last_progress_at >= self.clock.stall_ms
      };

      if !call.cut && due {
        call.cut = true;
        call.in_process = in_process;

        if in_process {
          let seconds = u64::try_from(elapsed.max(0) / 1_000).unwrap_or_default();
          let words = policy::fill_marks(
            &self.clock.in_process_cut,
            &[("tool", &call.words()), ("seconds", &seconds.to_string())],
          );

          self.in_process.push_back(InProcessCut {
            tool: call.words(),
            at: now,
            seconds,
            words,
          });
          lines.push(format!(
            "{name}: tool {} cut after {:.0}s with no process of its own and no answer",
            call.words(),
            elapsed as f64 / 1_000.0,
          ));
        } else {
          cutting.push(call.clone());
          lines.push(format!(
            "{name}: tool {} cut after {:.0}s with no progress: pid={}",
            call.words(),
            elapsed as f64 / 1_000.0,
            call.pids.first().map_or_else(|| "-".to_owned(), i32::to_string),
          ));
        }
      } else if call.started_said
        && elapsed >= self.clock.log_after_ms
        && now - call.still_said_at >= self.clock.still_every_ms
      {
        call.still_said_at = now;
        lines.push(format!(
          "{name}: tool {} still running pid={} after {:.0}s: {}",
          call.name,
          call.pids.first().map_or_else(|| "-".to_owned(), i32::to_string),
          elapsed as f64 / 1_000.0,
          call.summary,
        ));
      }
    }

    for call in &cutting {
      stop(&call.pids).await;
    }

    lines
  }

  /// Stop the calls that are making no progress, so a stop's words can be read at the next boundary. A call
  /// that is still burning processor time is left for the stop's own bound.
  pub async fn cut_for_stop(
    &mut self,
    name: &str,
    records: &[Value],
    from: usize,
    program: i32,
    now: i64,
  ) -> Vec<String> {
    let uses = tool_uses(records, from);
    let procs = scan();
    let mut lines = vec![];

    for used in &uses {
      let Some(call) = self.calls.get_mut(&used.id) else { continue };

      if call.cut {
        continue;
      }

      let owned = owned_by(&procs, program, call.started_at, &used.name, &used.input);

      if !owned.is_empty() {
        call.pids = owned.iter().map(|owned| owned.pid).collect();
      }

      if call.pids.is_empty() {
        continue;
      }

      let elapsed = now - call.started_at;

      if elapsed < self.clock.stop_stall_ms || now - call.last_progress_at < self.clock.stop_stall_ms {
        continue;
      }

      call.cut = true;
      lines.push(format!(
        "{name}: tool {} cut for the stop after {:.0}s: pid={}",
        call.words(),
        elapsed as f64 / 1_000.0,
        call.pids.first().map_or_else(|| "-".to_owned(), i32::to_string),
      ));
      stop(&call.pids).await;
    }

    lines
  }
}

fn start_line(name: &str, tool: &str, summary: &str, pid: Option<&i32>, elapsed: i64) -> String {
  let pid = pid.map_or_else(|| "-".to_owned(), i32::to_string);
  let at = elapsed.max(0) as f64 / 1_000.0;

  if summary.is_empty() {
    format!("{name}: tool {tool} started pid={pid} after {at:.1}s")
  } else {
    format!("{name}: tool {tool} started pid={pid} after {at:.1}s: {summary}")
  }
}

// The processes of the program's tree that began with the call, and that the call's command names where it
// names one. A process older than the call is a part of the program's own standing tree, not the call's.
fn owned_by(procs: &[Proc], program: i32, started_at: i64, tool: &str, input: &Value) -> Vec<Proc> {
  let since = started_at.saturating_sub(START_SLACK_MS);
  let mut owned = descendants(procs, program)
    .into_iter()
    .filter(|proc| proc.started_at >= since)
    .collect::<Vec<_>>();

  if tool == "Bash"
    && let Some(command) = input.get("command").and_then(Value::as_str)
  {
    let named = owned
      .iter()
      .filter(|proc| names_command(&proc.cmdline, command))
      .map(|proc| proc.pid)
      .collect::<HashSet<_>>();

    if !named.is_empty() {
      let under = descendants_of(procs, &named);

      owned.retain(|proc| under.contains(&proc.pid));
    }
  }

  owned
}

fn names_command(cmdline: &str, command: &str) -> bool {
  command
    .split_whitespace()
    .filter(|token| token.len() > 1 && !token.starts_with('-') && !token.starts_with('|'))
    .any(|token| cmdline.contains(token))
}

fn descendants(procs: &[Proc], root: i32) -> Vec<Proc> {
  let children = |pid: i32| procs.iter().filter(move |proc| proc.ppid == pid);
  let mut found = vec![];
  let mut queue = vec![root];
  let mut seen = HashSet::new();

  while let Some(pid) = queue.pop() {
    for child in children(pid) {
      if seen.insert(child.pid) {
        found.push(child.clone());
        queue.push(child.pid);
      }
    }
  }

  found
}

fn descendants_of(procs: &[Proc], roots: &HashSet<i32>) -> HashSet<i32> {
  let mut found = roots.clone();
  let mut queue: Vec<i32> = roots.iter().copied().collect();

  while let Some(pid) = queue.pop() {
    for proc in procs.iter().filter(|proc| proc.ppid == pid) {
      if found.insert(proc.pid) {
        queue.push(proc.pid);
      }
    }
  }

  found
}

fn scan() -> Vec<Proc> {
  let Some(boot) = boot_ms() else { return vec![] };
  let ticks = clock_ticks().max(1);
  let Ok(entries) = std::fs::read_dir("/proc") else { return vec![] };

  entries
    .flatten()
    .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
    .filter_map(|pid| {
      let stat: ProcessStat = process_stat(pid)?;

      if stat.state.starts_with('Z') || pid == std::process::id() as i32 {
        return None;
      }

      let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok().map(|line| {
        line
          .split(|byte| *byte == 0)
          .filter(|arg| !arg.is_empty())
          .map(|arg| String::from_utf8_lossy(arg).into_owned())
          .collect::<Vec<_>>()
          .join(" ")
      })?;

      let birth = i64::try_from(stat.birth.parse::<u64>().ok()?).ok()?;

      Some(Proc {
        pid,
        ppid: stat.ppid,
        cpu: stat.cpu,
        started_at: boot.saturating_add(birth.saturating_mul(1_000) / ticks),
        cmdline,
      })
    })
    .collect()
}

// A call's processes stopped: a polite end first, then the kill for what a blocked read will not take.
async fn stop(pids: &[i32]) {
  for pid in pids {
    signal(*pid, libc::SIGTERM);
  }

  tokio::time::sleep(TERM_GRACE).await;

  for pid in pids {
    signal(*pid, libc::SIGKILL);
  }
}

fn signal(pid: i32, signal: i32) {
  if pid > 1 {
    // SAFETY: a signal sent to a pid the call's own tree named a moment ago.
    unsafe {
      libc::kill(pid, signal);
    }
  }
}

/// The tool calls the record holds after `from` that no result answers yet, in order.
pub fn tool_uses(records: &[Value], from: usize) -> Vec<ToolUse> {
  let answered: HashSet<String> = tool_results(records, from).into_keys().collect();
  let mut uses = vec![];

  for record in records.get(from..).unwrap_or_default() {
    if record.get("type").and_then(Value::as_str) != Some("assistant")
      || record.get("isSidechain").and_then(Value::as_bool) == Some(true)
    {
      continue;
    }

    let started_at = record
      .get("timestamp")
      .and_then(Value::as_str)
      .and_then(|stamp| chrono::DateTime::parse_from_rfc3339(stamp).ok())
      .map(|stamp| stamp.timestamp_millis());

    for block in blocks(record) {
      if block.get("type").and_then(Value::as_str) != Some("tool_use") {
        continue;
      }
      let Some(id) = block.get("id").and_then(Value::as_str) else { continue };

      if answered.contains(id) {
        continue;
      }

      uses.push(ToolUse {
        id: id.to_owned(),
        name: block.get("name").and_then(Value::as_str).unwrap_or("a tool").to_owned(),
        input: block.get("input").cloned().unwrap_or(Value::Null),
        started_at,
      });
    }
  }

  uses
}

// Every answer the record holds after `from`, by the call it answers, and whether it was an error.
fn tool_results(records: &[Value], from: usize) -> HashMap<String, bool> {
  let mut results = HashMap::new();

  for record in records.get(from..).unwrap_or_default() {
    for block in blocks(record) {
      if block.get("type").and_then(Value::as_str) != Some("tool_result") {
        continue;
      }
      let Some(id) = block.get("tool_use_id").and_then(Value::as_str) else { continue };

      results.insert(id.to_owned(), block.get("is_error").and_then(Value::as_bool).unwrap_or(false));
    }
  }

  results
}

fn blocks(record: &Value) -> &[Value] {
  record.pointer("/message/content").and_then(Value::as_array).map_or(&[], Vec::as_slice)
}
