//! The pressure the node service tells on this machine, published again with the job behind each place,
//! which only agency knows (docs/agent-control.md, 「Pressure told as a fact」; docs/api.md, 「Events」).

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use policy::machine_room::MACHINE_ROOM;
use serde::Serialize;
use serde_json::Value;

use super::machine::{AtWork, at_work, counted};
use crate::Refusal;
use crate::events::Event;
use crate::peers::PeerFault;
use crate::peers::node::{PlaceHolding, Pressure, Standing};
use crate::places::Ground;

/// A pressure on this machine, as agency publishes it for the consumer that tells the chat agent.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachinePressure {
  #[serde(flatten)]
  pub told: Told,
  pub reading: Option<Value>,
  pub holders: Vec<Holder>,
  pub largest: Vec<Largest>,
  pub jobs: u64,
  pub browser_jobs: u64,
}

impl Event for MachinePressure {
  const NAME: &'static str = "machine-pressure";
}

/// What the pressure is, with its figures.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "cause", rename_all = "lowercase")]
pub enum Told {
  #[serde(rename_all = "camelCase")]
  Memory {
    full_pct: f64,
    alert_pct: f64,
    minutes: f64,
  },
  #[serde(rename_all = "camelCase")]
  Disk { free_gb: f64, limit_gb: f64 },
}

/// What one place holds, with the worker at work in it and the job and item its turn named.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Holder {
  pub place: String,
  pub megabytes: f64,
  pub browsers: u64,
  pub worker: Option<String>,
  pub job_id: Option<i64>,
  pub todo_id: Option<i64>,
}

/// One of the largest processes, with the job of the place it stands in.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Largest {
  pub pid: i64,
  pub name: String,
  pub megabytes: f64,
  pub place: Option<String>,
  pub job_id: Option<i64>,
}

fn worker_in<'a>(at_work: &'a [AtWork], place: Option<&str>) -> Option<&'a AtWork> {
  place.and_then(|place| at_work.iter().find(|worker| worker.worker == place))
}

/// The pressure told, each place named by the worker at work in it and its job; `standing` is what the
/// jobs with a browser are counted from where the pressure carries no places of its own.
pub fn named(pressure: Pressure, at_work: &[AtWork], standing: &Standing) -> MachinePressure {
  let (told, reading, places, largest) = match pressure {
    Pressure::Memory {
      full_pct,
      alert_pct,
      minutes,
      reading,
      places,
      largest,
    } => (
      Told::Memory {
        full_pct,
        alert_pct,
        minutes,
      },
      reading,
      places,
      largest,
    ),
    Pressure::Disk {
      free_gb,
      limit_gb,
      reading,
    } => (Told::Disk { free_gb, limit_gb }, reading, vec![], vec![]),
  };
  let counted_on = if places.is_empty() { standing.places.clone() } else { places.clone() };
  let (jobs, browser_jobs) = counted(
    at_work,
    &Standing {
      browsers: standing.browsers,
      places: counted_on,
    },
  );
  let holders = places
    .into_iter()
    .map(
      |PlaceHolding {
         place,
         megabytes,
         browsers,
       }| {
        let worker = worker_in(at_work, Some(&place));

        Holder {
          worker: worker.map(|worker| worker.worker.clone()),
          job_id: worker.and_then(|worker| worker.job).map(|job| job.job),
          todo_id: worker.and_then(|worker| worker.job).and_then(|job| job.item),
          place,
          megabytes,
          browsers,
        }
      },
    )
    .collect();
  let largest = largest
    .into_iter()
    .map(|process| Largest {
      job_id: worker_in(at_work, process.place.as_deref())
        .and_then(|worker| worker.job)
        .map(|job| job.job),
      pid: process.pid,
      name: process.name,
      megabytes: process.megabytes,
      place: process.place,
    })
    .collect();

  MachinePressure {
    told,
    reading,
    holders,
    largest,
    jobs,
    browser_jobs,
  }
}

// One pressure published with its jobs named; the standing is asked only where the pressure carries no
// places.
async fn tell(ground: &Ground, pressure: Pressure) -> Result<(), Refusal> {
  let standing = match &pressure {
    Pressure::Memory { .. } => Standing::default(),
    Pressure::Disk { .. } => ground.peers.node.machine_standing().await.unwrap_or_default(),
  };

  ground.events.publish(named(pressure, &at_work(&ground.db).await?, &standing));

  Ok(())
}

/// The node service's pressures listened to for as long as the server runs, the stream opened again a
/// while after it drops or cannot be opened; a stream that cannot be opened is said once until it opens.
pub async fn relay(ground: Arc<Ground>) {
  let reopen = Duration::from_secs(MACHINE_ROOM.clocks.pressure_reopen_seconds);
  let mut said_closed = false;

  loop {
    match ground.peers.node.pressures().await {
      Ok(mut told) => {
        said_closed = false;

        while let Some(pressure) = told.next().await {
          match pressure {
            Ok(pressure) => {
              if let Err(refusal) = tell(&ground, pressure).await {
                eprintln!("agency: a pressure on the machine could not be told: {}", refusal.detail);
              }
            }
            Err(fault @ PeerFault::Malformed { .. }) => eprintln!("agency: {fault}"),
            Err(fault) => {
              eprintln!("agency: the node service's pressures dropped: {fault}");
              break;
            }
          }
        }
      }
      Err(fault) if !said_closed => {
        said_closed = true;
        eprintln!("agency: the node service's pressures could not be opened: {fault}");
      }
      Err(_) => {}
    }

    tokio::time::sleep(reopen).await;
  }
}
