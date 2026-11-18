//! The facts of a person's supply (docs/supply.md, 「The facts told」): `supply.changed` published once as a run
//! moves to another source or finds none left, and the one order told, through the outbox, to the supervisor in
//! the room the run answers. The owner is not told.

use policy::fill_marks;
use policy::supply::SUPPLY;
use serde::Serialize;
use serde_json::json;

use super::Closed;
use super::meter::Metered;
use crate::events::Event;
use crate::outbox::{Keep, Notice};
use crate::places::Ground;

/// A run moved to another source.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplyMoved {
  pub fact: &'static str,
  pub run: String,
  pub account: Option<i64>,
  pub job: Option<i64>,
  pub supervisor: Option<String>,
  pub provider: String,
  pub model: String,
  pub from: String,
  pub to: String,
  pub reason: String,
}

impl Event for SupplyMoved {
  const NAME: &'static str = "supply.changed";
}

/// A run found no source left.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplyClosed {
  pub fact: &'static str,
  pub run: String,
  pub account: Option<i64>,
  pub job: Option<i64>,
  pub supervisor: Option<String>,
  pub provider: String,
  pub model: String,
  pub source: String,
  pub reason: String,
  pub reopens_at: Option<i64>,
  pub sentence: String,
}

impl Event for SupplyClosed {
  const NAME: &'static str = "supply.changed";
}

/// Who a run's facts are told to: the supervisor it answers for and the room.
#[derive(Clone, Debug, Default)]
pub struct Told {
  pub supervisor: Option<String>,
  pub room: Option<i64>,
}

// The one order to the person's own supervisor in the run's room, owed in the outbox under the run and the fact.
async fn order(ground: &Ground, told: &Told, run: &str, fact: &str, said: String) {
  let (Some(supervisor), Some(room)) = (&told.supervisor, told.room) else {
    return;
  };
  let body = json!({
    "agentName": supervisor,
    "room": room,
    "said": { "fact": said },
    "sort": "notice",
    "kind": format!("supply:{fact}:{run}"),
    "summary": said,
  });

  if let Err(why) =
    crate::outbox::owe(&ground.db, ground.vars(), Notice::instruction(body, Keep::Once)).await
  {
    eprintln!("agency: the supply notice of {run}: {why}");
  }
}

/// A run moved from one source to another, published and told once.
pub async fn moved(ground: &Ground, metered: &Metered, from: &str, to: &str, why: &str) {
  let chosen = &metered.chosen;
  let told = Told {
    supervisor: supervisor_of(ground, &metered.agent).await,
    room: metered.room,
  };

  moved_on(ground, chosen, metered.job, &told, from, to, why).await;
}

/// The same, for a run named by what it chose and who it is told to.
pub async fn moved_on(
  ground: &Ground,
  chosen: &super::Chosen,
  job: Option<i64>,
  told: &Told,
  from: &str,
  to: &str,
  why: &str,
) {
  ground.events.publish(SupplyMoved {
    fact: "supply-moved",
    run: chosen.run.clone(),
    account: chosen.account,
    job,
    supervisor: told.supervisor.clone(),
    provider: chosen.provider.clone(),
    model: chosen.model.clone(),
    from: from.to_owned(),
    to: to.to_owned(),
    reason: why.to_owned(),
  });

  let said = fill_marks(&SUPPLY.words.moved_told, &[("from", from), ("to", to), ("why", why)]);

  order(ground, told, &chosen.run, "supply-moved", said).await;
}

/// A run that found no source left, published and told once.
#[allow(clippy::too_many_arguments)]
pub async fn closed(
  ground: &Ground,
  run: &str,
  account: Option<i64>,
  job: Option<i64>,
  provider: &str,
  model: &str,
  told: &Told,
  closed: &Closed,
) {
  ground.events.publish(SupplyClosed {
    fact: "supply-closed",
    run: run.to_owned(),
    account,
    job,
    supervisor: told.supervisor.clone(),
    provider: provider.to_owned(),
    model: model.to_owned(),
    source: closed.source.clone(),
    reason: closed.reason.clone(),
    reopens_at: closed.reopens_at,
    sentence: closed.sentence.clone(),
  });
  order(ground, told, run, "supply-closed", closed.sentence.clone()).await;
}

/// A person's own key or login the provider refused, told to their own supervisor room once.
pub async fn refused(ground: &Ground, told: &Told, run: &str, source: &str, why: &str) {
  let said = fill_marks(&SUPPLY.words.refused_told, &[("source", source), ("why", why)]);

  order(ground, told, run, "supply-refused", said).await;
}

/// The supervisor an agent's runs answer for: the agent itself where it is a supervisor.
pub async fn supervisor_of(ground: &Ground, agent: &str) -> Option<String> {
  crate::supervisors::named(&ground.db, agent)
    .await
    .ok()
    .flatten()
    .map(|supervisor| supervisor.name)
}
