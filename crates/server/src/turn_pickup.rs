//! A turn in flight across agency's own restart, picked up again (docs/sessions.md, 「A restart hands the
//! session over」): every turn sent is kept in the settings under its run while it goes, with what its end
//! needs to reach its caller — the session and the turn's id, the agent, the room, its run in the ledger and
//! its job. A server that starts reads each one left by the process before it in the place of the pane
//! program that carries it, the agent's or that of the thrown-away session it went on, as that program
//! writes its turns down: a dsh pane's in its record and inbox, a Claude Code pane's in the place's run,
//! the hooks' calls and the session's transcript. A turn whose end the program wrote is handed back through
//! the path a live turn's end takes, a turn the program is still working on, or has not taken yet, is
//! attached to and waited on with the program held, its thrown-away session's place kept until its end is
//! handed back and ended then, and a turn nothing names is ended as lost.

use std::sync::Arc;
use std::time::Duration;

use agent_session::{SessionId, TurnEnd};
use policy::resident_turn::RESIDENT_TURN;
use policy::supervisor_runner::SUPERVISOR_RUNNER;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use store::{all, one};

use crate::door::execution::{Row, cancelled, publish_envelope};
use crate::door::inbox::{
  State, provider_named, read_turn_on, record_of, span_named, span_named_after, waiting_in_inbox,
};
use crate::door::place_run::{end_run, latest_failure, read_hooks, read_run, transcript_since};
use crate::door::record::{
  Reply, RoundState, UsageSpent, answered_since, read_round, read_turn, turn_closed_since, turn_error,
};
use crate::herdr_backend::{RoadWalked, TurnEnded, close_chat_run, spent_of};
use crate::lines::place::Place;
use crate::places::Ground;
use crate::places::locks::{agent_lock_path, hold};
use crate::queue::settings;
use crate::queue::stops::Stops;
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

/// A turn no longer kept, the run and why written to the log: its end reached its caller, was handed back,
/// or nothing names it any more (docs/sessions.md, 「A restart hands the session over」).
pub async fn forget(db: &sea_orm::DatabaseConnection, run: &str, why: &str) {
  match settings::forget(db, &format!("{PREFIX}{run}")).await {
    Ok(()) => println!("{run}: the turn is no longer kept in flight: {why}"),
    Err(refusal) => eprintln!("{run}: the turn kept in flight was not let go ({why}): {}", refusal.detail),
  }
}

// The runs of the turns this process carries now.
static CARRIED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
  std::sync::LazyLock::new(Default::default);

fn carried() -> std::sync::MutexGuard<'static, std::collections::HashSet<String>> {
  CARRIED.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A turn this process carries, marked from its start until the mark goes with its end: its own end closes
/// it, so the queue's pass leaves it alone, though its place's run ends between the roads its walk goes out
/// on (docs/sessions.md, 「A turn whose program dies」).
pub struct Carrying(String);

impl Carrying {
  pub fn of(run: &str) -> Self {
    carried().insert(run.to_owned());

    Self(run.to_owned())
  }
}

impl Drop for Carrying {
  fn drop(&mut self) {
    carried().remove(&self.0);
  }
}

/// Whether this process carries a turn under a run now.
pub fn carried_here(run: &str) -> bool {
  carried().contains(run)
}

/// Whether a run's turn is kept in flight, which the pass ending a job left running leaves to this one.
pub async fn kept(db: &sea_orm::DatabaseConnection, run: &str) -> bool {
  settings::get(db, &format!("{PREFIX}{run}")).await.ok().flatten().is_some()
}

/// Whether a turn of an agent's is kept in flight, its program carrying it whoever waits on it.
pub async fn carried_for(db: &sea_orm::DatabaseConnection, agent: &str) -> bool {
  in_flight(db).await.is_ok_and(|turns| turns.iter().any(|turn| turn.agent == agent))
}

