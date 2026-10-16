//! One turn put into an agent's standing Claude Code program and read back out of its record
//! (docs/agent-control.md, 「The order for putting in one turn and waiting for it to end」). A run that is
//! nobody's standing session takes its turn the same way in a pane of its own, opened for the turn and
//! closed with it: nothing of it outlives the turn, so nothing is waited out, stood again or interrupted,
//! and what it cost is read off the account line its program writes as it ends.
//!
//! The order is the measured one and every step of it is there for a reason. The program is stood
//! first, because a run told its work on the command line cannot be told a second thing; it is waited
//! for until it is ready, because a prompt pushed at an agent that is not ready is refused; the prompt
//! goes in without a wait attached, because a wait would hold the call for the whole of the turn; and
//! the turn is over when the agent has been seen working and has stopped, because the idle it starts
//! in looks exactly like the idle it ends in. What replaces a print-mode stream is the record the
//! program writes for itself, named on the way in, and the status line's readings either side of the
//! turn; nothing is scraped off the screen, and a screen is kept only as evidence.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{FixedOffset, TimeZone};
use harness_providers::claude_code::ClaudeCode;
use herdr::{Fault, Herdr, RUN_TOKEN, TOO_LARGE};
use policy::door_words::DOOR_WORDS;
use policy::fill_marks;
use policy::resident_turn::RESIDENT_TURN;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::confirm::{Asking, Confirmation, Questions};
use super::execution::{Children, Identity, Row, cancelled, publish_envelope};
use super::guard::{socket_refusal, turn_refusal};
use super::looks::{
  GoneConfirm, Look, folder_trust_keys, is_blocked, pane_listed, prompt, prompt_with_lead, screen_of,
  send_keys, status_of,
};
use super::place_run::{
  HookRecord, ProgramLogin, begin_run, end_run, hand_run, latest_failure, program_drawn, read_hooks,
  read_run, run_variables, status_after_turn,
};
use super::record::{
  ChildReader, ModelSpend, Reply, ReplyWindow, RoundState, Tail, answered_since, ended_on_error,
  last_activity, prompted_since, read_round, read_turn, status_figures, turn_closed_since, turn_error,
  turns_owed, word_written_down,
};
use super::stand::{Interrupt, interrupt_program, person_turn_done, stand_resident, stood_again};
use crate::control::claude_accounts;
use crate::lines::place::{Place, resident_of};
use crate::now_ms;
use crate::places::Ground;
use crate::places::home::{Home, METADATA_SOURCE, short_title};
use crate::places::trust::{Trust, config_file, trust_folder};

/// What the typed lead says about the words pasted after it, where the caller says nothing else: a
/// job's and an upkeep turn's are the owner's own words.
pub fn pasted_lead() -> String {
  RESIDENT_TURN.pasted_lead.clone()
}

// The work key a pushed word's guard names: the job worker's.
const JOB_WORK_KEY: &str = "job-worker";

/// The clocks a turn keeps, as the shared policy declares them; a test shortens them.
#[derive(Clone, Debug)]
pub struct Timings {
  /// How long a program is given to come up ready to be told something; also the bound on a turn that
  /// was taken and never started anything.
  pub ready: Duration,
  /// How often the agent is asked what it is doing.
  pub poll: Duration,
  /// How long a question just answered may still stand on the screen before it is answered again.
  pub redraw: Duration,
  /// How long a person's turn working in the program is waited out before this turn goes on.
  pub person: Duration,
  /// How long the status line's reading after the turn is waited for.
  pub account: Duration,
  /// How long a pane back at rest is given to write the line that closes its turn.
  pub settle: Duration,
  /// How long a prompt that went in is given to be seen submitted, before and after `enter`.
  pub submit: Duration,
  /// How long a turn told to stop is given to end its work before it is cut short.
  pub stop_grace: Duration,
  /// How long a program at rest may leave its record unchanged before a round whose count still
  /// expects a turn is read as over.
  pub quiet: Duration,
  /// How long herdr may answer none of the turn's looks before the turn ends on that silence; none
  /// for the policy's own, which is longer on a work node, whose link may be down while its program
  /// keeps working there.
  pub patience: Option<Duration>,
  pub gone: GoneConfirm,
}

impl Default for Timings {
  fn default() -> Self {
    let declared = &*RESIDENT_TURN;
    // A test whose stand-in program is made to stay silent shortens the wait for it to come up.
    let ready = std::env::var("BMO_PANE_READY_MS")
      .ok()
      .and_then(|named| named.parse::<u64>().ok())
      .filter(|ms| *ms > 0)
      .unwrap_or(declared.ready_seconds * 1000);
    // A test whose stand-in program never writes the turn its round still owes shortens the wait for it.
    let quiet = std::env::var("BMO_TURN_QUIET_MS")
      .ok()
      .and_then(|named| named.parse::<u64>().ok())
      .filter(|ms| *ms > 0)
      .unwrap_or(declared.quiet_ms);

    Self {
      ready: Duration::from_millis(ready),
      poll: Duration::from_millis(declared.poll_ms),
      redraw: Duration::from_millis(declared.redraw_ms),
      person: Duration::from_secs(declared.person_turn_minutes * 60),
      account: Duration::from_millis(declared.account_ms),
      settle: Duration::from_millis(declared.settle_ms),
      submit: Duration::from_millis(declared.submit_ms),
      stop_grace: Duration::from_secs(declared.stop_grace_seconds),
      quiet: Duration::from_millis(quiet),
      patience: None,
      gone: GoneConfirm::default(),
    }
  }
}

/// Where a turn is put: the program standing in an agent's place, which outlives the turn, or a pane of
/// the run's own, opened for the turn in a workspace of its own on the herdr given and closed with it, for
/// a run that is nobody's standing session (docs/agent-control.md, 「One standing pane per agent」).
#[derive(Clone, Debug)]
pub enum Seat {
  Standing(Home),
  Own(Herdr),
}

impl Seat {
  /// The herdr the turn's pane is on.
  pub fn herdr(&self) -> &Herdr {
    match self {
      Self::Standing(home) => &home.herdr,
      Self::Own(herdr) => herdr,
    }
  }
}

/// One turn as its caller names it.
#[derive(Clone, Debug)]
pub struct Turn {
  /// The run's name, which its place names it by and its execution row is a child of.
  pub name: String,
  /// What the turn is, for a person watching herdr.
  pub title: String,
  /// Where the turn is put.
  pub seat: Seat,
  pub cwd: String,
  /// The command the program is stood on where it must be started for this turn.
  pub command: Vec<String>,
  pub env: BTreeMap<String, String>,
  pub prompt: String,
  /// The session the command opens or resumes, whose record the turn is read out of.
  pub session: String,
  pub identity: Identity,
  /// The line the turn runs on, whose session token the program is handed.
  pub line: String,
  /// The provider the turn goes out on, for its execution row.
  pub provider: String,
  pub lead: String,
  /// Whether the run trusts the directory it works in, which a caller says of its place.
  pub trusts_folder: bool,
  /// How long the turn may take before its caller is asked about it; none for an unbounded turn.
  pub bound: Option<Duration>,
  /// How the program's working turn is cut short.
  pub interrupt: Interrupt,
  pub timings: Timings,
}

