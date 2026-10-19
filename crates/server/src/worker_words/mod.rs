//! The words said to a worker (docs/agent-control.md, 「The keepalive ping goes into the same program as the
//! turns」): a word written down with its line in the worker's log and offered at once, the words still
//! waiting offered again every few seconds, one worker apart from another. A turn working on the worker's
//! session — a job's — takes them where it hears words; where none works, they go into a turn of their own on
//! the worker's session, held to its clock, which takes the words said meanwhile at its next look. What the
//! worker is told them in is the consumer's to word (docs/api.md, 「Hooks」, the worker's words): agency
//! carries the words as said where nobody words them.

pub mod said;

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use agent_session::{Aside, Fault};
use policy::fill_marks;
use policy::worker_words::WORKER_WORDS;
use serde::Serialize;

use self::said::Word;
use crate::door::turn::{AsideOutcome, Watch};
use crate::hooks::HookPoint;
use crate::places::{AgentRow, Ground};
use crate::queue::routes::worker_pin;
use crate::run_clock::{self, Asked, Clocked};
use crate::worker_session::{Missed, PROVIDER, Turned};
use crate::{Refusal, now_ms};

/// How the words reach the worker: in a turn of their own, into the turn a job of its own is working, or
/// into the turn answering earlier words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Way {
  Task,
  Aside,
  More,
}

/// One word as the consumer words it: what was said and when, in epoch milliseconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaidWord {
  pub text: String,
  pub said_at: i64,
}

/// What the consumer is asked to word: the worker, the way the words reach it, and the words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Told {
  pub worker: String,
  pub way: Way,
  pub words: Vec<SaidWord>,
}

/// The point the worker's words are worded at: the consumer answers the whole text the worker is told, or
/// none, and the words go as said.
pub struct WorkerWordsHook;

impl HookPoint for WorkerWordsHook {
  type Ask = Told;
  type Answer = Option<String>;
  const NAME: &'static str = "worker-words";

  fn default_answer(_told: Told) -> Option<String> {
    None
  }
}

crate::wired!(WorkerWordsHook);

/// The words as said, one after another.
pub fn as_said(words: &[Word]) -> String {
  words.iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join("\n\n")
}

/// What the worker is told the words in: the consumer's wording, or the words as said.
pub async fn told(ground: &Ground, worker: &str, way: Way, words: &[Word]) -> String {
  let asked = Told {
    worker: worker.to_owned(),
    way,
    words: words
      .iter()
      .map(|word| SaidWord {
        text: word.text.clone(),
        said_at: word.added_at.saturating_mul(1000),
      })
      .collect(),
  };

  match ground.hooks.ask::<WorkerWordsHook>(asked).await {
    Some(text) if !text.trim().is_empty() => text,
    _ => as_said(words),
  }
}

/// A word's turn as it is asked of the road: what is said, the run's name, what a person watching reads over
/// it, and its bound.
pub struct WordTurn {
  pub prompt: String,
  pub name: String,
  pub title: String,
  pub bound: Duration,
}

/// Where the words go: the turn working on a worker's session, or a turn of their own on it.
pub trait WordRoad: Send + Sync + 'static {
  /// The words pushed into the turn the worker's session is working, where one hears them.
  fn push(&self, worker: &str, words: &str) -> impl Future<Output = Result<Aside, Fault>> + Send;

  /// One turn of the words on the worker's own session; none where a turn is already working there.
  fn turn<W: Watch + Send + Sync>(
    &self,
    worker: &AgentRow,
    asked: WordTurn,
    watch: &mut W,
  ) -> impl Future<Output = Result<Option<Turned>, Refusal>> + Send;
}

/// What one offer of a worker's words came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
  /// Nothing waits.
  None,
  /// A turn of the words is already running for the worker, or a turn works that hears none.
  Busy,
  /// The worker's pane or its job waits on a person, and the words are let be for a while.
  Held,
  /// The turn a job of the worker's is working took them.
  Taken,
  /// The worker is pinned to another provider, on whose road no word goes.
  Pinned,
  /// The words never reached the session.
  Unreached,
  Answered,
  Unanswered,
}

/// What the words keep between offers: the workers a turn of words is running for, since when each
/// worker's pane was found waiting on a person, and the line last written under each word.
#[derive(Default)]
struct Kept {
  handing: Mutex<HashSet<String>>,
  waiting_on_person: Mutex<HashMap<String, i64>>,
  noted: Mutex<HashMap<i64, String>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
  mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// A worker's name held as being handed words for as long as one turn of them runs.
struct Handing<'a> {
  kept: &'a Kept,
  worker: String,
}

impl<'a> Handing<'a> {
  fn take(kept: &'a Kept, worker: &str) -> Option<Self> {
    locked(&kept.handing).insert(worker.to_owned()).then(|| Self {
      kept,
      worker: worker.to_owned(),
    })
  }
}

impl Drop for Handing<'_> {
  fn drop(&mut self) {
    locked(&self.kept.handing).remove(&self.worker);
  }
}