/// The turns kept in flight, as the store holds them.
pub async fn in_flight(db: &sea_orm::DatabaseConnection) -> Result<Vec<InFlight>, Refusal> {
  let mut turns = vec![];

  for run in settings::named(db, PREFIX).await? {
    if let Some(text) = settings::get(db, &format!("{PREFIX}{run}")).await? {
      match serde_json::from_str::<InFlight>(&text) {
        Ok(turn) => turns.push(turn),
        Err(_) => forget(db, &run, "its kept row does not read").await,
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

/// The name of the place whose program carries a kept turn: the thrown-away session's own place where the
/// turn went on one, a job's turn among them (docs/api.md, 「sessions」), the agent's otherwise.
pub fn place_name_of(turn: &InFlight) -> String {
  let session = SessionId(turn.session.clone());

  if crate::herdr_backend::is_apart(&session) {
    crate::apart_places::place_name(&session)
  } else {
    turn.agent.clone()
  }
}

/// The names of the places the turns kept in flight are carried in, which the sweep of thrown-away
/// sessions' places at start keeps: the turns the pickup adopted, each place ended once its turn's end is
/// handed back.
pub async fn carrying_places(db: &sea_orm::DatabaseConnection) -> Vec<String> {
  in_flight(db).await.unwrap_or_default().iter().map(place_name_of).collect()
}

/// Where a kept turn stands, read from the place its program carries it in, as that program writes its
/// turns down: a dsh pane's in its record and its inbox, a Claude Code pane's in the place's run and the
/// hooks' calls (docs/sessions.md, 「A restart hands the session over」).
pub fn standing(place: &Place, turn: &InFlight) -> Standing {
  let records = record_of(&place.path).1;
  let Some(span) = span_named(&records, &turn.run) else {
    if waiting_in_inbox(&place.path, &turn.run) {
      return Standing::Going;
    }

    return standing_on_hooks(place, turn);
  };

  standing_in(&records[span])
}

// Where a turn of a Claude Code pane stands, which keeps no record of agency's but the place's run, its
// hooks' calls and the session's own transcript: going while the place's run is the turn's, the program
// took it — a submit since the run began — and no end has closed it; ended once the hooks have closed the
// run, its answer or its error read from the transcript the hooks name, from the run's start; lost where
// the run is another's or the program never took it.
fn standing_on_hooks(place: &Place, turn: &InFlight) -> Standing {
  let Some(run) =
    read_run(&place.path).filter(|run| !run.person && run.name.as_deref() == Some(turn.run.as_str()))
  else {
    return Standing::Lost;
  };
  let hooks = read_hooks(&place.path, run.started_at);

  if !hooks.iter().any(|hook| hook.kind == "submit") {
    return Standing::Lost;
  }

  if run.ended_at.is_none() {
    return Standing::Going;
  }

  // The hooks of the run's session name its transcript, the latest naming it the one it ended in.
  let named = |key: &str| {
    hooks
      .iter()
      .rev()
      .find_map(|hook| hook.payload.get(key).and_then(Value::as_str).map(str::to_owned))
  };
  let lines = named("transcript_path")
    .and_then(|transcript| transcript_since(std::path::Path::new(&transcript), run.started_at))
    .unwrap_or_default();
  let failure = latest_failure(&place.path, run.started_at).map(|failure| failure.failure());

  Standing::Ended(came_of(&lines, &named("session_id").unwrap_or_default(), failure))
}

// How a Claude Code turn came out, read from its transcript's lines since it went in: its error, the
// failure its place recorded, no answer, or its round's answer.
fn came_of(lines: &[Value], session: &str, failure: Option<(String, Option<String>)>) -> Came {
  let read = read_turn(lines, 0);
  let answered = answered_since(lines, 0);

  if let Some((error, _)) = turn_error(&read, answered, failure) {
    return Came::Failed(error);
  }

  if !answered {
    return Came::Failed(NO_ANSWER.to_owned());
  }

  let mut reply = read.reply(session, read.spend.clone());
  let round = read_round(lines).text;

  if !round.is_empty() {
    reply.text = round;
  }

  Came::Answered(Box::new(reply))
}

/// A turn's pane of its own, which no place stands for — a role's turn on Claude Code's own provider, a
/// job's among them: the execution its run opened, the pane and the program its liveness row names, the
/// session's transcript it reads, and when the execution began.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnPane {
  pub execution: String,
  pub pane: Option<String>,
  pub transcript: std::path::PathBuf,
  pub session: String,
  pub program: Option<crate::door::liveness::Program>,
  pub started_at: i64,
}

/// The pane of its own a kept turn went out in, found through the open execution its run opened and that
/// execution's liveness row; none where the run opened no such execution. The program is the one the row
/// names, else the process whose command line carries the session.
pub async fn own_pane(db: &sea_orm::DatabaseConnection, run: &str) -> Option<OwnPane> {
  let row = one(
    db,
    r#"SELECT l."execution", l."pane", l."session", l."records", l."program_pid", l."program_birth",
              e."body"
       FROM "turn_liveness" l JOIN "agent_executions" e ON e."id" = l."execution"
       WHERE l."run" = $1 AND (e."body"::jsonb)->>'endedAt' IS NULL
         AND COALESCE((e."body"::jsonb)->'resident', 'null'::jsonb) = 'null'::jsonb
       ORDER BY l."opened_at" DESC LIMIT 1"#,
    vec![run.into()],
  )
  .await
  .ok()
  .flatten()?;
  let text = |column: &str| row.try_get::<Option<String>>("", column).ok().flatten();
  let records: Vec<String> = text("records").and_then(|records| serde_json::from_str(&records).ok())?;
  let body: Value = text("body").and_then(|body| serde_json::from_str(&body).ok())?;
  let session = text("session").unwrap_or_default();
  let pid = row.try_get::<Option<i64>>("", "program_pid").ok().flatten();
  let program = match pid.and_then(|pid| i32::try_from(pid).ok()) {
    Some(pid) => Some(crate::door::liveness::Program {
      pid,
      birth: text("program_birth"),
    }),
    None => crate::door::liveness::program_of_session(&session),
  };

  Some(OwnPane {
    execution: text("execution")?,
    pane: text("pane"),
    transcript: records.first().map(std::path::PathBuf::from)?,
    session,
    program,
    started_at: body.get("startedAt").and_then(Value::as_i64).unwrap_or_default(),
  })
}

/// Where a turn in a pane of its own stands, read from its session's transcript since its execution began:
/// going while the round there is still working and its program stands; ended once the round has ended,
/// its answer or its error read from it; ended failed where the program is gone with the round open.
pub fn standing_in_pane(pane: &OwnPane) -> Standing {
  let lines = transcript_since(&pane.transcript, pane.started_at).unwrap_or_default();
  let round = read_round(&lines);

  match round.state {
    RoundState::Interrupted => Standing::Ended(Came::Failed("the turn was interrupted".to_owned())),
    RoundState::Ended if turn_closed_since(&lines, 0) => {
      Standing::Ended(came_of(&lines, &pane.session, None))
    }
    _ if pane.program.as_ref().is_some_and(crate::door::liveness::alive_here) => Standing::Going,
    _ => Standing::Ended(Came::Failed(GONE.to_owned())),
  }
}

// Why a turn whose end its program wrote with no answer ended.
const NO_ANSWER: &str = "the pane program wrote no answer";

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
      None if reply.text.is_empty() => Came::Failed(NO_ANSWER.to_owned()),
      None => Came::Answered(Box::new(reply)),
    }),
  }
}

