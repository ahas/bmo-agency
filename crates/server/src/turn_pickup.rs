//! A turn in flight across agency's own restart, picked up again (docs/sessions.md, 「A restart hands the
//! session over」): every turn sent is kept in the settings under its run while it goes, with what its end
//! needs to reach its caller — the session and the turn's id, the agent, the room, its run in the ledger and
//! its job. A server that starts reads each one left by the process before it against the record of the
//! agent's pane program: a turn whose end the record holds is handed back through the path a live turn's end
//! takes, a turn the program is still working on, or has not taken yet, is attached to and waited on with
//! the agent's program held, and a turn no record names is ended as lost.

use std::sync::Arc;
use std::time::Duration;

use agent_session::TurnEnd;
use policy::companion_runner::COMPANION_RUNNER;
use policy::resident_turn::RESIDENT_TURN;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use store::{all, one};

use crate::door::execution::{Row, publish_envelope};
use crate::door::inbox::{
  State, provider_named, read_turn_on, record_of, span_named, span_named_after, waiting_in_inbox,
};
use crate::door::place_run::end_run;
use crate::door::record::{Reply, UsageSpent};
use crate::herdr_backend::{RoadWalked, TurnEnded, close_chat_run, spent_of};
use crate::lines::place::Place;
use crate::places::Ground;
use crate::places::locks::{agent_lock_path, hold};
use crate::queue::settings;
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::worker_session::records::{note_answer, slot_account};
use crate::{Refusal, now_ms};

/// The settings family a turn in flight is kept under, by its run.
pub const PREFIX: &str = "turn_in_flight:";

/// What a turn's end needs to reach its caller after the process that sent it is gone.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InFlight {
  pub session: String,
  pub turn: i64,
  pub run: String,
  pub agent: String,
  #[serde(default)]
  pub chat_id: Option<i64>,
  /// The chat turn's run in the ledger; a job's run is the job's own.
  #[serde(default)]
  pub ledger: Option<i64>,
  #[serde(default)]
  pub job: Option<i64>,
  pub started_at: i64,
}

/// A turn kept while it goes; a store that would not take it is said and never fails the turn.
pub async fn keep(db: &sea_orm::DatabaseConnection, turn: &InFlight) {
  let text = serde_json::to_string(turn).unwrap_or_default();

  if let Err(why) = settings::set(db, &format!("{PREFIX}{}", turn.run), &text).await {
    eprintln!("{}: the turn in flight was not kept: {}", turn.run, why.detail);
  }
}

/// A turn whose end reached its caller, no longer kept.
pub async fn forget(db: &sea_orm::DatabaseConnection, run: &str) {
  let _ = settings::forget(db, &format!("{PREFIX}{run}")).await;
}

/// Whether a run's turn is kept in flight, which the pass ending a job left running leaves to this one.
pub async fn kept(db: &sea_orm::DatabaseConnection, run: &str) -> bool {
  settings::get(db, &format!("{PREFIX}{run}")).await.ok().flatten().is_some()
}

/// The turns kept in flight, as the store holds them.
pub async fn in_flight(db: &sea_orm::DatabaseConnection) -> Result<Vec<InFlight>, Refusal> {
  let mut turns = vec![];

  for run in settings::named(db, PREFIX).await? {
    if let Some(text) = settings::get(db, &format!("{PREFIX}{run}")).await? {
      match serde_json::from_str::<InFlight>(&text) {
        Ok(turn) => turns.push(turn),
        Err(_) => forget(db, &run).await,
      }
    }
  }

  Ok(turns)
}

/// How a picked-up turn came out: what the record holds of its end, or why it has none.
#[derive(Clone, Debug, PartialEq)]
pub enum Came {
  Answered(Box<Reply>),
  Failed(String),
}

/// Where a kept turn stands in its agent's pane program's record.
#[derive(Clone, Debug, PartialEq)]
pub enum Standing {
  /// Its end is in the record.
  Ended(Came),
  /// The program is working on it, or has it waiting in its inbox.
  Going,
  /// No record names it.
  Lost,
}

/// Where a kept turn stands, read from its agent's place.
pub fn standing(place: &Place, turn: &InFlight) -> Standing {
  let records = record_of(&place.path).1;
  let Some(span) = span_named(&records, &turn.run) else {
    return if waiting_in_inbox(&place.path, &turn.run) { Standing::Going } else { Standing::Lost };
  };

  standing_in(&records[span])
}

/// Where a turn put into a place under a name stands, the turns put in under that name before it being
/// `before`: read from the place's record where the program took it, going where it waits in the inbox, and
/// lost where neither holds it.
pub fn standing_after(place: &Place, name: &str, before: usize) -> Standing {
  let records = record_of(&place.path).1;
  let Some(span) = span_named_after(&records, name, before) else {
    let taken = (0..).take_while(|count| span_named_after(&records, name, *count).is_some()).count();

    return if taken == before && waiting_in_inbox(&place.path, name) {
      Standing::Going
    } else {
      Standing::Lost
    };
  };

  standing_in(&records[span])
}