/// What became of a word said to a turn that was already going: `Delivered`, the run has it; `Waiting`,
/// the one refusal that is not a fault — the run sits on a question a person has to answer, and asking
/// again in the same turn meets the same refusal; `Refused`, anything else, and the words stay unsent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsideOutcome {
  Delivered,
  Waiting,
  Refused,
}

/// What a turn past its bound tells its caller: when it started, its bound, and the last thing its
/// record says it did.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overrun {
  pub started_at: i64,
  pub bound_ms: i64,
  pub last_activity_at: Option<i64>,
  pub last_activity: Option<String>,
}

/// What a caller answers a turn past its bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Overtime {
  /// The caller asks nothing at the bound, and the turn is cut there.
  NotAsked,
  Cut,
  /// Go on, and ask again at this moment.
  AskAgainAt(i64),
  /// The one asked could not answer, and the turn is cut as a turn asked nothing is.
  Failed(String),
}

/// What a turn's caller is told while the turn runs and may say into it: the words it still has for
/// the working turn, what became of them, what to do at the bound, and the notes a log would keep.
/// Every method has the answer a caller with nothing to say gives.
pub trait Watch {
  /// The execution row the turn runs under, once it is open: the name a stop of the turn is aimed at.
  fn opened(&mut self, _execution: &str) -> impl Future<Output = ()> + Send {
    async {}
  }

  /// Words to say to the turn while it works, asked on every look at a working program.
  fn words(&mut self) -> impl Future<Output = Option<String>> + Send {
    async { None }
  }

  /// What became of the words last handed over.
  fn said(&mut self, _outcome: AsideOutcome) -> impl Future<Output = ()> + Send {
    async {}
  }

  /// Asked each time the turn's bound passes.
  fn overtime(&mut self, _asked: &Overrun) -> impl Future<Output = Overtime> + Send {
    async { Overtime::NotAsked }
  }

  /// A line for whoever reads the run afterwards: a screen kept as evidence, or what the turn did.
  fn note(&mut self, _text: &str) -> impl Future<Output = ()> + Send {
    async {}
  }

  /// Whether a question the program stops on is put to this caller rather than left for a person.
  fn puts_questions(&self) -> bool {
    false
  }

  /// A question the program stopped on in the middle of the turn, put to the caller, which answers while
  /// the turn is watched on; none where it puts nothing to anybody, and the question stands for a person.
  fn confirm(&mut self, _asked: &Confirmation) -> impl Future<Output = Option<Asking>> + Send {
    async { None }
  }

  /// What the record gained since it was last handed over, from the moment the prompt was seen taken:
  /// every line as the program wrote it, for a run log to read as it reads a stream.
  fn record(&mut self, _lines: &[String]) -> impl Future<Output = ()> + Send {
    async {}
  }

  /// Whether the caller has been told to stop the run, asked on every look at it: a run told so is stopped
  /// as a cancellation over its names stops it.
  fn stopping(&mut self) -> bool {
    false
  }
}

/// A caller with nothing to say and nothing to keep.
pub struct Quiet;

impl Watch for Quiet {}

/// What one turn came to.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
pub enum Ending {
  /// The ordinary ending.
  Answered { reply: Box<Reply>, records: usize },
  /// The program is asking a person something and nothing was delivered.
  Blocked { screen: String },
  /// The program would not get past the folder-trust question, trusted and stood up again.
  Untrusted {
    screen: String,
    folder: String,
    config: String,
  },
  /// The pane went away under the run.
  Vanished { screen: String },
  /// herdr answered none of the turn's looks for the whole of its patience.
  Unanswered { screen: String },
  /// The turn did not finish inside its bound.
  Overran { screen: String },
  /// The turn came back to rest with no answer in its record.
  Silent { screen: String },
  /// The turn's last word was the program's own error in the answer's place, or a failed request ended it:
  /// the error, and its kind where the program names one.
  Errored {
    error: String,
    // Read where the turn ends, never written out: what an ending is written as stays the same for every
    // reader.
    #[serde(skip)]
    kind: Option<String>,
  },
  /// The prompt never became a turn, and why.
  Unsubmitted {
    screen: String,
    // Read where the turn ends, never written out, like the error's kind above.
    #[serde(skip)]
    why: String,
  },
  /// Ended from outside: a cancellation stood over this run's name or its root.
  Stopped { screen: String },
}

impl Ending {
  /// The word the ending goes by.
  pub fn word(&self) -> &'static str {
    match self {
      Self::Answered { .. } => "answered",
      Self::Blocked { .. } => "blocked",
      Self::Untrusted { .. } => "untrusted",
      Self::Vanished { .. } => "vanished",
      Self::Unanswered { .. } => "unanswered",
      Self::Overran { .. } => "overran",
      Self::Silent { .. } => "silent",
      Self::Errored { .. } => "errored",
      Self::Unsubmitted { .. } => "unsubmitted",
      Self::Stopped { .. } => "stopped",
    }
  }
}

/// How a turn ended, for its execution row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
  Exited,
  Timeout,
  Cancelled,
}

impl End {
  fn word(self) -> &'static str {
    match self {
      Self::Exited => "exited",
      Self::Timeout => "timeout",
      Self::Cancelled => "cancelled",
    }
  }
}

/// The first line of a prompt put into a resident program: which run it is, what it is and when it
/// went in, on the deployment's clock (UTC+9).
pub fn run_header(name: &str, title: &str, at: i64) -> String {
  let seoul = FixedOffset::east_opt(9 * 3600).expect("a fixed offset");
  let clock = seoul
    .timestamp_millis_opt(at)
    .single()
    .map_or_else(String::new, |at| at.format("%H:%M").to_string());

  format!("[{name} · {title} · {clock}]")
}

/// The login a program runs under, as its environment names it: the configuration home it reads its
/// login from, and a fingerprint of a token it was handed instead — a digest cut short, from which no
/// token can be made back.
pub fn program_login(machine_config_dir: &Path, env: &BTreeMap<String, String>) -> ProgramLogin {
  ProgramLogin {
    config_dir: env
      .get("CLAUDE_CONFIG_DIR")
      .filter(|dir| !dir.is_empty())
      .cloned()
      .unwrap_or_else(|| machine_config_dir.to_string_lossy().into_owned()),
    token: env
      .get("CLAUDE_CODE_OAUTH_TOKEN")
      .filter(|token| !token.trim().is_empty())
      .map(|token| fingerprint(token)),
  }
}

fn fingerprint(token: &str) -> String {
  format!("{:x}", Sha256::digest(token.trim().as_bytes()))[..16].to_owned()
}

/// The registered account a program's login belongs to: a token account by its token alone, any other
/// by the configuration home it holds, and where it was handed a token beside it — a program on a work
/// node — only where that token is the account's token for the nodes, kept under the data root named.
pub fn account_of_login(
  accounts: &[claude_accounts::Account],
  login: &ProgramLogin,
  data_root: &Path,
) -> Option<String> {
  let held = |path: &Path| std::fs::read_to_string(path).ok().map(|held| fingerprint(&held));

  if let Some(token) = &login.token
    && let Some(account) = accounts.iter().find(|account| {
      account.credential_kind == "token"
        && held(Path::new(&account.credential_path)).as_ref() == Some(token)
    })
  {
    return Some(account.id.clone());
  }

  let home = std::path::absolute(&login.config_dir).unwrap_or_else(|_| PathBuf::from(&login.config_dir));

  accounts
    .iter()
    .find(|account| {
      account.credential_kind == "directory"
        && std::path::absolute(&account.credential_path)
          .unwrap_or_else(|_| PathBuf::from(&account.credential_path))
          == home
        && login
          .token
          .as_ref()
          .is_none_or(|token| held(&account.node_token_file(data_root)).as_ref() == Some(token))
    })
    .map(|account| account.id.clone())
}

