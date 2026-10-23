//! A run held to a clock is asked about past its bound rather than cut (docs/agent-control.md, 「A clock
//! asks before it cuts」). The clocked run is a row of `run_clocks` while it goes; past its bound an ask is a
//! row of `clock_asks`, published as `clock-asked` for the consumer that queues the order in the owner's
//! chat, and the run's door looks for the verdict on each pass until one comes or the wait for it runs
//! out, when agency cuts the run itself. A run that ends while asked closes its ask as ended.

use std::time::Duration;

use policy::fill_marks;
use policy::run_clock::RUN_CLOCK;
use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde::Serialize;
use serde_json::json;
use store::{all, one, run};

use crate::door::confirm::{Asking, Confirmation};
use crate::door::turn::{AsideOutcome, Overrun, Overtime, Watch};
use crate::events::{Bus, Event};
use crate::{Refusal, now_ms};

const MINUTE_MS: i64 = 60_000;

fn seconds_ms(seconds: u64) -> i64 {
  i64::try_from(seconds).unwrap_or(i64::MAX / 1_000).saturating_mul(1_000)
}

/// A run past its clock asked about (docs/api.md, 「Events」): the consumer holding the owner's chat queues
/// the order for the chat agent.
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
  pub answer_by: i64,
  pub verb: String,
}

impl Event for ClockAsked {
  const NAME: &'static str = "overtime-request.created";
}

/// An ask settled: by a verdict, by agency for want of one, or by its run ending while asked.
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
const BY_AGENCY: &str = "agency";

/// One clocked run as it opens: its name and the name it runs under, the work key and agent it is for,
/// what it is doing, and its bound.
#[derive(Clone, Debug)]
pub struct Clocked {
  pub run: String,
  pub parent: Option<String>,
  pub work_key: String,
  pub agent: Option<String>,
  pub what: String,
  pub bound: Duration,
}

impl Clocked {
  fn bound_ms(&self) -> i64 {
    i64::try_from(self.bound.as_millis()).unwrap_or(i64::MAX)
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
      clocked.bound_ms().into(),
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

/// A name's clock as a consumer reads it: when the latest bound of the runs standing under it runs out,
/// and until when the latest keep or open ask on them stands, each none where none stands.
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
         MAX(CASE WHEN a."closed_at" IS NULL THEN a."answer_by"
                  WHEN a."verdict" = 'keep' THEN a."kept_until" END) AS "kept_until"
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
  pub answer_by: i64,
}

/// Every ask still open, oldest first.
pub async fn open_asks<C: ConnectionTrait>(db: &C) -> Result<Vec<OpenAsk>, Refusal> {
  let rows = all(
    db,
    r#"SELECT a."id", a."run", a."asked_at", a."answer_by", c."agent", c."what"
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
        answer_by: row.try_get("", "answer_by")?,
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

/// The chat agent's verdict on an ask, closing it: a keep holds for the minutes named, the run's own bound
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

/// A run's door watch with the clock's ask put in front of it: past the bound an ask is opened and
/// published, its verdict looked for on each pass, the run kept while a keep holds, and cut on a verdict
/// to cut or once the wait for one runs out; every other word of the door goes to the watch it wraps.
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
    (self.clocked.bound_ms() / MINUTE_MS).to_string()
  }

  async fn ask_now(&mut self, overdue: &Overrun) -> Result<Overtime, Refusal> {
    let now = now_ms();
    let answer_by = now + seconds_ms(RUN_CLOCK.answer_wait_seconds);
    let agent = self.clocked.agent.clone().unwrap_or_else(|| self.clocked.work_key.clone());
    let facts = json!({
      "agent": agent,
      "workKey": self.clocked.work_key,
      "run": self.clocked.run,
      "what": self.clocked.what,
      "startedAt": overdue.started_at,
      "boundMs": overdue.bound_ms,
      "lastActivityAt": overdue.last_activity_at,
      "lastActivity": overdue.last_activity,
    });
    let row = one(
      self.db,
      r#"INSERT INTO "clock_asks" ("run", "facts", "asked_at", "answer_by") VALUES ($1, $2, $3, $4)
         RETURNING "id""#,
      vec![self.clocked.run.clone().into(), facts.to_string().into(), now.into(), answer_by.into()],
    )
    .await?
    .ok_or_else(|| Refusal::failed("the ask was not written"))?;
    let ask: i64 = row.try_get("", "id")?;

    self.ask = Some(ask);
    self.events.publish(ClockAsked {
      ask,
      agent,
      run: self.clocked.run.clone(),
      what: self.clocked.what.clone(),
      started_at: overdue.started_at,
      bound_ms: overdue.bound_ms,
      last_activity_at: overdue.last_activity_at,
      last_activity: overdue.last_activity.clone(),
      answer_by,
      verb: RUN_CLOCK.verb.clone(),
    });

    Ok(self.look_again())
  }

  // The verdict on the open ask, read once more: a keep holds the run until it runs out, when a new ask
  // is opened; a cut cuts it; and with none past the answer's due moment agency cuts it itself.
  async fn looked(&mut self, ask: i64, overdue: &Overrun) -> Result<Overtime, Refusal> {
    let Some(row) = one(
      self.db,
      r#"SELECT "verdict", "kept_until", "why", "answer_by", "closed_at" FROM "clock_asks" WHERE "id" = $1"#,
      vec![ask.into()],
    )
    .await?
    else {
      return self.ask_now(overdue).await;
    };
    let now = now_ms();
    let verdict: Option<String> = row.try_get("", "verdict")?;

    match verdict.as_deref() {
      Some(KEEP) => match row.try_get::<Option<i64>>("", "kept_until")?.filter(|until| *until > now) {
        Some(until) => Ok(Overtime::AskAgainAt(until)),
        None => self.ask_now(overdue).await,
      },
      Some(CUT) => {
        let why: Option<String> = row.try_get("", "why")?;

        self.cut = Some(fill_marks(
          &RUN_CLOCK.cut_by_agent,
          &[("minutes", &self.minutes()), ("why", why.as_deref().unwrap_or_default())],
        ));

        Ok(Overtime::Cut)
      }
      _ if row.try_get::<i64>("", "answer_by")? <= now => {
        run(
          self.db,
          r#"UPDATE "clock_asks" SET "verdict" = $2, "decided_by" = $3, "closed_at" = $4
             WHERE "id" = $1 AND "closed_at" IS NULL"#,
          vec![ask.into(), CUT.into(), BY_AGENCY.into(), now.into()],
        )
        .await?;
        self.events.publish(ClockAskClosed {
          ask,
          run: self.clocked.run.clone(),
          verdict: CUT.to_owned(),
          by: Some(BY_AGENCY.to_owned()),
        });
        self.cut = Some(fill_marks(
          &RUN_CLOCK.cut_by_agency,
          &[
            ("minutes", &self.minutes()),
            ("wait", &(RUN_CLOCK.answer_wait_seconds / 60).to_string()),
          ],
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