fn person_wait_ms() -> i64 {
  i64::try_from(WORKER_WORDS.person_wait_seconds)
    .unwrap_or(i64::MAX / 1000)
    .saturating_mul(1000)
}

fn held_line(missed: Missed) -> &'static str {
  let held = &WORKER_WORDS.held;

  match missed {
    Missed::Person => &held.person,
    Missed::Untrusted => &held.untrusted,
    Missed::Stopped => &held.stopped,
    Missed::Disabled => &held.disabled,
    Missed::Lost => &held.lost,
    Missed::Resting => &held.resting,
    Missed::Unsubmitted | Missed::Idle => &held.unsubmitted,
  }
}

/// The words of one deployment: its ground, the road they go out on, and what is kept between offers.
/// Cloned freely: every clone keeps the one record.
pub struct Words<R> {
  ground: Arc<Ground>,
  road: Arc<R>,
  kept: Arc<Kept>,
}

impl<R> Clone for Words<R> {
  fn clone(&self) -> Self {
    Self {
      ground: self.ground.clone(),
      road: self.road.clone(),
      kept: self.kept.clone(),
    }
  }
}

impl<R: WordRoad> Words<R> {
  pub fn new(ground: Arc<Ground>, road: Arc<R>) -> Self {
    Self {
      ground,
      road,
      kept: Arc::default(),
    }
  }

  /// A word said to one worker: written down with its line in the worker's log, and offered at once without
  /// the caller waiting on its turn. Its id.
  pub async fn say(&self, worker: &str, text: &str) -> Result<i64, Refusal> {
    let id = said::say(&self.ground.db, worker, text).await?;
    let words = self.clone();
    let worker = worker.to_owned();

    tokio::spawn(async move {
      if let Err(why) = words.offer(&worker, false).await {
        eprintln!("word {worker}: {}", why.detail);
      }
    });

    Ok(id)
  }

  /// Every worker's waiting words offered again, each worker apart from the others, for as long as the
  /// process runs.
  pub fn stand(self) {
    let every = Duration::from_secs(WORKER_WORDS.tick_seconds);

    tokio::spawn(async move {
      let mut ticks = tokio::time::interval(every);

      ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
      ticks.tick().await;

      loop {
        ticks.tick().await;

        let workers = match AgentRow::workers(&self.ground.db).await {
          Ok(workers) => workers,
          Err(why) => {
            eprintln!("words: the workers could not be read: {}", why.detail);
            continue;
          }
        };

        for row in workers {
          let words = self.clone();

          tokio::spawn(async move {
            if let Err(why) = words.offer(&row.name, true).await {
              eprintln!("word {}: {}", row.name, why.detail);
            }
          });
        }
      }
    });
  }

  /// One worker handed whatever it has not been handed. A round offering words again rather than a word just
  /// said leaves a pane waiting on a person alone until its while is up.
  pub async fn offer(&self, worker: &str, retry: bool) -> Result<Delivery, Refusal> {
    let db = &self.ground.db;
    let words = said::waiting(db, worker).await?;

    if words.is_empty() {
      return Ok(Delivery::None);
    }

    let waited = locked(&self.kept.waiting_on_person).get(worker).copied();

    if retry && waited.is_some_and(|since| now_ms() - since < person_wait_ms()) {
      return Ok(Delivery::Held);
    }

    // A turn of the words already running takes the words said meanwhile at its own next look.
    if locked(&self.kept.handing).contains(worker) {
      return Ok(Delivery::Busy);
    }

    let aside = told(&self.ground, worker, Way::Aside, &words).await;

    match self.road.push(worker, &aside).await {
      Ok(Aside::Delivered) => {
        said::delivered(db, &words).await?;
        locked(&self.kept.waiting_on_person).remove(worker);
        self.note_under(worker, &words, &WORKER_WORDS.held.in_job).await?;

        return Ok(Delivery::Taken);
      }
      Ok(Aside::Waiting) => {
        locked(&self.kept.waiting_on_person).insert(worker.to_owned(), now_ms());
        self.note_under(worker, &words, &WORKER_WORDS.held.job_waiting).await?;

        return Ok(Delivery::Held);
      }
      Ok(Aside::Refused) => {}
      Err(fault) => return Err(Refusal::failed(fault.message)),
    }

    // A worker pinned to another provider that is on holds no Claude session to say them into, and agency
    // carries no run on that pin, so they wait.
    let pin = worker_pin(db, worker).await.map_err(|why| Refusal::failed(why.0))?;

    if let Some((provider, _)) = pin.filter(|(provider, _)| provider != PROVIDER) {
      let line = fill_marks(&WORKER_WORDS.held.pinned, &[("provider", &provider)]);

      self.note_under(worker, &words, &line).await?;

      return Ok(Delivery::Pinned);
    }

    let Some(_handing) = Handing::take(&self.kept, worker) else {
      return Ok(Delivery::Busy);
    };
    let Some(row) = AgentRow::named(db, worker).await? else { return Ok(Delivery::None) };

    self.own_turn(&row).await
  }