// Where a turn stands, read from its lines of the record.
fn standing_in(lines: &[Value]) -> Standing {
  let provider = provider_named(lines).unwrap_or_default();
  let (state, error, reply) = read_turn_on(lines, "", &provider);

  match state {
    State::Working => Standing::Going,
    State::Interrupted => Standing::Ended(Came::Failed("the turn was interrupted".to_owned())),
    State::Ended => Standing::Ended(match error {
      Some(error) => Came::Failed(error),
      None if reply.text.is_empty() => Came::Failed("the pane program wrote no answer".to_owned()),
      None => Came::Answered(Box::new(reply)),
    }),
  }
}

// Why a turn no record names ended.
const LOST: &str = "agency restarted while the turn went, and no record its program keeps names it";
// Why a turn whose program went away while it was attached to ended.
const GONE: &str = "the pane program went away while the turn was picked up after agency restarted";

/// Every turn the process before this one left in flight, read at start: handed back where it ended, attached
/// to where it goes on, ended as lost where nothing names it. A job's lost turn is left to the pass that
/// ends a job left running.
pub async fn at_start(ground: &Arc<Ground>) -> Result<usize, Refusal> {
  let turns = in_flight(&ground.db).await?;
  let count = turns.len();

  for turn in turns {
    let place = Place::of(&ground.roots.data, &turn.agent);

    match standing(&place, &turn) {
      Standing::Ended(came) => hand_back(ground, &place, &turn, came).await,
      Standing::Lost if turn.job.is_some() => forget(&ground.db, &turn.run).await,
      Standing::Lost => hand_back(ground, &place, &turn, Came::Failed(LOST.to_owned())).await,
      Standing::Going => {
        let ground = ground.clone();

        tokio::spawn(async move { attach(&ground, place, turn).await });
      }
    }
  }

  Ok(count)
}

// A turn the program goes on with, waited on with the agent's program held, so no turn of this process
// goes into it meanwhile, and handed back once the record says it ended; a program gone is a turn gone.
async fn attach(ground: &Ground, place: Place, turn: InFlight) {
  let _held = hold(agent_lock_path(&ground.roots.data, &turn.agent)).await;
  let every = Duration::from_millis(RESIDENT_TURN.inbox_poll_ms);
  let mut unseen = 0u32;

  for row in open_rows(ground, &turn.run).await {
    publish_envelope(&ground.roots.data, row.id(), None);
  }

  loop {
    match standing(&place, &turn) {
      Standing::Ended(came) => return hand_back(ground, &place, &turn, came).await,
      Standing::Lost => return hand_back(ground, &place, &turn, Came::Failed(LOST.to_owned())).await,
      Standing::Going => {}
    }

    unseen = if place.program_alive().is_some() { 0 } else { unseen + 1 };

    if unseen >= 40 {
      return hand_back(ground, &place, &turn, Came::Failed(GONE.to_owned())).await;
    }

    tokio::time::sleep(every).await;
  }
}

/// The open execution rows a turn put in under a run left behind, kept as this process's own while it waits on
/// them, so no pass of the executions reads their collector as gone.
pub async fn own_rows(ground: &Ground, run: &str) {
  for row in open_rows(ground, run).await {
    publish_envelope(&ground.roots.data, row.id(), None);
  }
}

/// The open execution rows a turn put in under a run left behind, closed on how the turn came out.
pub async fn close_rows(ground: &Ground, run: &str, answered: bool) {
  for mut row in open_rows(ground, run).await {
    let state = if answered { "exited" } else { "cancelled" };

    publish_envelope(&ground.roots.data, row.id(), Some(state));
    row.set("state", json!(state));
    row.set("endedAt", json!(now_ms()));
    row.set("reconciliationEvidence", json!("picked up again after agency restarted"));

    if let Err(why) = row.save(&ground.db).await {
      eprintln!("{run}: the execution row was not closed: {}", why.detail);
    }
  }
}

// The open execution rows a turn's program put in under its run.
async fn open_rows(ground: &Ground, run: &str) -> Vec<Row> {
  all(
    &ground.db,
    r#"SELECT "body" FROM "agent_executions"
       WHERE ("body"::jsonb)->>'endedAt' IS NULL AND ("body"::jsonb)->'inboxTurn'->>'name' = $1"#,
    vec![run.into()],
  )
  .await
  .unwrap_or_default()
  .iter()
  .filter_map(|row| row.try_get::<String>("", "body").ok())
  .filter_map(|body| serde_json::from_str::<Map<String, Value>>(&body).ok())
  .map(|body| Row { body })
  .collect()
}

// How many of a turn's record lines its run log already holds: each progress and answer line is filed once,
// as the line it is, by whichever process read it.
async fn filed(ground: &Ground, turn: &InFlight) -> usize {
  one(
    &ground.db,
    r#"SELECT COUNT(*) AS "filed" FROM "agent_log"
       WHERE "agent" = $1 AND "run" = $2 AND "kind" IN ('progress', 'answer')"#,
    vec![turn.agent.clone().into(), turn.run.clone().into()],
  )
  .await
  .ok()
  .flatten()
  .and_then(|row| row.try_get::<i64>("", "filed").ok())
  .unwrap_or(0)
  .max(0) as usize
}