// One turn's state as it is watched.
struct Door<'a, W: Watch> {
  ground: &'a Ground,
  turn: &'a Turn,
  watch: &'a mut W,
  herdr: &'a Herdr,
  // The standing program's place and home; none for a turn in a pane of its own.
  standing: Option<(Place, &'a Home)>,
  tail: Tail,
  row: Row,
  children: Children,
  reader: ChildReader,
  observed: usize,
  // Where the record was last handed to the watch, once the prompt was seen taken.
  fed: Option<usize>,
  questions: Questions,
  trust_config: PathBuf,
}

impl<W: Watch> Door<'_, W> {
  fn lines(&mut self) -> Vec<Value> {
    self.tail.look()
  }

  async fn cancelled(&self) -> bool {
    let names = [self.turn.name.as_str(), self.row.id(), self.row.root()];

    cancelled(&self.ground.db, &names).await.unwrap_or(false)
  }

  // What a launch asks first, each time: not cancelled, and the guard's word.
  async fn check_launch(&self) -> Result<(), String> {
    if self.cancelled().await {
      return Err("execution cancelled".to_owned());
    }

    match turn_refusal(&self.ground.db, "pane", &self.turn.env).await {
      Some(why) => Err(why),
      None => Ok(()),
    }
  }

  async fn save(&self) {
    if let Err(why) = self.row.save(&self.ground.db).await {
      eprintln!(
        "{}: the execution row ({}): {why}",
        self.turn.name,
        self.row.text("state").unwrap_or("")
      );
    }
  }

  // The row closed with how the turn ended: the envelope first, so a collector killed before the
  // archive's write still leaves the cause behind.
  async fn close(&mut self, state: &str) {
    publish_envelope(&self.ground.roots.data, self.row.id(), Some(state));
    self.row.set("state", json!(state));
    self.row.set("endedAt", json!(now_ms()));
    self.save().await;
  }

  // What the record's account line written after `from` says the run spent, waited for as long as given;
  // none where it does not come.
  async fn account_line(&mut self, from: usize, wait: Duration) -> Option<Vec<ModelSpend>> {
    let by = tokio::time::Instant::now() + wait;

    loop {
      let records = self.lines();

      if records
        .iter()
        .skip(from)
        .any(|record| record.get("type").and_then(Value::as_str) == Some(COST_STATE))
      {
        return spend_since(&records, from);
      }

      if tokio::time::Instant::now() >= by {
        return None;
      }

      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  }

  async fn finish_children(&mut self) {
    let row = self.row.clone();

    if let Err(why) = self.children.finish(&self.ground.db, &row).await {
      eprintln!("{}: the child runs' rows: {why}", self.turn.name);
    }
  }

  // The row of a turn whose own pane its collector has closed: one whose prompt never went in did not
  // launch; one whose pane herdr says is gone ended the way the turn read it; one herdr still holds, or
  // does not answer about, may still be working, so it stays open and unknown.
  async fn settle_own(&mut self, pane: &str, attempted: bool, ending: End) {
    if !attempted {
      self.close("launch-error").await;
      return;
    }

    if pane_listed(self.herdr, pane).await == Some(false) {
      self.close(ending.word()).await;
    } else {
      self.row.set("state", json!("unknown"));
      self.save().await;
    }
  }

  async fn kept_screen(&mut self, why: &str, screen: String) -> String {
    let text = if screen.is_empty() { why.to_owned() } else { format!("{why}\n\n{screen}") };

    self.watch.note(&text).await;
    screen
  }

  async fn screen(&self, pane: &str) -> String {
    screen_of(self.herdr, pane).await
  }

  async fn record_children(&mut self) {
    let records = self.lines();

    for record in records.iter().skip(self.observed) {
      let said = {
        let children = &self.children;

        self.reader.read(record, |id| children.known(id))
      };

      for evidence in said {
        if let Err(why) = self.children.update(&self.ground.db, &self.row, &evidence).await {
          eprintln!("{}: a child run's row: {why}", self.turn.name);
        }
      }
    }

    self.observed = records.len();
    self.feed().await;
  }

  // What the record gained handed to the watch, once the turn is under way.
  async fn feed(&mut self) {
    let Some(at) = self.fed else {
      return;
    };
    self.tail.look();

    let lines = self.tail.raw();

    if lines.len() > at {
      self.watch.record(&lines[at..]).await;
    }

    self.fed = Some(lines.len().max(at));
  }

  // A run that trusts its folder has the folder written as trusted, in the file its own login reads,
  // before any program comes up in it, so the question never stands.
  async fn trust_here(&mut self) {
    if !self.turn.trusts_folder {
      return;
    }

    let config = self.trust_config.display().to_string();
    let cwd = self.turn.cwd.clone();
    let trust = if Path::new(&cwd).is_absolute() {
      trust_folder(&self.trust_config, &cwd)
    } else {
      Trust::Unreadable
    };

    match trust {
      Trust::Trusted => {}
      Trust::Written => {
        self
          .watch
          .note(&fill_marks(
            &DOOR_WORDS.notes.trust_written,
            &[("config", &config.to_string()), ("cwd", &cwd.to_string())],
          ))
          .await;
      }
      Trust::NoConfigDir | Trust::Unreadable | Trust::Busy => {
        let word = match trust {
          Trust::NoConfigDir => "no-config-dir".to_owned(),
          Trust::Busy => "busy".to_owned(),
          _ if Path::new(&cwd).is_absolute() => "unreadable".to_owned(),
          _ => format!("Error: folder trust is kept by absolute path, not {cwd}"),
        };

        self
          .watch
          .note(&fill_marks(
            &DOOR_WORDS.notes.trust_unwritten,
            &[("config", &config.to_string()), ("cwd", &cwd.to_string()), ("why", &word.to_string())],
          ))
          .await;
      }
    }
  }

  // The standing program's working turn cut short, or the program ended; a pane of the turn's own is closed
  // with the turn instead, so nothing is done to it here.
  async fn interrupt(&self, pane: &str, interrupt: &Interrupt) {
    if let Some((place, home)) = &self.standing {
      interrupt_program(place, home, pane, interrupt).await;
    }
  }

  // Whether the place's run is marked as handed words the bot put in; a pane of the turn's own has no
  // place.
  fn hand(&self, handed: bool) {
    if let Some((place, _)) = &self.standing {
      hand_run(&place.path, handed);
    }
  }

  // The failure a failed request ended the program's latest turn with since a moment, where it ended so; a
  // pane of the turn's own has no place, and its record alone says how its turn ended.
  fn failure_since(&self, since: i64) -> Option<HookRecord> {
    self.standing.as_ref().and_then(|(place, _)| latest_failure(&place.path, since))
  }

  // Bringing a wedged program back to rest: it is ended, its loop starts it again on the same session
  // with an empty input box, and the place is watched until the program standing is a different one,
  // because until its record says so a prompt put in would go into the program just ended. A pane of the
  // turn's own has nothing standing to reset.
  async fn stand_again(&mut self, pane: &str, why: &str) -> bool {
    let Some((place, home)) = &self.standing else { return false };

    if self.cancelled().await {
      return false;
    }

    let was = place.running();

    self.watch.note(&fill_marks(&DOOR_WORDS.notes.stand_again, &[("why", why)])).await;
    interrupt_program(place, home, pane, &Interrupt::EndProgram).await;

    let ground = self.ground;
    let names = [self.turn.name.clone(), self.row.id().to_owned(), self.row.root().to_owned()];

    stood_again(place, was.as_ref(), self.turn.timings.ready, self.turn.timings.poll, || async {
      let names: Vec<&str> = names.iter().map(String::as_str).collect();

      cancelled(&ground.db, &names).await.unwrap_or(false)
    })
    .await
  }

  // Waiting for the program to be ready to be told something, keeping what its screen said meanwhile —
  // the most worth reading is said by a program about to be gone. The folder-trust question a run that
  // trusts its folder may answer is answered here, a bounded number of times, and the wait goes on.
  async fn settled(&mut self, pane: &str) -> Result<(Look, String), String> {
    let timings = self.turn.timings.clone();
    let deadline = tokio::time::Instant::now() + timings.ready;
    let mut screen = String::new();
    let mut answers = 0;
    let mut answered_at: Option<tokio::time::Instant> = None;

    loop {
      self.check_launch().await?;

      let now = status_of(self.herdr, pane, timings.gone).await;

      if now == Look::Gone {
        return Ok((now, screen));
      }

      if matches!(now, Look::Silent(_)) {
        if tokio::time::Instant::now() >= deadline {
          return Ok((now, screen));
        }

        tokio::time::sleep(timings.poll).await;
        continue;
      }

      let seen = self.screen(pane).await;

      if !seen.trim().is_empty() {
        screen.clone_from(&seen);
      }

      // A program started again for this turn reads at rest from the one before it until it is up, and
      // words put in meanwhile are lost whole. A pane of the turn's own has only its own program.
      if now.at_rest() && self.standing.as_ref().is_none_or(|(place, _)| program_drawn(place)) {
        return Ok((now, screen));
      }

      if tokio::time::Instant::now() >= deadline {
        return Ok((now, screen));
      }

      let due = answered_at.is_none_or(|at| at.elapsed() >= timings.redraw);

      if now.is("blocked")
        && answers < RESIDENT_TURN.trust_answers
        && due
        && self.turn.trusts_folder
        && let Some(keys) = folder_trust_keys(&seen)
        && send_keys(self.herdr, pane, &keys).await.is_ok()
      {
        self.watch.note(&DOOR_WORDS.notes.trusted_folder).await;
        answers += 1;
        answered_at = Some(tokio::time::Instant::now());
      }

      tokio::time::sleep(timings.poll).await;
    }
  }

  // Whether a prompt put in was submitted: the record gained a person's prompt after the turn began, or
  // the pane went to work, within the wait. A pane gone, or stopped on a question, counts as taken, so
  // the turn's own watching reads what it is doing.
  async fn taken(&mut self, pane: &str, from: usize) -> Submit {
    let by = tokio::time::Instant::now() + self.turn.timings.submit;

    loop {
      if prompted_since(&self.lines(), from) {
        return Submit::Seen;
      }

      let now = status_of(self.herdr, pane, GoneConfirm::default()).await;

      if now.is("working") {
        return Submit::Working;
      }

      if now == Look::Gone || now.is("blocked") {
        return Submit::Seen;
      }

      if tokio::time::Instant::now() >= by {
        return Submit::Unsent;
      }

      tokio::time::sleep(self.turn.timings.poll).await;
    }
  }

  // A word pushed into the working turn. herdr's working can trail the turn's real end by a look, and
  // making the push takes seconds more, so whether the turn still works is read again just before the
  // keys go: off the place, whose stop hook closes the run at the end, and off the record. The run is
  // marked as handed the word first, so an end landing between that reading and the keys still finds
  // the word the bot's; a push that does not go in takes the mark back, and one that does is waited on
  // until the record holds it.
  async fn push_word(&mut self, pane: &str, from: usize, text: &str) -> Result<Value, Fault> {
    self.hand(true);

    let run = self.standing.as_ref().and_then(|(place, _)| read_run(&place.path));
    let records = self.lines();
    let over = run.is_some_and(|run| run.person || run.ended_at.is_some())
      || read_round(records.get(from..).unwrap_or_default()).state != RoundState::Working;
    let at = records.len();
    let said = if over {
      Err(Fault {
        code: "turn_ended".to_owned(),
        message: "the turn ended before the words could go in".to_owned(),
      })
    } else {
      prompt_with_lead(self.herdr, pane, &self.turn.lead, text).await
    };

    if said.is_err() {
      self.hand(false);

      return said;
    }

    let by = tokio::time::Instant::now() + self.turn.timings.submit;

    while !word_written_down(&self.lines(), at) && tokio::time::Instant::now() < by {
      tokio::time::sleep(self.turn.timings.poll).await;
    }

    said
  }

  fn round_state(&mut self, from: usize) -> RoundState {
    let records = self.lines();

    read_round(records.get(from..).unwrap_or_default()).state
  }
}