  // The words still waiting carried in a turn of their own, on the clock a word's turn carries; the answer
  // stands under the last word it took.
  async fn own_turn(&self, row: &AgentRow) -> Result<Delivery, Refusal> {
    let ground = &*self.ground;
    let worker = &row.name;
    // Read again: a job's turn may have taken them meanwhile.
    let words = said::waiting(&ground.db, worker).await?;

    if words.is_empty() {
      return Ok(Delivery::None);
    }

    let bound = Duration::from_secs(WORKER_WORDS.answer_minutes * 60);
    let name = format!("{}-word-{worker}", policy::deployment::DEPLOYMENT.agent_key);
    let clocked = Clocked {
      run: name.clone(),
      parent: None,
      work_key: WORKER_WORDS.work_key.clone(),
      agent: Some(worker.clone()),
      what: fill_marks(&WORKER_WORDS.what, &[("worker", worker)]),
      bound,
    };
    let watch = WordWatch {
      ground,
      worker: worker.clone(),
      taken: words.clone(),
      pending: vec![],
    };
    let mut asked = Asked::new(watch, &ground.db, &ground.events, &clocked);
    let turn = WordTurn {
      prompt: told(ground, worker, Way::Task, &words).await,
      name: name.clone(),
      title: fill_marks(&WORKER_WORDS.title, &[("worker", worker)]),
      bound,
    };

    if let Err(why) = run_clock::open(&ground.db, &clocked).await {
      eprintln!("word {worker}: the turn's clock was not written: {}", why.detail);
    }

    let turned = self.road.turn(row, turn, &mut asked).await;

    if let Err(why) = run_clock::close(&ground.db, &ground.events, &name).await {
      eprintln!("word {worker}: the turn's clock was not closed: {}", why.detail);
    }

    let cut = asked.cut.take();
    let taken = asked.inner.taken;
    let Some(turned) = turned? else { return Ok(Delivery::Busy) };

    // Marked once the words reached the session, whether or not that turn went on to answer; a turn that
    // never saw them leaves them for the next round, with what stood in the way under them.
    if !turned.reached {
      let missed = turned.missed.unwrap_or(Missed::Unsubmitted);

      if missed == Missed::Person {
        locked(&self.kept.waiting_on_person).insert(worker.clone(), now_ms());
      }

      self.note_under(worker, &words, held_line(missed)).await?;

      return Ok(Delivery::Unreached);
    }

    locked(&self.kept.waiting_on_person).remove(worker);
    said::delivered(&ground.db, &taken).await?;

    let last = taken.last().map_or(words[words.len() - 1].id, |word| word.id);
    let answer = turned.reply.map(|reply| reply.text).filter(|text| !text.trim().is_empty());

    match answer {
      Some(text) => {
        said::write(&ground.db, worker, last, "text", None, &text).await?;

        Ok(Delivery::Answered)
      }
      None => {
        let line = match cut {
          Some(cut) => fill_marks(
            &WORKER_WORDS.ended.cut,
            &[("clock", &WORKER_WORDS.answer_minutes.to_string()), ("cut", &cut)],
          ),
          None => WORKER_WORDS.ended.silent.clone(),
        };

        said::write(&ground.db, worker, last, "note", None, &line).await?;

        Ok(Delivery::Unanswered)
      }
    }
  }

  // What held the words, written once under the last of them.
  async fn note_under(&self, worker: &str, words: &[Word], line: &str) -> Result<(), Refusal> {
    let Some(last) = words.last() else { return Ok(()) };

    if locked(&self.kept.noted).get(&last.id).is_some_and(|noted| noted == line) {
      return Ok(());
    }

    said::write(&self.ground.db, worker, last.id, "note", None, line).await?;
    locked(&self.kept.noted).insert(last.id, line.to_owned());

    Ok(())
  }
}

// The watch on a word's turn: the words said while it answers handed into it at its next look, and kept as
// taken once it took them.
struct WordWatch<'a> {
  ground: &'a Ground,
  worker: String,
  taken: Vec<Word>,
  pending: Vec<Word>,
}

impl Watch for WordWatch<'_> {
  async fn words(&mut self) -> Option<String> {
    if !self.pending.is_empty() {
      return None;
    }

    let waiting = said::waiting(&self.ground.db, &self.worker).await.ok()?;
    let more: Vec<Word> = waiting
      .into_iter()
      .filter(|word| !self.taken.iter().any(|taken| taken.id == word.id))
      .collect();

    if more.is_empty() {
      return None;
    }

    let text = told(self.ground, &self.worker, Way::More, &more).await;

    self.pending = more;
    Some(text)
  }

  async fn said(&mut self, outcome: AsideOutcome) {
    let pending = std::mem::take(&mut self.pending);

    if outcome == AsideOutcome::Delivered {
      self.taken.extend(pending);
    }
  }
}
