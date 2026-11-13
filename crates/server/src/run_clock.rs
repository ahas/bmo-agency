//! A run held to a clock is asked about past it, never cut by it (docs/agent-control.md, 「A clock asks, it
//! never cuts」). The clocked run is a row of `run_clocks` while it goes; past its clock an ask is a row of
//! `clock_asks`, published as `clock-asked` for the consumer that queues the order in the owner's chat, and
//! the run's door looks for the verdict on each pass for as long as the run goes. Only the supervisor's
//! verdict cuts it. A run that ends while asked closes its ask as ended.
//!
//! A job's tool call gone silent and idle is asked about the same way (docs/agent-control.md, 「A job
//! turn's tool calls are held to a clock」): each such call is a clocked run of its own under the turn's
//! run, opened when the call clock first hands it over.

use std::collections::BTreeMap;
use std::time::Duration;

use policy::fill_marks;
use policy::run_clock::RUN_CLOCK;
use policy::tool_clock::TOOL_CLOCK;
use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::Serialize;
use serde_json::json;
use store::{all, one, run};

use crate::door::confirm::{Asking, Confirmation};
use crate::door::tool_clock::{CallVerdict, SilentCall};
use crate::door::turn::{AsideOutcome, Overrun, Overtime, Watch};
use crate::events::{Bus, Event};
use crate::{Refusal, now_ms};

const MINUTE_MS: i64 = 60_000;

fn seconds_ms(seconds: u64) -> i64 {
  i64::try_from(seconds).unwrap_or(i64::MAX / 1_000).saturating_mul(1_000)
}

/// A run past its clock asked about (docs/api.md, 「Events」): the consumer holding the owner's chat queues
/// the order for the supervisor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockAsked {
  pub ask: i64,
  pub agent: String,
  pub run: String,
  pub what: String,
  pub started_at: i64,
  pub bound_ms: i64,
  pub last_activity_at: Option<i64>,
  pub last_activity: Option<String>,
  pub verb: String,
}

impl Event for ClockAsked {
  const NAME: &'static str = "overtime-request.created";
}

/// An ask settled: by the supervisor's verdict, or by its run ending while asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockAskClosed {
  pub ask: i64,
  pub run: String,
  pub verdict: String,
  pub by: Option<String>,
}

impl Event for ClockAskClosed {
  const NAME: &'static str = "overtime-request.changed";
}

const KEEP: &str = "keep";
const CUT: &str = "cut";
const ENDED: &str = "ended";
const BY_AGENT: &str = "agent";

/// One clocked run as it opens: its name and the name it runs under, the work key and agent it is for,
/// what it is doing, and its clock.
#[derive(Clone, Debug)]
pub struct Clocked {
  pub run: String,
  pub parent: Option<String>,
  pub work_key: String,
  pub agent: Option<String>,
  pub what: String,
  pub clock: Duration,
}

impl Clocked {
  fn clock_ms(&self) -> i64 {
    i64::try_from(self.clock.as_millis()).unwrap_or(i64::MAX)
  }
}