/// A kept turn's end handed back through the path a live turn's end takes: what of its record its run log
/// lacks filed there with the end, its execution row and its place's run closed, its road and spend
/// published for a chat turn and its run in the ledger closed, its end published as `turn.ended` under its
/// turn's id, a job ended with it, and the turn no longer kept.
pub async fn hand_back(ground: &Ground, place: &Place, turn: &InFlight, came: Came) {
  let db = &ground.db;
  let (written, records) = record_of(&place.path);
  let rows = open_rows(ground, &turn.run).await;
  let source = rows
    .first()
    .and_then(|row| row.text("provider").map(str::to_owned))
    .or_else(|| span_named(&records, &turn.run).and_then(|span| provider_named(&records[span])))
    .unwrap_or_default();
  let mut log = RunLog::new(Target {
    agent: turn.agent.clone(),
    run: turn.run.clone(),
    chat_id: turn.chat_id,
    job_id: turn.job,
  });

  if let Some(span) = span_named(&records, &turn.run) {
    let skip = filed(ground, turn).await;
    let mut seen = 0;
    let rest: Vec<String> = written[span.clone()]
      .iter()
      .zip(&records[span])
      .filter(|(_, record)| {
        let filed = matches!(record.get("kind").and_then(Value::as_str), Some("progress" | "answer"));

        if filed {
          seen += 1;
        }

        filed && seen > skip
      })
      .map(|(line, _)| line.clone())
      .collect();

    log.feed(db, &rest, &source).await;
  }

  let answered = matches!(came, Came::Answered(_));

  log.write(db, Line::note("The turn was picked up again after agency restarted.")).await;
  log
    .write(
      db,
      Line {
        kind: "end",
        text: match &came {
          Came::Answered(_) => COMPANION_RUNNER.log.answered.clone(),
          Came::Failed(why) => why.clone(),
        },
        state: Some(if answered { "succeeded" } else { "failed" }),
        ..Line::default()
      },
    )
    .await;

  close_rows(ground, &turn.run, answered).await;

  let _ = end_run(&place.path, now_ms());

  let (mut end, spent) = match &came {
    Came::Answered(reply) => (TurnEnd::answered(&turn.run, reply.text.clone()), spent_of(&reply.spend)),
    Came::Failed(why) => (TurnEnd::failed(&turn.run, why.clone(), None), None),
  };

  end.turn = turn.turn;

  if turn.job.is_none() {
    // A turn no program's record names walked no road anybody could read.
    if let Some(chat_id) = turn.chat_id.filter(|_| !source.is_empty()) {
      let reply = match &came {
        Came::Answered(reply) => Some(reply.as_ref()),
        Came::Failed(_) => None,
      };

      ground.events.publish(road_walked(chat_id, turn, &source, reply));
    }

    if let Came::Answered(reply) = &came {
      spend_told(ground, turn, reply).await;
    }

    close_chat_run(db, turn.ledger, &end, spent, None).await;
  }

  ground.events.publish(TurnEnded {
    session: turn.session.clone(),
    end: end.clone(),
  });

  if let Some(job) = turn.job {
    crate::control::work::job_turn_ended(ground, job, end).await;
  }

  forget(db, &turn.run).await;
}

// The road a picked-up chat turn walked, from its start to now.
fn road_walked(chat_id: i64, turn: &InFlight, provider: &str, reply: Option<&Reply>) -> RoadWalked {
  let spend = reply.map_or(&[][..], |reply| reply.spend.as_slice());
  let reported = !spend.is_empty();
  let sum = |pick: fn(&crate::door::record::ModelSpend) -> i64| {
    reported.then(|| spend.iter().map(pick).sum::<i64>())
  };

  RoadWalked {
    stage: COMPANION_RUNNER.turn_stage.clone(),
    chat_id,
    run: turn.run.clone(),
    provider: reply.map_or_else(|| provider.to_owned(), |reply| reply.mode.clone()),
    model: reply.and_then(|reply| reply.model.clone()).unwrap_or_default(),
    outcome: if reply.is_some() { "answered" } else { "error" },
    ms: now_ms() - turn.started_at,
    input_tokens: sum(|one| one.input_tokens),
    output_tokens: sum(|one| one.output_tokens),
    cache_read_tokens: sum(|one| one.cache_read_tokens),
    cache_write_tokens: sum(|one| one.cache_write_tokens),
    context_ratio: reply.and_then(|reply| reply.context_ratio),
  }
}

// What a picked-up chat turn spent, written and published as a live turn's is.
async fn spend_told(ground: &Ground, turn: &InFlight, reply: &Reply) {
  if let Err(why) =
    note_answer(&ground.db, &turn.agent, &reply.mode, reply.model.as_deref(), reply.effort.as_deref()).await
  {
    eprintln!("{}: its last answer was not written: {}", turn.agent, why.detail);
  }

  let account = slot_account(&reply.mode, reply.account.as_deref());

  if let Some(spent) =
    UsageSpent::of(&turn.agent, Some(&turn.run), now_ms() / 1000, &reply.spend, |_| account.clone())
  {
    ground.events.publish(spent);
  }
}