// Why a turn no record names ended.
const LOST: &str = "agency restarted while the turn went, and no record its program keeps names it";
// Why a turn whose program went away while it was attached to ended.
const GONE: &str = "the pane program went away while the turn was picked up after agency restarted";

/// Every turn the process before this one left in flight, read at start from the place its program carries
/// it in: handed back where it ended, attached to where it goes on, the lease bound to its run then standing
/// as seen and its open execution rows kept as this process's own, ended as lost where nothing names it. A
/// job's lost turn is left to the pass that ends a job left running.
pub async fn at_start(ground: &Arc<Ground>) -> Result<usize, Refusal> {
  let turns = in_flight(&ground.db).await?;
  let count = turns.len();

  for turn in turns {
    let name = place_name_of(&turn);
    let place = Place::of(&ground.roots.data, &name);
    let mut now = standing(&place, &turn);
    // A turn no place names may have gone out in a pane of its own.
    let pane = match now {
      Standing::Lost => own_pane(&ground.db, &turn.run).await,
      _ => None,
    };

    if let Some(pane) = &pane {
      now = standing_in_pane(pane);
    }

    match now {
      Standing::Ended(came) => {
        hand_back(ground, &place, &turn, came).await;
        close_pane(ground, pane.as_ref()).await;
      }
      Standing::Lost if turn.job.is_some() => {
        forget(&ground.db, &turn.run, "nothing names it at start; its job is left to the reap").await
      }
      Standing::Lost => hand_back(ground, &place, &turn, Came::Failed(LOST.to_owned())).await,
      Standing::Going => {
        // The adopted turn stands as seen: the lease bound to its run is given back by its end, or by its
        // program found gone, and never waits on a sight of it this process would not have.
        if let Err(why) = crate::order_run::dispatch::seen_standing(&ground.db, &turn.run).await {
          eprintln!("{}: the lease bound to the adopted turn was not marked seen: {why}", turn.run);
        }

        // Its rows are this process's from now, before any pass of the executions reads their collector.
        own_rows(ground, &turn.run).await;

        let ground = ground.clone();

        tokio::spawn(async move { attach(&ground, &name, place, pane, turn).await });
      }
    }
  }

  Ok(count)
}