/// The clock opened on a run, its rows of runs ended past the policy's keeping let go first. A run of the
/// same name clocked again starts its clock afresh.
pub async fn open(db: &DatabaseConnection, clocked: &Clocked) -> Result<(), Refusal> {
  let now = now_ms();
  let kept_ms = i64::try_from(RUN_CLOCK.kept_days).unwrap_or(0).saturating_mul(24 * 60 * MINUTE_MS);
  let before = now - kept_ms;

  run(
    db,
    r#"DELETE FROM "clock_asks" WHERE "run" IN (SELECT "run" FROM "run_clocks" WHERE "ended_at" < $1)"#,
    vec![before.into()],
  )
  .await?;
  run(db, r#"DELETE FROM "run_clocks" WHERE "ended_at" < $1"#, vec![before.into()]).await?;
  run(
    db,
    r#"INSERT INTO "run_clocks" ("run", "parent", "work_key", "agent", "what", "started_at", "bound_ms", "ended_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7, NULL)
       ON CONFLICT ("run") DO UPDATE SET "parent" = $2, "work_key" = $3, "agent" = $4, "what" = $5,
         "started_at" = $6, "bound_ms" = $7, "ended_at" = NULL"#,
    vec![
      clocked.run.clone().into(),
      clocked.parent.clone().into(),
      clocked.work_key.clone().into(),
      clocked.agent.clone().into(),
      clocked.what.clone().into(),
      now.into(),
      clocked.clock_ms().into(),
    ],
  )
  .await?;

  Ok(())
}

/// The run's clock closed as it ends: every ask still open on it closed as ended, and published so an
/// order not yet delivered is withdrawn.
pub async fn close(db: &DatabaseConnection, events: &Bus, run_name: &str) -> Result<(), Refusal> {
  let now = now_ms();
  let open = all(
    db,
    r#"SELECT "id" FROM "clock_asks" WHERE "run" = $1 AND "closed_at" IS NULL"#,
    vec![run_name.into()],
  )
  .await?;

  run(
    db,
    r#"UPDATE "clock_asks" SET "verdict" = $2, "closed_at" = $3 WHERE "run" = $1 AND "closed_at" IS NULL"#,
    vec![run_name.into(), ENDED.into(), now.into()],
  )
  .await?;
  run(
    db,
    r#"UPDATE "run_clocks" SET "ended_at" = $2 WHERE "run" = $1"#,
    vec![run_name.into(), now.into()],
  )
  .await?;

  for row in open {
    events.publish(ClockAskClosed {
      ask: row.try_get("", "id")?,
      run: run_name.to_owned(),
      verdict: ENDED.to_owned(),
      by: None,
    });
  }

  Ok(())
}

/// A name's clock as a consumer reads it: when the latest clock of the runs standing under it passes, and
/// until when the latest keep on them stands, each none where none stands. Neither ends a run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockRead {
  pub cap_ends: Option<i64>,
  pub kept_until: Option<i64>,
}

/// The clock of the runs standing under a name: the run of that name, and those that name it as the unit
/// they run under.
pub async fn clock_of<C: ConnectionTrait>(db: &C, name: &str) -> Result<ClockRead, Refusal> {
  let row = one(
    db,
    r#"SELECT MAX(c."started_at" + c."bound_ms") AS "cap_ends",
         MAX(CASE WHEN a."verdict" = 'keep' THEN a."kept_until" END) AS "kept_until"
       FROM "run_clocks" c LEFT JOIN "clock_asks" a ON a."run" = c."run"
       WHERE c."ended_at" IS NULL AND (c."run" = $1 OR c."parent" = $1)"#,
    vec![name.into()],
  )
  .await?;
  let now = now_ms();
  let read = |column: &str| -> Result<Option<i64>, Refusal> {
    Ok(match &row {
      Some(row) => row.try_get::<Option<i64>>("", column)?,
      None => None,
    })
  };

  Ok(ClockRead {
    cap_ends: read("cap_ends")?,
    kept_until: read("kept_until")?.filter(|until| *until > now),
  })
}

/// An ask still open, as a list shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAsk {
  pub ask: i64,
  pub agent: Option<String>,
  pub run: String,
  pub what: String,
  pub asked_at: i64,
}

/// Every ask still open, oldest first.
pub async fn open_asks<C: ConnectionTrait>(db: &C) -> Result<Vec<OpenAsk>, Refusal> {
  let rows = all(
    db,
    r#"SELECT a."id", a."run", a."asked_at", c."agent", c."what"
       FROM "clock_asks" a JOIN "run_clocks" c ON c."run" = a."run"
       WHERE a."closed_at" IS NULL ORDER BY a."id""#,
    vec![],
  )
  .await?;

  rows
    .iter()
    .map(|row| {
      Ok(OpenAsk {
        ask: row.try_get("", "id")?,
        agent: row.try_get("", "agent")?,
        run: row.try_get("", "run")?,
        what: row.try_get("", "what")?,
        asked_at: row.try_get("", "asked_at")?,
      })
    })
    .collect()
}

/// A verdict on an ask: keep it going for a while, or cut it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
  /// Kept for so many minutes, the run's own bound where none is named.
  Keep(Option<i64>),
  Cut,
}

/// Why a verdict was not taken.
#[derive(Debug)]
pub enum NotSettled {
  /// No ask of that number.
  Unknown,
  /// The ask was settled already, or its run ended.
  Closed,
  Store(Refusal),
}

impl From<Refusal> for NotSettled {
  fn from(refusal: Refusal) -> Self {
    Self::Store(refusal)
  }
}

impl From<sea_orm::DbErr> for NotSettled {
  fn from(why: sea_orm::DbErr) -> Self {
    Self::Store(why.into())
  }
}

impl From<store::Fault> for NotSettled {
  fn from(fault: store::Fault) -> Self {
    Self::Store(fault.into())
  }
}

