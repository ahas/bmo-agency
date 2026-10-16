//! A companion's run going in a room (docs/api.md, 「Events」, `companion-run-going`): published as the run
//! starts and again as it ends, so a consumer that stands cancellations over a room's runs knows which are
//! going (docs/companions.md, 「What the reset cuts off」).

use serde::Serialize;

use crate::events::{Bus, Event};

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
