//! A pressure the node service tells, published again with the worker at work in each place and the job
//! and item its turn named.

use agency_server::peers::node::{LargeProcess, PlaceHolding, Pressure, Standing};
use agency_server::watches::machine::AtWork;
use agency_server::watches::pressure::named;
use agent_session::JobMark;
use serde_json::json;

fn place(place: &str, megabytes: f64, browsers: u64) -> PlaceHolding {
  PlaceHolding {
    place: place.into(),
    megabytes,
    browsers,
  }
}

fn at_work() -> Vec<AtWork> {
  vec![
    AtWork {
      worker: "w1".into(),
      job: Some(JobMark {
        job: 41,
        item: Some(1802),
      }),
    },
    AtWork {
      worker: "w2".into(),
      job: None,
    },
  ]
}

#[test]
fn a_memory_pressure_names_the_job_behind_each_place_and_counts_the_jobs() {
  let pressure = Pressure::Memory {
    full_pct: 12.5,
    alert_pct: 10.0,
    minutes: 2.0,
    reading: Some(json!({ "load1": 30 })),
    places: vec![place("w1", 900.5, 2), place("chat", 400.0, 0), place("w2", 100.0, 0)],
    largest: vec![
      LargeProcess {
        pid: 7,
        name: "chrome".into(),
        megabytes: 400.0,
        place: Some("w1".into()),
      },
      LargeProcess {
        pid: 9,
        name: "postgres".into(),
        megabytes: 300.0,
        place: None,
      },
    ],
  };

  assert_eq!(
    json!(named(pressure, &at_work(), &Standing::default())),
    json!({
      "cause": "memory",
      "fullPct": 12.5,
      "alertPct": 10.0,
      "minutes": 2.0,
      "reading": { "load1": 30 },
      "holders": [
        { "place": "w1", "megabytes": 900.5, "browsers": 2, "worker": "w1", "jobId": 41, "todoId": 1802 },
        { "place": "chat", "megabytes": 400.0, "browsers": 0, "worker": null, "jobId": null, "todoId": null },
        { "place": "w2", "megabytes": 100.0, "browsers": 0, "worker": "w2", "jobId": null, "todoId": null }
      ],
      "largest": [
        { "pid": 7, "name": "chrome", "megabytes": 400.0, "place": "w1", "jobId": 41 },
        { "pid": 9, "name": "postgres", "megabytes": 300.0, "place": null, "jobId": null }
      ],
      "jobs": 2,
      "browserJobs": 1
    })
  );
}

#[test]
fn a_disk_pressure_carries_no_places_and_counts_the_jobs_with_a_browser_from_the_standing() {
  let pressure = Pressure::Disk {
    free_gb: 12.0,
    limit_gb: 20.0,
    reading: None,
  };
  let standing = Standing {
    browsers: 4,
    places: vec![place("w1", 900.0, 1), place("w2", 300.0, 3)],
  };

  assert_eq!(
    json!(named(pressure, &at_work(), &standing)),
    json!({
      "cause": "disk",
      "freeGb": 12.0,
      "limitGb": 20.0,
      "reading": null,
      "holders": [],
      "largest": [],
      "jobs": 2,
      "browserJobs": 2
    })
  );
}