/// What a verdict came to: the ask, its run, the verdict and until when a keep holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settled {
  pub ask: i64,
  pub run: String,
  pub verdict: String,
  pub kept_until: Option<i64>,
}

/// The supervisor's verdict on an ask, closing it: a keep holds for the minutes named, the run's own clock
/// where none is, and never longer than the policy's longest keep; the run's door reads it on its next look.
pub async fn settle(
  db: &DatabaseConnection,
  events: &Bus,
  ask: i64,
  verdict: &Verdict,
  why: &str,
) -> Result<Settled, NotSettled> {
  let Some(row) = one(
    db,
    r#"SELECT a."run", a."closed_at", c."bound_ms" FROM "clock_asks" a
       LEFT JOIN "run_clocks" c ON c."run" = a."run" WHERE a."id" = $1"#,
    vec![ask.into()],
  )
  .await?
  else {
    return Err(NotSettled::Unknown);
  };

  if row.try_get::<Option<i64>>("", "closed_at")?.is_some() {
    return Err(NotSettled::Closed);
  }

  let run_name: String = row.try_get("", "run")?;
  let now = now_ms();
  let (word, kept_until) = match verdict {
    Verdict::Keep(minutes) => {
      let asked = match minutes {
        Some(minutes) => minutes.saturating_mul(MINUTE_MS),
        None => row.try_get::<Option<i64>>("", "bound_ms")?.unwrap_or(MINUTE_MS),
      };
      let held = asked.clamp(MINUTE_MS, seconds_ms(RUN_CLOCK.longest_keep_seconds));

      (KEEP, Some(now + held))
    }
    Verdict::Cut => (CUT, None),
  };
  let why = why.trim();
  let taken = run(
    db,
    r#"UPDATE "clock_asks" SET "verdict" = $2, "kept_until" = $3, "decided_by" = $4, "why" = $5,
         "closed_at" = $6 WHERE "id" = $1 AND "closed_at" IS NULL"#,
    vec![
      ask.into(),
      word.into(),
      kept_until.into(),
      BY_AGENT.into(),
      (!why.is_empty()).then(|| why.to_owned()).into(),
      now.into(),
    ],
  )
  .await?;

  if taken == 0 {
    return Err(NotSettled::Closed);
  }

  events.publish(ClockAskClosed {
    ask,
    run: run_name.clone(),
    verdict: word.to_owned(),
    by: Some(BY_AGENT.to_owned()),
  });

  Ok(Settled {
    ask,
    run: run_name,
    verdict: word.to_owned(),
    kept_until,
  })
}

/// An ask opened on a clocked run past its clock: its row written with the facts it carries, and published
/// for the consumer that queues the order in the owner's chat.
async fn put_ask(
  db: &DatabaseConnection,
  events: &Bus,
  clocked: &Clocked,
  overdue: &Overrun,
) -> Result<i64, Refusal> {
  let now = now_ms();
  let agent = clocked.agent.clone().unwrap_or_else(|| clocked.work_key.clone());
  let facts = json!({
    "agent": agent,
    "workKey": clocked.work_key,
    "run": clocked.run,
    "what": clocked.what,
    "startedAt": overdue.started_at,
    "boundMs": overdue.bound_ms,
    "lastActivityAt": overdue.last_activity_at,
    "lastActivity": overdue.last_activity,
  });
  let row = one(
    db,
    r#"INSERT INTO "clock_asks" ("run", "facts", "asked_at") VALUES ($1, $2, $3) RETURNING "id""#,
    vec![clocked.run.clone().into(), facts.to_string().into(), now.into()],
  )
  .await?
  .ok_or_else(|| Refusal::failed("the ask was not written"))?;
  let ask: i64 = row.try_get("", "id")?;

  events.publish(ClockAsked {
    ask,
    agent,
    run: clocked.run.clone(),
    what: clocked.what.clone(),
    started_at: overdue.started_at,
    bound_ms: overdue.bound_ms,
    last_activity_at: overdue.last_activity_at,
    last_activity: overdue.last_activity.clone(),
    verb: RUN_CLOCK.verb.clone(),
  });

  Ok(ask)
}

/// An ask as it stands: its verdict where one was given, until when a keep holds, and why.
struct AskRead {
  verdict: Option<String>,
  kept_until: Option<i64>,
  why: Option<String>,
}

