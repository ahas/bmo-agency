//! The servers a worker's job left listening: what the sweep makes of each, and how an age reads.

use agency_server::places::servers::{ProcessRow, Verdict, WorkerPlan, minutes, plan};

fn process(pid: i32, group: i32, age_seconds: i64, place: Option<&str>) -> ProcessRow {
  ProcessRow {
    pid,
    group,
    age_seconds,
    kilobytes: 1024,
    place: place.map(str::to_owned),
  }
}

fn worker(name: &str, job_running: bool) -> WorkerPlan {
  WorkerPlan {
    name: name.into(),
    place: format!("/d/agent-places/{name}"),
    job_running,
  }
}

#[test]
fn a_group_is_one_server_judged_by_its_workers_job_its_age_and_who_shares_it() {
  let processes = [
    // A worker's leftover server, its group its own, listening twice.
    process(10, 10, 60, Some("/d/agent-places/idle")),
    process(11, 10, 60, Some("/d/agent-places/idle")),
    // A busy worker's server, young and old.
    process(20, 20, 60, Some("/d/agent-places/busy")),
    process(30, 30, 99_999, Some("/d/agent-places/busy")),
    // A server sharing its group with the worker's program, which names no place.
    process(40, 40, 60, Some("/d/agent-places/idle")),
    process(41, 40, 60, None),
    // A person's server names no place.
    process(50, 50, 60, None),
  ];
  let planned = plan(
    &processes,
    &[10, 11, 20, 30, 40, 50],
    &[worker("idle", false), worker("busy", true)],
    180.0 * 60_000.0,
  );

  assert_eq!(
    planned.iter().map(|server| (server.pid, server.verdict)).collect::<Vec<_>>(),
    [(10, Verdict::Ended), (20, Verdict::Kept), (30, Verdict::Expired), (40, Verdict::Shared)]
  );
  assert_eq!(planned[0].kilobytes, 2048);
}

#[test]
fn an_age_reads_in_hours_and_minutes() {
  assert_eq!(minutes(59), "0m");
  assert_eq!(minutes(3_720), "1h2m");
}