// Whether a prompt put in was seen submitted.
enum Submit {
  Working,
  Seen,
  Unsent,
}

/// One turn, start to finish, in the agent's standing program or in a pane of its own opened for it. An
/// error is a turn that never got under way or was thrown out of — cancelled before it went in, refused by
/// the guard, a pane herdr would not take — and its row says so.
pub async fn put<W: Watch>(ground: &Ground, turn: &Turn, watch: &mut W) -> Result<Ending, String> {
  let herdr = turn.seat.herdr();

  if let Some(why) = socket_refusal(herdr.socket()) {
    return Err(why);
  }

  let machine = ground.machine();
  let machine_config = ClaudeCode::machine_config_dir(&machine);
  let records_dir = turn
    .env
    .get("CLAUDE_CONFIG_DIR")
    .filter(|dir| !dir.is_empty())
    .map_or_else(|| machine_config.clone(), PathBuf::from);
  let record = ClaudeCode::records_under(&records_dir, Path::new(&turn.cwd), &turn.session).swap_remove(0);
  let trust_config = config_file(
    &turn
      .env
      .get("CLAUDE_CONFIG_DIR")
      .filter(|dir| !dir.is_empty())
      .map_or_else(|| PathBuf::from(machine.var("HOME").unwrap_or_default()), PathBuf::from),
  );
  let mut tail = Tail::new(&record);
  let from = tail.count();
  // A turn of a standing program is for the agent whose program it is, where the caller names none.
  let identity = Identity {
    agent: turn.identity.agent.clone().or_else(|| match &turn.seat {
      Seat::Standing(home) => Some(home.agent.clone()),
      Seat::Own(_) => None,
    }),
    ..turn.identity.clone()
  };
  let row = Row::begin(
    &ground.db,
    &turn.provider,
    &turn.line,
    &turn.name,
    Some(&turn.session),
    &turn.command,
    &identity,
  )
  .await
  .map_err(|why| why.detail)?;

  publish_envelope(&ground.roots.data, row.id(), None);
  watch.opened(row.id()).await;

  let standing = match &turn.seat {
    Seat::Standing(home) => Some((Place::of(&ground.roots.data, &home.agent), home)),
    Seat::Own(_) => None,
  };
  let mut door = Door {
    ground,
    turn,
    watch,
    herdr,
    standing,
    tail,
    row,
    children: Children::default(),
    reader: ChildReader::default(),
    observed: from,
    fed: None,
    questions: Questions::new(turn.timings.redraw),
    trust_config,
  };
  let opened = match &turn.seat {
    Seat::Standing(home) => stand(&mut door, home).await,
    Seat::Own(_) => open_own(&mut door).await,
  };
  let pane = match opened {
    Ok(pane) => pane,
    Err(why) => {
      door.close("launch-error").await;

      return Err(why);
    }
  };

  door.row.set("paneId", json!(pane));

  // A turn of a standing program says so on its row, with where its record is read from and which process
  // the program was, since the pane outlives the turn and cannot be what closes it.
  if let Some((place, _)) = &door.standing {
    let running = place.running();
    let resident = json!({
      "place": place.path,
      "record": record,
      "from": from,
      "pid": running.as_ref().map(|running| running.pid),
      "birth": running.as_ref().and_then(|running| running.birth.clone()),
    });

    door.row.set("resident", resident);
  }

  let mut attempted = false;
  let mut ending = End::Exited;
  let watched = watch_turn(&mut door, &pane, from, &mut attempted, &mut ending).await;
  let watched = match watched {
    Ok(done) => Ok(done),
    Err(why) => {
      ending = End::Cancelled;
      Err(why)
    }
  };

  door.feed().await;
  // A question still being put is about a pane that is going away.
  door.questions.withdraw();

  match &turn.seat {
    Seat::Standing(home) => {
      // The program and its pane stay for the next turn; the run it was ends here, and the place says so
      // to the program's status line.
      if let Some((place, _)) = &door.standing {
        let _ = end_run(&place.path, now_ms());
      }

      if !door.row.ended() {
        door.close(if attempted { ending.word() } else { "launch-error" }).await;
      }

      door.finish_children().await;
      // A label that named the run goes back to what the agent is, once the row is closed.
      home.rest(&pane).await;
    }
    Seat::Own(_) => {
      // Closed before the archive's writes, so a broken archive never strands the program.
      let _ = herdr.call("pane.close", json!({ "pane_id": pane }), None).await;

      if !door.row.ended() {
        door.settle_own(&pane, attempted, ending).await;
      }

      door.finish_children().await;
    }
  }

  watched
}