// A turn the program goes on with, waited on with its place's program held, so no turn of this process
// goes into it meanwhile, and handed back once the record says it ended; a program gone is a turn gone. A
// cancellation of its run cuts it short with its surface's interrupt, as the door carrying a live turn
// does, and the record's end of it is handed back as any other. A thrown-away session's place it was
// carried in is ended once the end is handed back, its session ending with its turn, and a pane of the
// turn's own is closed, as the door closes it with its run.
async fn attach(ground: &Ground, name: &str, place: Place, pane: Option<OwnPane>, turn: InFlight) {
  {
    let _held = hold(agent_lock_path(&ground.roots.data, name)).await;

    waited(ground, &place, pane.as_ref(), &turn).await;
  }

  close_pane(ground, pane.as_ref()).await;

  if crate::apart_places::is_apart_place(&ground.roots.data, name) {
    crate::apart_places::end(ground, name).await;
  }
}

// A pane of the turn's own closed once its end is handed back.
async fn close_pane(ground: &Ground, pane: Option<&OwnPane>) {
  if let (Some(herdr), Some(pane)) = (&ground.herdr, pane.and_then(|pane| pane.pane.as_deref())) {
    let _ = herdr.call("pane.close", json!({ "pane_id": pane }), None).await;
  }
}

// An adopted turn waited on until its end is handed back: in its place, or in its pane of its own.
async fn waited(ground: &Ground, place: &Place, pane: Option<&OwnPane>, turn: &InFlight) {
  let every = Duration::from_millis(RESIDENT_TURN.inbox_poll_ms);
  let mut unseen = 0u32;
  let mut cut = false;

  own_rows(ground, &turn.run).await;

  loop {
    let now = match pane {
      Some(pane) => standing_in_pane(pane),
      None => standing(place, turn),
    };

    match now {
      Standing::Ended(came) => return hand_back(ground, place, turn, came).await,
      Standing::Lost => return hand_back(ground, place, turn, Came::Failed(LOST.to_owned())).await,
      Standing::Going => {}
    }

    if !cut && cancelled(&ground.db, &[turn.run.as_str()]).await.unwrap_or(false) {
      cut = true;
      interrupt(ground, &turn.run, pane).await;
    }

    // A pane of its own reads its program on every look above.
    if pane.is_some() {
      tokio::time::sleep(every).await;
      continue;
    }

    // The program gone is the turn's end; read so on as many looks in a row as a pane gone is confirmed
    // on, so one read caught between its record's writes does not end a turn that goes on.
    unseen = if place.program_alive().is_some() { 0 } else { unseen + 1 };

    if unseen >= RESIDENT_TURN.gone_looks {
      return hand_back(ground, place, turn, Came::Failed(GONE.to_owned())).await;
    }

    tokio::time::sleep(every).await;
  }
}