/// The ask of that number as it stands; none where there is no such ask.
async fn ask_read(db: &DatabaseConnection, ask: i64) -> Result<Option<AskRead>, Refusal> {
  let Some(row) = one(
    db,
    r#"SELECT "verdict", "kept_until", "why" FROM "clock_asks" WHERE "id" = $1"#,
    vec![ask.into()],
  )
  .await?
  else {
    return Ok(None);
  };

  Ok(Some(AskRead {
    verdict: row.try_get("", "verdict")?,
    kept_until: row.try_get("", "kept_until")?,
    why: row.try_get("", "why")?,
  }))
}

/// A run's door watch with the clock's ask put in front of it: past the clock an ask is opened and
/// published, its verdict looked for on each pass for as long as the run goes, the run kept while a keep
/// holds and cut on a verdict to cut; every other word of the door goes to the watch it wraps.
pub struct Asked<'a, W> {
  pub inner: W,
  db: &'a DatabaseConnection,
  events: &'a Bus,
  clocked: &'a Clocked,
  ask: Option<i64>,
  /// Why the run was cut on its clock, once it was.
  pub cut: Option<String>,
}

impl<'a, W: Watch + Send> Asked<'a, W> {
  pub fn new(inner: W, db: &'a DatabaseConnection, events: &'a Bus, clocked: &'a Clocked) -> Self {
    Self {
      inner,
      db,
      events,
      clocked,
      ask: None,
      cut: None,
    }
  }

  fn look_again(&self) -> Overtime {
    Overtime::AskAgainAt(now_ms() + seconds_ms(RUN_CLOCK.look_every_seconds))
  }

  fn minutes(&self) -> String {
    (self.clocked.clock_ms() / MINUTE_MS).to_string()
  }

  async fn ask_now(&mut self, overdue: &Overrun) -> Result<Overtime, Refusal> {
    self.ask = Some(put_ask(self.db, self.events, self.clocked, overdue).await?);

    Ok(self.look_again())
  }

  // The verdict on the open ask, read once more: a keep holds the run until it runs out, when a new ask
  // is opened; a cut cuts it; and with none the run goes on and the ask stands.
  async fn looked(&mut self, ask: i64, overdue: &Overrun) -> Result<Overtime, Refusal> {
    let Some(read) = ask_read(self.db, ask).await? else {
      return self.ask_now(overdue).await;
    };
    let now = now_ms();

    match read.verdict.as_deref() {
      Some(KEEP) => match read.kept_until.filter(|until| *until > now) {
        Some(until) => Ok(Overtime::AskAgainAt(until)),
        None => self.ask_now(overdue).await,
      },
      Some(CUT) => {
        self.cut = Some(fill_marks(
          &RUN_CLOCK.cut_by_agent,
          &[("minutes", &self.minutes()), ("why", read.why.as_deref().unwrap_or_default())],
        ));

        Ok(Overtime::Cut)
      }
      _ => Ok(self.look_again()),
    }
  }
}

impl<W: Watch + Send> Watch for Asked<'_, W> {
  fn opened(&mut self, execution: &str) -> impl Future<Output = ()> + Send {
    self.inner.opened(execution)
  }

  fn words(&mut self) -> impl Future<Output = Option<String>> + Send {
    self.inner.words()
  }

  fn said(&mut self, outcome: AsideOutcome) -> impl Future<Output = ()> + Send {
    self.inner.said(outcome)
  }

  async fn overtime(&mut self, overdue: &Overrun) -> Overtime {
    let asked = match self.ask {
      Some(ask) => self.looked(ask, overdue).await,
      None => self.ask_now(overdue).await,
    };

    asked.unwrap_or_else(|why| Overtime::Failed(why.detail))
  }

  fn note(&mut self, text: &str) -> impl Future<Output = ()> + Send {
    self.inner.note(text)
  }

  fn puts_questions(&self) -> bool {
    self.inner.puts_questions()
  }

  fn confirm(&mut self, asked: &Confirmation) -> impl Future<Output = Option<Asking>> + Send {
    self.inner.confirm(asked)
  }

  fn record(&mut self, lines: &[String]) -> impl Future<Output = ()> + Send {
    self.inner.record(lines)
  }

  fn stopping(&mut self) -> bool {
    self.inner.stopping()
  }
}

/// The asks a job's turn opens on its tool calls gone silent and idle: each call a clocked run of its own,
/// named for the call under the turn's run, its clock the call clock's silent window. The call clock hands a
/// call over when it goes silent and idle and on each read while its ask stands; a call that ends, and the
/// turn's end, close the asks on it as ended.
pub struct CallAsks {
  turn: String,
  agent: String,
  job: i64,
  // The job's item, read once; none where the store names none.
  item: Option<Option<i64>>,
  // Every call asked about, by its id, with the ask that stands on it with no verdict.
  calls: BTreeMap<String, Option<i64>>,
}