// Where a standing program is stood for the turn: a person's turn working in it waited out first, before
// anything is stood, replaced or reset; then the program the turn needs, whose pane is answered.
async fn stand<W: Watch>(door: &mut Door<'_, W>, home: &Home) -> Result<String, String> {
  let ground = door.ground;
  let turn = door.turn;
  let place = Place::of(&ground.roots.data, &home.agent);
  let program =
    resident_of(&ground.machine(), &place.path, &turn.cwd, &turn.command, &turn.env, &turn.session);

  door.check_launch().await?;
  door.trust_here().await;

  let waited = {
    let door = &*door;

    person_turn_done(&place, home, turn.timings.person, turn.timings.poll, || door.check_launch()).await?
  };

  if waited {
    door.watch.note(&DOOR_WORDS.notes.person_turn).await;
  }

  stand_resident(
    ground,
    &place,
    home,
    &program,
    &turn.line,
    turn.timings.ready,
    Duration::from_millis(250),
  )
  .await
  .map(|stood| stood.pane)
  .map_err(|fault| format!("herdr would not take {}: {}: {}", turn.name, fault.code, fault.message))
}

// A pane of the turn's own, in a workspace of its own labelled with what the run is, on what model and on
// which attempt, its pane named with the run's execution as its token.
async fn open_own<W: Watch>(door: &mut Door<'_, W>) -> Result<String, String> {
  door.check_launch().await?;
  door.trust_here().await;

  let turn = door.turn;
  let model = door.row.text("model").unwrap_or("claude-code").to_owned();
  let attempt = door.row.body.get("attempt").and_then(Value::as_i64).unwrap_or(1);
  let label = short_title(
    &fill_marks(
      &DOOR_WORDS.notes.title,
      &[("title", &turn.title), ("model", &model), ("attempt", &attempt.to_string())],
    ),
    TITLE_WIDTH,
  );

  open_pane(door.herdr, door.row.id(), &label, turn)
    .await
    .map_err(|fault| format!("herdr would not take {}: {}: {}", turn.name, fault.code, fault.message))
}

// The record line a program writes its running account into as it ends.
const COST_STATE: &str = "cost-state";

/// What a run spent since `from`, by the record's account lines: the last one after it less the last one
/// before it, model by model, each kind never below nought; none where no account line follows `from` or
/// nothing moved.
pub fn spend_since(records: &[Value], from: usize) -> Option<Vec<ModelSpend>> {
  let usage_at = |records: &mut dyn Iterator<Item = &Value>| {
    records
      .filter(|record| record.get("type").and_then(Value::as_str) == Some(COST_STATE))
      .last()
      .map(|record| record.get("modelUsage").cloned().unwrap_or(Value::Null))
  };
  let after = usage_at(&mut records.iter().skip(from))?;
  let before = usage_at(&mut records.iter().take(from)).unwrap_or(Value::Null);
  let number = |usage: Option<&Value>, name: &str| {
    usage.and_then(|usage| usage.get(name)).and_then(Value::as_f64).unwrap_or(0.0)
  };
  let moved: Vec<ModelSpend> = after
    .as_object()
    .into_iter()
    .flatten()
    .filter_map(|(model, total)| {
      let earlier = before.get(model);
      let less = |name: &str| (number(Some(total), name) - number(earlier, name)).max(0.0);
      let (input, output) = (less("inputTokens"), less("outputTokens"));
      let (read, write) = (less("cacheReadInputTokens"), less("cacheCreationInputTokens"));
      let cost = less("costUSD");

      [input, output, read, write, cost].iter().any(|value| *value > 0.0).then(|| {
        let (input, output, read, write) = (input as i64, output as i64, read as i64, write as i64);

        ModelSpend {
          mode: "claude-code".to_owned(),
          model: model.clone(),
          tokens: input + output + read + write,
          input_tokens: input,
          output_tokens: output,
          cache_read_tokens: read,
          cache_write_tokens: write,
          cost,
        }
      })
    })
    .collect();

  (!moved.is_empty()).then_some(moved)
}

// How wide a workspace's label is kept, as a sidebar can show it.
const TITLE_WIDTH: usize = 48;

