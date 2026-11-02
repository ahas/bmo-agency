//! The servers a worker's job left listening (docs/worker-bench.md, 「Servers a job leaves listening」). A
//! worker's resident program stays up between jobs, so a `nuxi dev` one of its turns started lives on with
//! it unless something ends it; what ends it is its worker's job ending, never its age.
//!
//! A candidate is a process listening on a TCP port whose environment names a worker's agent place. The
//! worker's own dev server carries no `BMO_` variable and a person's shell names no place, so neither is
//! ever one.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use super::{AgentRow, Ground, locks};
use crate::Refusal;

// The variable a run's place is named in.
const PLACE_VARIABLE: &str = "BMO_AGENT_PLACE";

/// One process as the sweep reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessRow {
  pub pid: i32,
  pub group: i32,
  pub age_seconds: i64,
  /// Resident memory and swap together.
  pub kilobytes: i64,
  /// The agent place its environment names, as a whole path.
  pub place: Option<String>,
}

/// What the sweep makes of a server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
  /// No job of its worker runs.
  Ended,
  /// Its worker's job still runs.
  Kept,
  /// Due to end, but its group holds a process naming no place — the worker's program and its head —
  /// so it is left.
  Shared,
}

impl Verdict {
  fn word(self) -> &'static str {
    match self {
      Self::Ended => "ended",
      Self::Kept => "kept",
      Self::Shared => "shared",
    }
  }
}

/// A server a worker's job left listening, with what the sweep makes of it.
#[derive(Clone, Debug, PartialEq)]
pub struct JobServer {
  pub worker: String,
  pub pid: i32,
  pub group: i32,
  pub age_seconds: i64,
  pub kilobytes: i64,
  pub verdict: Verdict,
}

/// A worker whose place is read, with whether a job of it holds its checkout.
pub struct WorkerPlan {
  pub name: String,
  pub place: String,
  pub job_running: bool,
}

/// What the sweep makes of every candidate.
pub fn plan(processes: &[ProcessRow], listening: &[i32], workers: &[WorkerPlan]) -> Vec<JobServer> {
  let by_pid: HashMap<i32, &ProcessRow> = processes.iter().map(|row| (row.pid, row)).collect();
  let mut seen_groups = HashSet::new();
  let mut servers = vec![];

  for pid in listening {
    let Some(row) = by_pid.get(pid) else { continue };
    let Some(worker) = workers.iter().find(|worker| row.place.as_deref() == Some(worker.place.as_str()))
    else {
      continue;
    };

    // One server is one process group, however many of its processes listen.
    if !seen_groups.insert(row.group) {
      continue;
    }

    let members: Vec<&ProcessRow> = processes.iter().filter(|member| member.group == row.group).collect();
    let own_group = row.group > 1 && members.iter().all(|member| member.place == row.place);

    servers.push(JobServer {
      worker: worker.name.clone(),
      pid: *pid,
      group: row.group,
      age_seconds: row.age_seconds,
      kilobytes: members.iter().map(|member| member.kilobytes).sum(),
      verdict: if worker.job_running {
        Verdict::Kept
      } else if own_group {
        Verdict::Ended
      } else {
        Verdict::Shared
      },
    });
  }

  servers
}

fn place_of(pid: i32) -> Option<String> {
  let environ = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
  let prefix = format!("{PLACE_VARIABLE}=");

  environ
    .split(|byte| *byte == 0)
    .filter_map(|pair| std::str::from_utf8(pair).ok())
    .find_map(|pair| pair.strip_prefix(&prefix))
    .map(|place| place.trim_end_matches('/').to_owned())
    .filter(|place| !place.is_empty())
}

// What a process holds in memory and swap: a server long idle has most of what it took swapped out,
// which its resident size alone would hide.
fn held_kilobytes(pid: i32) -> i64 {
  let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
  let read = |name: &str| {
    status
      .lines()
      .find_map(|line| line.strip_prefix(name)?.strip_prefix(':'))
      .and_then(|value| value.split_whitespace().next()?.parse::<i64>().ok())
      .unwrap_or(0)
  };

  read("VmRSS") + read("VmSwap")
}