impl CallAsks {
  pub fn new(turn: &str, agent: &str, job: i64) -> Self {
    Self {
      turn: turn.to_owned(),
      agent: agent.to_owned(),
      job,
      item: None,
      calls: BTreeMap::new(),
    }
  }

  /// The name a call's clock runs under.
  pub fn run_of(&self, call: &str) -> String {
    format!("{}/call/{call}", self.turn)
  }

  /// The policy's answer on a call handed over: an ask opened where none stands, else the verdict on the
  /// one that does. An ask that cannot be put or read leaves the call running.
  pub async fn hear(&mut self, db: &DatabaseConnection, events: &Bus, call: &SilentCall) -> CallVerdict {
    self.heard(db, events, call).await.unwrap_or_else(|why| CallVerdict::Failed(why.detail))
  }

  async fn heard(
    &mut self,
    db: &DatabaseConnection,
    events: &Bus,
    call: &SilentCall,
  ) -> Result<CallVerdict, Refusal> {
    if let Some(Some(ask)) = self.calls.get(&call.id).copied() {
      let verdict = match ask_read(db, ask).await? {
        None => CallVerdict::Failed(format!("the ask {ask} is gone")),
        Some(read) => match read.verdict.as_deref() {
          None => return Ok(CallVerdict::Waiting),
          Some(KEEP) => CallVerdict::Kept(read.kept_until.unwrap_or_else(now_ms)),
          Some(CUT) => CallVerdict::Cut(read.why.unwrap_or_default()),
          Some(other) => CallVerdict::Failed(format!("the ask {ask} closed as {other}")),
        },
      };

      self.calls.insert(call.id.clone(), None);

      return Ok(verdict);
    }

    let item = self.item(db).await?;
    let silent = &TOOL_CLOCK.call.silent;
    let what = fill_marks(
      &silent.what,
      &[
        ("job", &self.job.to_string()),
        ("item", &item.map_or_else(|| "-".to_owned(), |item| item.to_string())),
        ("tool", &call.tool),
        ("cpu", &format!("{:.1}", call.cpu_percent)),
        ("minutes", &(call.window_ms / MINUTE_MS).max(1).to_string()),
      ],
    );
    let clocked = Clocked {
      run: self.run_of(&call.id),
      parent: Some(self.turn.clone()),
      work_key: crate::queue::routes::JOB_WORK_KEY.to_owned(),
      agent: Some(self.agent.clone()),
      what,
      clock: Duration::from_millis(u64::try_from(call.window_ms).unwrap_or_default()),
    };
    let overdue = Overrun {
      started_at: call.started_at,
      bound_ms: call.window_ms,
      last_activity_at: Some(call.output_changed_at),
      last_activity: Some(call.last_line.clone().unwrap_or_else(|| silent.nothing_written.clone())),
    };

    open(db, &clocked).await?;

    let ask = put_ask(db, events, &clocked, &overdue).await?;

    self.calls.insert(call.id.clone(), Some(ask));

    Ok(CallVerdict::Waiting)
  }

  async fn item(&mut self, db: &DatabaseConnection) -> Result<Option<i64>, Refusal> {
    if let Some(item) = self.item {
      return Ok(item);
    }

    let row = one(db, r#"SELECT "item_id" FROM "jobs" WHERE "id" = $1"#, vec![self.job.into()]).await?;
    let item = match row {
      Some(row) => row.try_get::<Option<i64>>("", "item_id")?,
      None => None,
    };

    self.item = Some(item);

    Ok(item)
  }

  /// A call asked about has ended, or with none every call of the turn: its clock closes, and an ask still
  /// standing on it closes as ended and is withdrawn.
  pub async fn ended(&mut self, db: &DatabaseConnection, events: &Bus, call: Option<&str>) {
    let ended: Vec<String> = match call {
      Some(call) => self.calls.contains_key(call).then(|| call.to_owned()).into_iter().collect(),
      None => self.calls.keys().cloned().collect(),
    };

    for call in ended {
      self.calls.remove(&call);

      if let Err(why) = close(db, events, &self.run_of(&call)).await {
        eprintln!("{}: the clock of the call {call} was not closed: {}", self.turn, why.detail);
      }
    }
  }
}