// The workspace made and the run laid out alone in its tab, so no shell is left standing beside it to keep
// the workspace up after the run; a workspace that ends up with nothing running in it, and a pane that
// cannot be named, are closed rather than left behind with nobody to close them.
async fn open_pane(herdr: &Herdr, name: &str, label: &str, turn: &Turn) -> Result<String, Fault> {
  let made = herdr
    .call("workspace.create", json!({ "label": label, "cwd": turn.cwd, "env": turn.env }), None)
    .await?;
  let (Some(workspace), Some(tab)) = (
    made.pointer("/workspace/workspace_id").and_then(Value::as_str),
    made.pointer("/tab/tab_id").and_then(Value::as_str),
  ) else {
    return Err(Fault {
      code: "unreadable".to_owned(),
      message: "workspace.create named no workspace and tab".to_owned(),
    });
  };
  let root = json!({ "type": "pane", "command": turn.command, "cwd": turn.cwd, "env": turn.env });
  let applied = herdr.call("layout.apply", json!({ "tab_id": tab, "root": root }), None).await;
  let pane = applied
    .as_ref()
    .ok()
    .and_then(|applied| applied.pointer("/layout/root/pane_id"))
    .and_then(Value::as_str)
    .map(str::to_owned);
  let Some(pane) = pane else {
    let _ = herdr.call("workspace.close", json!({ "workspace_id": workspace }), None).await;

    return Err(applied.err().unwrap_or_else(|| Fault {
      code: "unreadable".to_owned(),
      message: "layout.apply named no pane".to_owned(),
    }));
  };
  let named = herdr
    .call(
      "pane.report_metadata",
      json!({ "pane_id": pane, "source": METADATA_SOURCE, "tokens": { RUN_TOKEN: name } }),
      None,
    )
    .await;

  if let Err(fault) = named {
    let _ = herdr.call("pane.close", json!({ "pane_id": pane }), None).await;

    return Err(fault);
  }

  Ok(pane)
}