async fn columns(program: &str, args: &[&str]) -> String {
  tokio::process::Command::new(program)
    .args(args)
    .stdin(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .output()
    .await
    .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
    .unwrap_or_default()
}

/// Every process of the machine, as `ps` numbers them.
pub async fn processes() -> Vec<ProcessRow> {
  columns("ps", &["-e", "-o", "pid=,pgid=,etimes="])
    .await
    .lines()
    .filter_map(|line| {
      let mut numbers = line.split_whitespace().map(str::parse::<i64>);
      let (Some(Ok(pid)), Some(Ok(group)), Some(Ok(age))) =
        (numbers.next(), numbers.next(), numbers.next())
      else {
        return None;
      };
      let (pid, group) = (i32::try_from(pid).ok()?, i32::try_from(group).ok()?);

      Some(ProcessRow {
        pid,
        group,
        age_seconds: age,
        kilobytes: held_kilobytes(pid),
        place: place_of(pid),
      })
    })
    .collect()
}

/// Every process listening on a TCP port, as `lsof` names them.
async fn listening() -> Vec<i32> {
  let mut pids: Vec<i32> = columns("lsof", &["-nP", "-iTCP", "-sTCP:LISTEN", "-t"])
    .await
    .lines()
    .filter_map(|line| line.trim().parse().ok())
    .filter(|pid| *pid > 0)
    .collect();

  pids.sort_unstable();
  pids.dedup();
  pids
}

/// An age as a line reads it, in hours and minutes.
pub fn minutes(seconds: i64) -> String {
  if seconds >= 3600 {
    format!("{}h{}m", seconds / 3600, (seconds % 3600) / 60)
  } else {
    format!("{}m", seconds / 60)
  }
}

/// The line written for one server: its worker, checkout, age, memory and command, read while the
/// process can still say where it ran. Which job it was started in is the job service's to say, so the
/// line names none.
pub fn line(server: &JobServer) -> String {
  let cwd = std::fs::read_link(format!("/proc/{}/cwd", server.pid))
    .map_or_else(|_| "?".to_owned(), |cwd| cwd.to_string_lossy().into_owned());
  let command = std::fs::read(format!("/proc/{}/cmdline", server.pid))
    .map(|line| {
      line
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
    })
    .unwrap_or_default();

  format!(
    "job server of {} {}: pid {} group {}, in {cwd}, up {}, {} MB: {}",
    server.worker,
    server.verdict.word(),
    server.pid,
    server.group,
    minutes(server.age_seconds),
    (server.kilobytes as f64 / 1024.0).round(),
    command.chars().take(160).collect::<String>(),
  )
}

fn signal(group: i32, signal: i32) -> bool {
  // SAFETY: a signal to a process group by its number touches nothing of this process's memory.
  unsafe { libc::kill(-group, signal) == 0 }
}

// A group asked to end with TERM, waited on until it has gone however long its ending takes.
async fn end_group(group: i32) {
  signal(group, libc::SIGTERM);

  while signal(group, 0) {
    tokio::time::sleep(Duration::from_millis(100)).await;
  }
}

/// Every server a worker's job left listening, with what the sweep makes of it. Only the workers named are
/// read where some are; a job's runner sweeping its own worker as it ends reads that job as over, though
/// it still holds the hold.
pub async fn read(
  ground: &Ground,
  only: Option<&[String]>,
  ending: bool,
) -> Result<Vec<JobServer>, Refusal> {
  let workers: Vec<WorkerPlan> = AgentRow::workers(&ground.db)
    .await?
    .iter()
    .filter(|worker| only.is_none_or(|only| only.contains(&worker.name)))
    .map(|worker| WorkerPlan {
      name: worker.name.clone(),
      place: ground.roots.place(&worker.name).to_string_lossy().into_owned(),
      job_running: !ending
        && locks::held(&locks::job_hold_path(&ground.roots.data, &ground.checkout_of(worker))),
    })
    .collect();

  Ok(plan(&processes().await, &listening().await, &workers))
}

/// Every server due to end ended — its worker's job over — answering one line for
/// each server ended or left in a shared group. Only the workers named are read where some are; a job's
/// runner sweeping its own worker as it ends reads that job as over, though it still holds the hold.
pub async fn sweep(ground: &Ground, only: Option<&[String]>, ending: bool) -> Result<Vec<String>, Refusal> {
  let servers = read(ground, only, ending).await?;
  let mut lines = vec![];

  for server in servers.iter().filter(|server| server.verdict != Verdict::Kept) {
    lines.push(line(server));

    if server.verdict != Verdict::Shared {
      end_group(server.group).await;
    }
  }

  Ok(lines)
}