// A kept turn cut short: its surface's interrupt typed into the pane of each open row it put in, at once;
// a role's turn in a pane of its own is told the stop words, as the door tells a run that is no chat round.
async fn interrupt(ground: &Ground, run: &str, pane: Option<&OwnPane>) {
  let Some(herdr) = &ground.herdr else { return };

  if let Some(pane) = pane.and_then(|pane| pane.pane.as_deref()) {
    let words = &policy::run_stop::RUN_STOP.instruction;
    let _ = herdr.call("agent.prompt", json!({ "target": pane, "text": words }), None).await;

    return;
  }

  for row in open_rows(ground, run).await {
    let directory = ground.roots.data.join("executions").join(row.id());
    let body = Value::Object(row.body);

    Stops::interrupt_resident(&ground.db, herdr, &body, &directory, true).await;
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

// The open execution rows a turn's program put in under its run: a dsh pane's turn names its run on its
// row, and a Claude Code pane's is named by the turn's liveness row, which names the run and its execution.
async fn open_rows(ground: &Ground, run: &str) -> Vec<Row> {
  all(
    &ground.db,
    r#"SELECT "body" FROM "agent_executions"
       WHERE ("body"::jsonb)->>'endedAt' IS NULL
         AND (("body"::jsonb)->'inboxTurn'->>'name' = $1
              OR "id" IN (SELECT "execution" FROM "turn_liveness" WHERE "run" = $1))"#,
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
          Came::Answered(_) => SUPERVISOR_RUNNER.log.answered.clone(),
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

  if end.ending.is_none() {
    end.ending = Some(crate::herdr_backend::ending_of(&end).to_owned());
  }

  ground.events.publish(TurnEnded {
    session: turn.session.clone(),
    end: end.clone(),
  });

  if let Some(job) = turn.job {
    crate::control::work::job_turn_ended(ground, job, end).await;
  }

  forget(db, &turn.run, "its end was handed back").await;
}

/// A turn whose program died with its run still open: closed with the moment and the reason, as the
/// program's own end would close it, so the room's turn is free and the words it did not answer wait for
/// the next turn. A pane that is still standing but whose turn's collector is gone is left alone. Each turn
/// is read in the place its program carries it in, and only where that place's run is the turn's: the run
/// of another turn, an earlier one in the agent's own place among them, says nothing of this one. A turn
/// this process still carries is its own end's to close, and is left to it.
pub async fn reap_dead(ground: &Ground) -> usize {
  let mut closed = 0;

  for turn in in_flight(&ground.db).await.unwrap_or_default() {
    if carried_here(&turn.run) {
      continue;
    }

    let place = Place::of(&ground.roots.data, &place_name_of(&turn));
    let Some(run) = crate::door::place_run::read_run(&place.path)
      .filter(|run| run.name.as_deref() == Some(turn.run.as_str()))
    else {
      continue;
    };

    if run.ended_at.is_some() {
      forget(&ground.db, &turn.run, "its place's run has ended").await;
      continue;
    }

    let rows = open_rows(ground, &turn.run).await;
    let dead = rows.iter().any(|row| crate::queue::reconcile::resident_gone(&row.body));

    if !dead {
      continue;
    }

    let (pid, birth) = rows
      .iter()
      .find_map(|row| {
        let resident = row.body.get("resident")?;
        Some((
          resident.get("pid").and_then(Value::as_i64),
          resident.get("birth").and_then(Value::as_str).map(str::to_owned),
        ))
      })
      .unwrap_or((None, None));
    let reason = format!(
      "The turn died silent: run {} in room {:?} on pid {:?} birth {:?} had no end.",
      turn.run, turn.chat_id, pid, birth
    );

    crate::door::place_run::end_run(&place.path, now_ms()).ok();
    hand_back(ground, &place, &turn, Came::Failed(reason)).await;
    closed += 1;
  }

  closed
}

// The road a picked-up chat turn walked, from its start to now.
fn road_walked(chat_id: i64, turn: &InFlight, provider: &str, reply: Option<&Reply>) -> RoadWalked {
  let spend = reply.map_or(&[][..], |reply| reply.spend.as_slice());
  let reported = !spend.is_empty();
  let sum = |pick: fn(&crate::door::record::ModelSpend) -> i64| {
    reported.then(|| spend.iter().map(pick).sum::<i64>())
  };

  RoadWalked {
    stage: SUPERVISOR_RUNNER.turn_stage.clone(),
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
