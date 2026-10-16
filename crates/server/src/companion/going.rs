//! A companion's run going in a room (docs/api.md, 「Events」, `companion-run-going`): published as the run
//! starts and again as it ends, so a consumer that stands cancellations over a room's runs knows which are
//! going (docs/companions.md, 「What the reset cuts off」).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use serde::Serialize;

use crate::events::{Bus, Event, Subscription};

/// A run of the companion's own starting or ending in a room.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionRunGoing {
  pub agent: String,
  pub run: String,
  pub chat_id: i64,
  /// True while it runs, false once it has ended.
  pub going: bool,
}

impl Event for CompanionRunGoing {
  const NAME: &'static str = "companion-run-going";
}

/// A run named as going until this is dropped, which publishes its end.
pub struct Going {
  events: Bus,
  ended: CompanionRunGoing,
}

/// The run named as going from now: its start published, and its end once what this answers is dropped.
pub fn going(events: &Bus, agent: &str, run: &str, chat_id: i64) -> Going {
  let started = CompanionRunGoing {
    agent: agent.to_owned(),
    run: run.to_owned(),
    chat_id,
    going: true,
  };

  events.publish(started.clone());

  Going {
    events: events.clone(),
    ended: CompanionRunGoing {
      going: false,
      ..started
    },
  }
}

impl Drop for Going {
  fn drop(&mut self) {
    self.events.publish(self.ended.clone());
  }
}

/// The runs going for each companion, as `companion-run-going` names them: read off the one subscription
/// it holds, every event waiting taken in before a reading, so a reset stands its cancellations over the
/// runs going as it cuts the companion off.
pub struct GoingRuns {
  subscription: Mutex<Subscription<CompanionRunGoing>>,
  going: Mutex<BTreeMap<String, BTreeSet<String>>>,
}

impl GoingRuns {
  /// The runs going from now on, as the bus names them.
  pub fn on(events: &Bus) -> Self {
    Self {
      subscription: Mutex::new(events.subscribe()),
      going: Mutex::default(),
    }
  }

  /// The runs of a companion going now, in name order.
  pub fn of(&self, agent: &str) -> Vec<String> {
    let mut going = self.going.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut subscription = self.subscription.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    while let Some(event) = subscription.ready() {
      let runs = going.entry(event.agent).or_default();

      if event.going {
        runs.insert(event.run);
      } else {
        runs.remove(&event.run);
      }
    }

    going.get(agent).map(|runs| runs.iter().cloned().collect()).unwrap_or_default()
  }
}