async fn watch_turn<W: Watch>(
  door: &mut Door<'_, W>,
  pane: &str,
  opened_from: usize,
  attempted: &mut bool,
  ending: &mut End,
) -> Result<Ending, String> {
  let turn = door.turn;
  let timings = turn.timings.clone();
  let every = timings.poll;
  // Where this turn's own lines begin in the record, read again as the prompt goes in: a program
  // started again for this turn writes lines of its own on the way up.
  let mut from = opened_from;
  let mut prompted_at;

  door.save().await;

  // One reset and one retry per turn, never a loop.
  let mut reset = false;

  loop {
    let (ready, screen) = door.settled(pane).await?;

    if ready == Look::Gone {
      *ending = End::Cancelled;

      let screen = door.kept_screen(&DOOR_WORDS.screens.vanished, screen).await;

      return Ok(Ending::Vanished { screen });
    }

    if matches!(ready, Look::Silent(_)) {
      door.watch.note(&DOOR_WORDS.notes.herdr_silent_ready).await;
      *ending = End::Cancelled;

      return Ok(Ending::Unanswered {
        screen: "herdr did not answer while the turn waited for its program".to_owned(),
      });
    }

    if !ready.at_rest() {
      // A question nobody is coming to answer, or no state at all: a wedge, given its one reset. The
      // folder-trust question in a run that trusts its folder is nobody's to answer: the trust is
      // written again and the program stood up again, and where even that leaves it standing the turn
      // names it.
      let untrusted = turn.trusts_folder && ready.is("blocked") && folder_trust_keys(&screen).is_some();
      // A question is left standing for whoever is to answer it, and only a turn that can put one to
      // somebody has such a question; for any other turn a blocked program is a wedge like any other.
      let asks_somebody = ready.is("blocked") && door.watch.puts_questions() && !untrusted;
      let shown = if screen.is_empty() { door.screen(pane).await } else { screen };
      let screen = door.kept_screen(&DOOR_WORDS.screens.not_ready, shown).await;

      if !reset && !asks_somebody {
        reset = true;

        if untrusted {
          door.trust_here().await;
        }

        let why = if untrusted { &DOOR_WORDS.notes.untrusted } else { &DOOR_WORDS.notes.not_ready };

        if door.stand_again(pane, why).await {
          continue;
        }
      }

      *ending = End::Cancelled;

      return Ok(if untrusted {
        Ending::Untrusted {
          screen,
          folder: turn.cwd.clone(),
          config: door.trust_config.display().to_string(),
        }
      } else {
        Ending::Blocked { screen }
      });
    }

    if door.cancelled().await {
      return Err("execution cancelled".to_owned());
    }

    door.check_launch().await?;
    *attempted = true;
    // The run this turn is goes into the place before the prompt, and the prompt's first line names it
    // where the screen shows it. A command of the program's own is the whole of what it is told, since
    // the program reads it as a command only when nothing stands before it.
    prompted_at = now_ms();

    if let Some((place, _)) = &door.standing
      && let Err(why) =
        begin_run(&place.path, &turn.name, &turn.title, prompted_at, run_variables(&turn.env))
    {
      eprintln!("{}: the place's run: {why}", turn.name);
    }

    from = from.max(door.lines().len());

    if let Some(resident) = door.row.body.get_mut("resident") {
      resident["from"] = json!(from);
    }

    let sent = if turn.prompt.starts_with('/') {
      prompt(door.herdr, pane, &turn.prompt).await
    } else if door.standing.is_some() {
      let lead = format!("{} {}", run_header(&turn.name, &turn.title, prompted_at), turn.lead);

      prompt_with_lead(door.herdr, pane, &lead, &turn.prompt).await
    } else {
      // A pane of the turn's own runs nothing else, so its first line needs no name of the run.
      prompt_with_lead(door.herdr, pane, &turn.lead, &turn.prompt).await
    };

    if let Err(fault) = sent {
      let shown = door.screen(pane).await;
      let screen = door
        .kept_screen(
          &fill_marks(
            &DOOR_WORDS.screens.not_taken,
            &[("code", &fault.code.to_string()), ("message", &fault.message)],
          ),
          shown,
        )
        .await;

      // A refusal that is a question somebody is to be asked is left standing like the one above.
      if is_blocked(&fault) && door.watch.puts_questions() {
        *ending = End::Cancelled;

        return Ok(Ending::Blocked { screen });
      }

      // Words too long for herdr are refused the same way however often the program is stood up again.
      if !reset && fault.code != TOO_LARGE {
        reset = true;

        if door.stand_again(pane, &DOOR_WORDS.notes.not_taken).await {
          continue;
        }
      }

      // Refused for good, the words never became a turn. The lead typed before the refusal is still in
      // the box, so the program is started again with it empty.
      door.interrupt(pane, &Interrupt::EndProgram).await;
      *ending = End::Cancelled;

      return Ok(Ending::Unsubmitted {
        screen,
        why: fault.to_string(),
      });
    }

    break;
  }

  // Accepted is not submitted: pasted words can stand in the input box unsent, and a program at rest
  // with words in its box reads exactly like one that has finished.
  let mut taken = door.taken(pane, from).await;

  if matches!(taken, Submit::Unsent) {
    door.watch.note(&DOOR_WORDS.notes.enter_pressed).await;
    let _ = send_keys(door.herdr, pane, &["enter"]).await;
    taken = door.taken(pane, from).await;

    if matches!(taken, Submit::Unsent) {
      let shown = door.screen(pane).await;

      door.watch.note(&DOOR_WORDS.notes.unsubmitted).await;
      // The words left in the box would go out in front of the next turn's prompt.
      door.interrupt(pane, &Interrupt::EndProgram).await;
      *ending = End::Cancelled;

      let screen = door.kept_screen(&DOOR_WORDS.screens.unsubmitted, shown).await;

      return Ok(Ending::Unsubmitted {
        screen,
        why: "the words stood unsent in the input box after Enter".to_owned(),
      });
    }
  }

  door.row.set("state", json!("running"));
  door.save().await;
  door.fed = Some(from);

  let mut worked = matches!(taken, Submit::Working);
  let mut deadline = turn.bound.map(|bound| now_ms() + bound.as_millis() as i64);
  // A prompt taken that never started anything, given as long as a program is given to come up, is a
  // turn that came back silent rather than one to wait on for ever.
  let mut unstarted_by = now_ms() + timings.ready.as_millis() as i64;
  let mut answer_from = from;
  // The moment since which a failed request's end is this turn's, moved with the words owed after a
  // refusal.
  let mut failed_from = prompted_at;
  let mut stop_asked_at: Option<i64> = None;
  let mut silent_since: Option<i64> = None;
  let patience = timings.patience.unwrap_or_else(|| {
    Duration::from_secs(if door.standing.as_ref().and_then(|(place, _)| place.node()).is_some() {
      RESIDENT_TURN.node_patience_seconds
    } else {
      RESIDENT_TURN.herdr_patience_seconds
    })
  });

  loop {
    // Stopped from outside while the turn was running. A job is asked to end its work first and cut
    // short with the surface's interrupt once the grace is spent while it still works; a chat round has
    // no work of its own to wind down and is cut short at once. A pane of the turn's own is closed with
    // the turn, which ends its program.
    if door.watch.stopping() || door.cancelled().await {
      let asks_first = turn.line != "chat";

      if door.standing.is_some() && stop_asked_at.is_none() {
        stop_asked_at = Some(now_ms());

        if asks_first {
          if let Some(why) = turn_refusal(&door.ground.db, JOB_WORK_KEY, &turn.env).await {
            return Err(why);
          }

          let _ =
            prompt_with_lead(door.herdr, pane, &turn.lead, &policy::run_stop::RUN_STOP.instruction).await;
          tokio::time::sleep(every).await;
          continue;
        }
      }

      if let Some(asked) = stop_asked_at {
        let standing = status_of(door.herdr, pane, GoneConfirm::default()).await;

        if asks_first && now_ms() - asked < timings.stop_grace.as_millis() as i64 && standing.is("working")
        {
          tokio::time::sleep(every).await;
          continue;
        }

        if standing.is("working") {
          door.interrupt(pane, &turn.interrupt).await;
          // The row closes as a stop on the interrupted marker the program writes, looked for a bounded
          // while.
          let settle_by = now_ms() + timings.settle.as_millis() as i64;

          while door.round_state(from) == RoundState::Working && now_ms() < settle_by {
            tokio::time::sleep(every).await;
          }
        }
      }

      *ending = End::Cancelled;

      let shown = door.screen(pane).await;
      let screen = door.kept_screen(&DOOR_WORDS.screens.stopped, shown).await;

      return Ok(Ending::Stopped { screen });
    }

    if let (Some(at), Some(bound)) = (deadline, turn.bound)
      && now_ms() >= at
    {
      let activity = last_activity(&door.lines(), from);
      let asked = Overrun {
        started_at: prompted_at,
        bound_ms: bound.as_millis() as i64,
        last_activity_at: activity.at,
        last_activity: activity.what,
      };

      match door.watch.overtime(&asked).await {
        Overtime::AskAgainAt(again) => deadline = Some(again.max(now_ms() + 1)),
        Overtime::Failed(why) => {
          door
            .watch
            .note(&fill_marks(&DOOR_WORDS.notes.overrun_unasked, &[("why", &why.to_string())]))
            .await;
        }
        Overtime::Cut | Overtime::NotAsked => {}
      }
    }

    if deadline.is_some_and(|at| now_ms() >= at) {
      door.interrupt(pane, &turn.interrupt).await;
      *ending = End::Timeout;

      let shown = door.screen(pane).await;
      let screen = door.kept_screen(&DOOR_WORDS.screens.overran, shown).await;

      return Ok(Ending::Overran { screen });
    }

    let now = status_of(door.herdr, pane, timings.gone).await;

    if now == Look::Gone {
      // Gone mid-turn, as herdr has said look after look: somebody closing it.
      *ending = End::Cancelled;

      return Ok(Ending::Vanished {
        screen: "the pane went away while the turn was running".to_owned(),
      });
    }

    // herdr said nothing either way: the record is read on and herdr asked again, and only a silence
    // that outlasts the patience ends the turn, as the silence it is.
    if let Look::Silent(cause) = &now {
      let since = match silent_since {
        Some(since) => since,
        None => {
          door
            .watch
            .note(&fill_marks(&DOOR_WORDS.notes.herdr_silent, &[("cause", &cause.to_string())]))
            .await;
          *silent_since.insert(now_ms())
        }
      };

      if now_ms() - since >= patience.as_millis() as i64 {
        door
          .watch
          .note(&fill_marks(
            &DOOR_WORDS.notes.herdr_gave_up,
            &[("seconds", &((now_ms() - since) as f64 / 1000.0).round().to_string())],
          ))
          .await;
        *ending = End::Cancelled;

        return Ok(Ending::Unanswered {
          screen: "herdr did not answer while the turn was running".to_owned(),
        });
      }

      tokio::time::sleep(every).await;
      continue;
    }

    // The silence's end is written as its start was.
    if let Some(since) = silent_since.take() {
      door
        .watch
        .note(&fill_marks(
          &DOOR_WORDS.notes.herdr_back,
          &[
            ("seconds", &((now_ms() - since) as f64 / 1000.0).round().to_string()),
            ("state", now.word()),
          ],
        ))
        .await;
    }

    door.record_children().await;

    if now.is("working") {
      worked = true;
    }

    // Over where the program ended the turn on its own error, whatever the pane shows: a full window leaves
    // the program standing at its own choice of what to do next, which nobody is coming to answer and one
    // of whose answers spends usage credits, so it is closed with the keys the provider cuts a turn
    // with — on Claude Code its 「stop and wait」 — and the program stands at rest for the next turn.
    // It is read before any question the pane holds is put to anybody, since this one is no question of
    // the turn's.
    if !now.is("working")
      && (ended_on_error(&door.lines(), answer_from) || door.failure_since(failed_from).is_some())
    {
      if now.is("blocked") {
        door.watch.note(&DOOR_WORDS.notes.error_menu).await;
        door.interrupt(pane, &turn.interrupt).await;
      }

      break;
    }

    door.questions.look(door.herdr, pane, &now, door.watch).await;

    // Words owed after a refusal the screen could not carry go in once the run is back at rest, and the
    // turn goes on: what it does with them is still this turn's work.
    if now.at_rest()
      && let Some(owed) = door.questions.owed()
    {
      door.check_launch().await?;
      door.hand(true);

      let told = prompt_with_lead(door.herdr, pane, &turn.lead, &owed).await;

      door.questions.paid();

      if told.is_err() {
        door.hand(false);
      } else {
        answer_from = door.lines().len();
        failed_from = now_ms();
        worked = false;
        unstarted_by = now_ms() + timings.ready.as_millis() as i64;
        tokio::time::sleep(every).await;
        continue;
      }
    }

    // Over when it has come back to rest having done something: seen at work, or with an answer in the
    // record written after this turn began, which the idle before the prompt cannot have.
    if now.at_rest() && (worked || answered_since(&door.lines(), answer_from)) {
      break;
    }

    if now.at_rest() && !worked && now_ms() >= unstarted_by {
      break;
    }

    // Only while it is working: a word pushed at an agent that has not started is one the turn's own
    // prompt has not reached yet, and one pushed after it has stopped belongs to the next turn.
    if now.is("working")
      && let Some(words) = door.watch.words().await
    {
      if let Some(why) = turn_refusal(&door.ground.db, JOB_WORK_KEY, &BTreeMap::new()).await {
        return Err(why);
      }

      let outcome = match door.push_word(pane, from, &words).await {
        Ok(_) => AsideOutcome::Delivered,
        Err(fault) if is_blocked(&fault) => AsideOutcome::Waiting,
        Err(_) => AsideOutcome::Refused,
      };

      door.watch.said(outcome).await;
    }

    tokio::time::sleep(every).await;
  }

  // At rest is not over: the program's screen can come to rest before the answer of a turn of a second or
  // two has reached its record, so a turn seen at work is given the settling while for its answer to be
  // written, and the record's closing line comes after the last of the answer's words.
  let settle_by = now_ms() + timings.settle.as_millis() as i64;

  while worked && !answered_since(&door.lines(), answer_from) && now_ms() < settle_by {
    tokio::time::sleep(every).await;
  }

  if answered_since(&door.lines(), answer_from) {
    while !turn_closed_since(&door.lines(), answer_from) && now_ms() < settle_by {
      tokio::time::sleep(every).await;
    }
  }

  // The program holds on to what it was handed and takes it as a turn of its own once the turn it was
  // in has ended, so the round is over only once nothing it was handed is still queued and every turn
  // it owes has written its end. A program at rest whose record has stopped growing has nothing still
  // to come, and the wait ends there and says what the count still expected. A program of the turn's own
  // is handed nothing past its prompt.
  if door.standing.is_some() && answered_since(&door.lines(), from) {
    let mut grown = door.lines().len();
    let mut quiet_since = now_ms();

    while !door.cancelled().await && deadline.is_none_or(|at| now_ms() < at) {
      let records = door.lines();
      let round = records.get(from..).unwrap_or_default();

      if read_round(round).state != RoundState::Working {
        break;
      }

      if from + round.len() != grown {
        grown = from + round.len();
        quiet_since = now_ms();
      } else if now_ms() - quiet_since >= timings.quiet.as_millis() as i64 {
        let now = status_of(door.herdr, pane, GoneConfirm::default()).await;

        if now.at_rest() {
          let owed = turns_owed(round);
          let ended = round
            .iter()
            .filter(|record| {
              record.get("type").and_then(Value::as_str) == Some("system")
                && record.get("subtype").and_then(Value::as_str) == Some("turn_duration")
            })
            .count();

          door
            .watch
            .note(&fill_marks(
              &DOOR_WORDS.notes.quiet,
              &[
                ("seconds", &((now_ms() - quiet_since) as f64 / 1000.0).round().to_string()),
                ("queued", &owed.queued.to_string()),
                ("owed", &owed.owed.to_string()),
                ("ended", &ended.to_string()),
              ],
            ))
            .await;
          break;
        }
      }

      tokio::time::sleep(every).await;
    }
  }

  // The round is read to its end, so no word of it is still to arrive, and a mark left by a word whose
  // submit never came would take the next person's line for the bot's.
  door.hand(false);

  let records = door.lines();
  let answered = answered_since(&records, from);
  let read = read_turn(&records, from);
  let failure = door.failure_since(failed_from).map(|failure| failure.failure());

  if let Some((error, kind)) = turn_error(&read, answered, failure) {
    if answered {
      door.record_children().await;
    }

    return Ok(Ending::Errored { error, kind });
  }

  if !answered {
    let shown = door.screen(pane).await;
    let screen = door.kept_screen(&DOOR_WORDS.screens.silent, shown).await;

    return Ok(Ending::Silent { screen });
  }

  door.record_children().await;

  let accounts = claude_accounts::accounts(&door.ground.db).await.unwrap_or_default();
  let machine_config = ClaudeCode::machine_config_dir(&door.ground.machine());
  let login = program_login(&machine_config, &turn.env);
  let Some(place) = door.standing.as_ref().map(|(place, _)| place.path.clone()) else {
    // What a program of the turn's own cost it writes as it ends, so its pane is closed first and the
    // record watched for that line; a run whose account never comes keeps what its own lines say, with no
    // price.
    let _ = door.herdr.call("pane.close", json!({ "pane_id": pane }), None).await;

    let spend = door.account_line(records.len(), timings.account).await;

    door.close("exited").await;

    let mut reply = read.reply(&turn.session, spend.unwrap_or_else(|| read.spend.clone()));

    reply.account = account_of_login(&accounts, &login, &door.ground.roots.data);

    return Ok(Ending::Answered {
      reply: Box::new(reply),
      records: records.len(),
    });
  };

  // The program stays up, so what the turn cost is its status line's running total after the turn less
  // the total before it, with the account's windows beside it; a reading that does not come keeps what
  // the record says, with no price. The turn ended where its Stop hook says.
  let stopped_at = read_hooks(&place, prompted_at)
    .iter()
    .rev()
    .find(|hook| hook.ends_turn())
    .map_or_else(now_ms, |hook| hook.at);
  let around =
    status_after_turn(&place, prompted_at, stopped_at, Some(&turn.session), timings.account, every).await;
  // The windows are the login's that made the reading, named by the reading itself; a reading that
  // names no registered login is written under nobody.
  let data = &door.ground.roots.data;
  let wrote = around.login.as_ref().and_then(|login| account_of_login(&accounts, login, data));
  let figures = status_figures(around.before.as_ref(), around.after.as_ref());
  let spend = match figures.money {
    Some(money) if read.spend.len() == 1 => read
      .spend
      .iter()
      .cloned()
      .map(|mut row| {
        row.cost = money;
        row
      })
      .collect(),
    _ => read.spend.clone(),
  };
  let spanned = read_round(records.get(from..).unwrap_or_default()).text;
  let answered_by = wrote.clone().or_else(|| account_of_login(&accounts, &login, data));

  door.close("exited").await;

  let mut reply = read.reply(&turn.session, spend);

  reply.text = if spanned.is_empty() { read.text.clone() } else { spanned };

  if wrote.is_some() {
    reply.session_usage = figures.session_usage.clone();
    reply.windows = figures.windows.iter().cloned().map(ReplyWindow::Status).collect();
  }

  reply.context_ratio = figures.context_ratio;
  reply.account = answered_by;
  reply.context_tokens = figures.context_tokens.or(read.context_tokens);

  Ok(Ending::Answered {
    reply: Box::new(reply),
    records: records.len(),
  })
}
