//! The session an agent holds on its line, as the TypeScript side's agent instance keeps it
//! (docs/cache-fallback.md): the road it stands on, the prefix it was opened against and when that prefix
//! was last read, kept in one setting per scope. A run is judged by it on the way in — a session on another
//! road, another login, a prefix that moved or a cache that outlived its life is replaced, and the next run
//! is handed the note the replaced one left — and the session that answered is kept again on the way out.
//! Here it is the worker line's, on the provider the run goes out on; the note itself is the session
//! summariser's, a hook the caller registers (`super::summary`). A worker's own Claude Code session turned
//! from for another model is set aside on its own model rather than replaced, and resumed on a return while
//! its cache is warm (docs/sessions.md, 「A worker's session per model」).

use std::collections::BTreeMap;

use harness_providers::provider;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use store::run;

use super::summary::{self, Closed, Kind};
use super::warmth;
use crate::control::claude_accounts::{Account, SEED_ACCOUNT};
use crate::door::record::Reply;
use crate::lines::Trees;
use crate::lines::standing::worker_prefix_digest;
use crate::lines::worker::WorkerRun;
use crate::places::Ground;
use crate::queue::settings;
use crate::{Refusal, now_ms};

/// The provider a worker's own session stands on where no road moved it.
pub const MODE: &str = "claude-code";

// The provider whose threads a worker's scope keeps beside its sessions.
const CODEX: &str = crate::lines::codex::PROVIDER;

// How much of what was said on a road with no session of its own is held, in the UTF-16 units the
// TypeScript side counts a text in, since either side reads the other's.
pub(crate) const TRANSCRIPT_UNITS: usize = 40_000;

// The last of a text, by its UTF-16 units.
pub(crate) fn last_units(text: &str, units: usize) -> String {
  let all: Vec<u16> = text.encode_utf16().collect();

  String::from_utf16_lossy(&all[all.len().saturating_sub(units)..])
}

/// The scope a worker's session is held under.
pub fn scope_of(worker: &str) -> String {
  format!("job-worker:{worker}")
}

pub(crate) fn key(scope: &str) -> String {
  format!("agent_session:{scope}")
}

pub(crate) fn pending_key(scope: &str) -> String {
  format!("agent_session_summary:{scope}")
}

pub(crate) fn pending_thread_key(scope: &str) -> String {
  format!("agent_session_summary_thread:{scope}")
}

// The sessions a scope has set aside on their own models while it turned to another.
pub(crate) fn shelf_key(scope: &str) -> String {
  format!("agent_session_shelf:{scope}")
}

/// A held session as the setting keeps it.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Held {
  pub mode: String,
  pub model: String,
  pub effort: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tier: Option<Value>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub placement: Option<String>,
  #[serde(default)]
  pub id: Option<String>,
  // The fields in the order the TypeScript side writes them, since either side may read the other's.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cwd: Option<String>,
  pub prefix: String,
  pub at: i64,
  #[serde(default)]
  pub missed: bool,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub transcript: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub thread: Option<String>,
}

/// One run on a worker's session as it is judged: the provider and the line it goes out on, the words it is
/// handed, the session it resumes, the login it goes out under, the node its session stands on where its
/// provider places sessions, and the thread it is on — a job's item, whose next run alone is handed the
/// note a session closed on it leaves.
#[derive(Clone, Debug)]
pub struct SessionRun {
  pub mode: &'static str,
  pub line: WorkerRun,
  pub prompt: String,
  pub resume: Option<String>,
  pub account: Option<Account>,
  pub placement: Option<String>,
  pub thread: Option<String>,
}

impl SessionRun {
  /// The login the run's session is recorded under: its account, or the seed account a run naming none
  /// stands for.
  pub fn recorded_account(&self) -> String {
    self
      .account
      .as_ref()
      .map_or_else(|| SEED_ACCOUNT.to_owned(), |account| account.id.clone())
  }
}

/// The held session of a scope, where one is held and readable.
pub async fn held(ground: &Ground, scope: &str) -> Result<Option<Held>, Refusal> {
  Ok(
    settings::get(&ground.db, &key(scope))
      .await?
      .and_then(|text| serde_json::from_str(&text).ok()),
  )
}

// A file the line points at, read whole, and nothing where it is not on the disk: the line still opens
// on it.
fn file(path: Option<&std::path::Path>) -> Value {
  path
    .and_then(|path| std::fs::read_to_string(path).ok())
    .map_or(Value::Null, Value::String)
}

/// What a worker run's system prefix reduces to on a provider, as the TypeScript side's chat prefix and
/// the worker prefix together reduce it: the provider, the line and its words, and the files it is
/// handed, with the worker line's own prefix digest after them.
pub async fn prefix(ground: &Ground, line: &WorkerRun, mode: &str) -> Result<String, Refusal> {
  // The fields in the order the TypeScript side writes them, since the digest is of the text.
  let fields: [(&str, Value); 13] = [
    ("mode", json!(mode)),
    ("line", json!("worker")),
    ("model", json!(line.model)),
    ("effort", json!(line.effort)),
    ("thinking", json!(line.thinking)),
    ("hostCommands", json!(true)),
    ("system", file(line.system_prompt.as_deref())),
    ("toolsShown", Value::Null),
    ("toolsAllowed", Value::Null),
    ("toolsDenied", json!(line.tools_denied)),
    ("mcp", file(Some(&line.mcp_config))),
    ("mcpTools", Value::Null),
    ("cwd", json!(line.cwd)),
  ];
  let text = format!(
    "{{{}}}",
    fields
      .iter()
      .map(|(name, value)| format!("{}:{}", json!(name), value))
      .collect::<Vec<_>>()
      .join(",")
  );
  let chat = format!("{:x}", Sha256::digest(text.as_bytes()));
  let worker = worker_prefix_digest(ground, &Trees::of(ground), &line.cwd, &line.model, None).await?;
  let mut both = Sha256::new();

  both.update(chat.as_bytes());
  both.update(worker.as_bytes());

  Ok(format!("{:x}", both.finalize()))
}

// Whether the run goes out on the road the held session was opened on: the same provider, model and
// effort, and the same login where the provider holds logins of agency's.
fn same_road(held: &Held, run: &SessionRun) -> bool {
  let logins = provider(run.mode).is_some_and(|provider| provider.holds_logins());

  held.mode == run.mode
    && held.model == run.line.model
    && held.effort == run.line.effort
    && (!logins || held.account.as_deref().unwrap_or(SEED_ACCOUNT) == run.recorded_account())
}

// Whether the session held may not be resumed by this run: none held for a run that resumes, a road
// moved, a prefix moved, or a prefix the cache has let go of.
async fn must_replace(ground: &Ground, held: Option<&Held>, run: &SessionRun) -> Result<bool, Refusal> {
  let Some(held) = held else { return Ok(run.resume.is_some()) };

  if !same_road(held, run) {
    return Ok(true);
  }

  if held.prefix != prefix(ground, &run.line, run.mode).await? {
    return Ok(true);
  }

  // A lifetime the provider does not declare leaves a session nothing to outlive.
  let lifetime = warmth::lifetime_ms(run.mode, &run.line.model);

  Ok(lifetime.is_some_and(|lifetime| !warmth::warm(held.at, now_ms(), lifetime)))
}

// The note left for the scope where it belongs to the run's thread.
pub(crate) async fn pending(
  ground: &Ground,
  scope: &str,
  thread: Option<&str>,
) -> Result<Option<String>, Refusal> {
  let note = settings::get(&ground.db, &pending_key(scope)).await?;

  Ok(if settings::get(&ground.db, &pending_thread_key(scope)).await?.as_deref() == thread {
    note
  } else {
    None
  })
}

// The sessions a scope has set aside, one to a model, as the setting keeps them.
async fn shelf(ground: &Ground, scope: &str) -> Result<BTreeMap<String, Held>, Refusal> {
  Ok(
    settings::get(&ground.db, &shelf_key(scope))
      .await?
      .and_then(|text| serde_json::from_str(&text).ok())
      .unwrap_or_default(),
  )
}

async fn keep_shelf(ground: &Ground, scope: &str, shelf: &BTreeMap<String, Held>) -> Result<(), Refusal> {
  if shelf.is_empty() {
    return settings::forget(&ground.db, &shelf_key(scope)).await;
  }

  let text =
    serde_json::to_string(shelf).map_err(|why| Refusal::bad("session-unwritten", why.to_string()))?;

  settings::set(&ground.db, &shelf_key(scope), &text).await
}

/// Whether a session set aside is still warm at a moment: its last answer inside the cache lifetime its
/// provider declares for its model, or an hour where none is declared (`super::warmth`).
pub fn warm(held: &Held, now: i64) -> bool {
  warmth::warm(held.at, now, warmth::set_aside_lifetime_ms(&held.mode, &held.model))
}

/// Whether the run turns the worker's own session to another model, which sets the held one aside.
pub fn turns_from(held: &Held, run: &SessionRun) -> bool {
  held.mode == MODE && run.mode == MODE && held.model != run.line.model
}

// The session set aside on the run's model, taken off the shelf: handed back where the run resumes it, it
// is still warm and its road and prefix have not moved; dropped otherwise, with the record that would
// resume it where the run was about to, so the run opens a new session.
async fn take_shelved(
  ground: &Ground,
  worker: &str,
  scope: &str,
  run: &mut SessionRun,
) -> Result<Option<Held>, Refusal> {
  let mut shelf = shelf(ground, scope).await?;
  let Some(kept) = shelf.remove(&run.line.model) else { return Ok(None) };

  keep_shelf(ground, scope, &shelf).await?;

  let named = run.resume.is_some() && kept.id == run.resume;

  if named
    && same_road(&kept, run)
    && kept.prefix == prefix(ground, &run.line, run.mode).await?
    && warm(&kept, now_ms())
  {
    return Ok(Some(kept));
  }

  if named {
    sessions_of(ground, worker, &run.line.model).await?;
    run.resume = None;
  }

  Ok(None)
}

// The held session set aside on its own model as the worker turns to another: its note left for the new
// session under the switch note, and its record kept for the worker's return.
async fn turn_from(
  ground: &Ground,
  worker: &str,
  scope: &str,
  left: &Held,
  run: &SessionRun,
) -> Result<(), Refusal> {
  let account = left.account.clone().unwrap_or_else(|| SEED_ACCOUNT.to_owned());
  let note = summary::handover(
    &ground.hooks,
    Closed {
      kind: Kind::Switch,
      mode: left.mode.clone(),
      session_id: left.id.clone(),
      cwd: left.cwd.clone().unwrap_or_else(|| run.line.cwd.clone()),
      account: Some(account),
      transcript: left.transcript.clone(),
      own_note: None,
      chat_id: None,
    },
  )
  .await;

  leave_note(ground, scope, note.as_deref(), left.thread.as_deref()).await?;

  let mut shelf = shelf(ground, scope).await?;

  shelf.insert(left.model.clone(), left.clone());
  keep_shelf(ground, scope, &shelf).await?;
  // A record on the run's model that no session set aside answers for is not resumed into blind.
  sessions_of(ground, worker, &run.line.model).await?;

  settings::forget(&ground.db, &key(scope)).await
}

/// The run as it may go out on the worker's held session: on it; on the session set aside on its model,
/// where it comes back to that model while the session is warm; or on a fresh session with the note the
/// replaced or set-aside one left ahead of its words (docs/sessions.md, 「A worker's session per model」).
pub async fn prepare(ground: &Ground, worker: &str, mut run: SessionRun) -> Result<SessionRun, Refusal> {
  let scope = scope_of(worker);
  let held = held(ground, &scope).await?;

  if run.mode == MODE
    && let Some(back) = take_shelved(ground, worker, &scope, &mut run).await?
  {
    // The session turned from is set aside in its turn where it is the worker's own; a stand-in road's
    // keeps no record to come back to, and its thread goes.
    match held.as_ref() {
      Some(left) if left.mode == MODE && left.model != back.model => {
        let mut shelf = shelf(ground, &scope).await?;

        shelf.insert(left.model.clone(), left.clone());
        keep_shelf(ground, &scope, &shelf).await?;
      }
      Some(left) if left.mode == CODEX => threads_of(ground, &scope, &left.model, &left.effort).await?,
      _ => {}
    }

    let text =
      serde_json::to_string(&back).map_err(|why| Refusal::bad("session-unwritten", why.to_string()))?;

    settings::set(&ground.db, &key(&scope), &text).await?;

    return Ok(run);
  }

  if let Some(left) = held.as_ref().filter(|left| turns_from(left, &run)) {
    turn_from(ground, worker, &scope, left, &run).await?;
    run.resume = None;
  } else if must_replace(ground, held.as_ref(), &run).await? {
    let thread = held.as_ref().and_then(|held| held.thread.clone());

    replace(ground, worker, &scope, held.as_ref(), &run, thread.as_deref(), None).await?;
    run.resume = None;
  }

  if run.resume.is_none()
    && let Some(note) = pending(ground, &scope, run.thread.as_deref()).await?
  {
    run.prompt = format!("{note}\n\n{}", run.prompt);
  }

  Ok(run)
}

/// The worker's session handed over on purpose — as its job ends, or ahead of a retry on a fresh session:
/// the session it holds, or the run's own where it holds none, closed with its note left for the run's
/// thread alone, and every session it set aside on another model dropped. The note is the worker's own
/// where it wrote one, and otherwise asked of the session summariser out of the session's record.
pub async fn hand_over(
  ground: &Ground,
  worker: &str,
  run: &SessionRun,
  own_note: Option<&str>,
) -> Result<(), Refusal> {
  let scope = scope_of(worker);
  let held = held(ground, &scope).await?;

  replace(ground, worker, &scope, held.as_ref(), run, run.thread.as_deref(), own_note).await?;

  // Every session set aside ends with the one held, its record with it.
  for kept in shelf(ground, &scope).await?.values().filter(|kept| kept.mode == MODE) {
    sessions_of(ground, worker, &kept.model).await?;
  }

  settings::forget(&ground.db, &shelf_key(&scope)).await
}

// The held session closed: its note left for the thread given, and every record that would resume it gone.
async fn replace(
  ground: &Ground,
  worker: &str,
  scope: &str,
  held: Option<&Held>,
  run: &SessionRun,
  thread: Option<&str>,
  own_note: Option<&str>,
) -> Result<(), Refusal> {
  // The record being read is the held session's, under the login that opened it; with nothing held, it
  // is the run's own.
  let account = match held {
    None => run.account.as_ref().map(|account| account.id.clone()),
    Some(held) => Some(held.account.clone().unwrap_or_else(|| SEED_ACCOUNT.to_owned())),
  };
  let session_id = held.and_then(|held| held.id.clone()).or_else(|| run.resume.clone());
  let cwd = held.and_then(|held| held.cwd.clone()).unwrap_or_else(|| run.line.cwd.clone());
  let note = summary::handover(
    &ground.hooks,
    Closed {
      kind: Kind::Worker,
      mode: held.map_or(run.mode, |held| held.mode.as_str()).to_owned(),
      session_id,
      cwd,
      account,
      transcript: held.and_then(|held| held.transcript.clone()),
      own_note: own_note.map(str::to_owned),
      chat_id: None,
    },
  )
  .await;
  let db = &ground.db;

  leave_note(ground, scope, note.as_deref(), thread).await?;

  // A thread is kept by Codex's road and a session record by Claude Code's: whichever the closed session
  // and this run stood on goes.
  if let Some(held) = held {
    if held.mode == CODEX {
      threads_of(ground, scope, &held.model, &held.effort).await?;
    }

    if held.mode == MODE {
      sessions_of(ground, worker, &held.model).await?;
    }
  }

  if run.mode == CODEX {
    threads_of(ground, scope, &run.line.model, &run.line.effort).await?;
  }

  if run.mode == MODE {
    sessions_of(ground, worker, &run.line.model).await?;
  }

  settings::forget(db, &key(scope)).await
}

// The note a closed session left kept for the next run of the thread given; where the summariser left
// none, nothing is carried over and no note of an earlier session stands in its place.
async fn leave_note(
  ground: &Ground,
  scope: &str,
  note: Option<&str>,
  thread: Option<&str>,
) -> Result<(), Refusal> {
  let db = &ground.db;
  let Some(note) = note else {
    settings::forget(db, &pending_key(scope)).await?;

    return settings::forget(db, &pending_thread_key(scope)).await;
  };

  settings::set(db, &pending_key(scope), note).await?;

  match thread {
    None => settings::forget(db, &pending_thread_key(scope)).await,
    Some(thread) => settings::set(db, &pending_thread_key(scope), thread).await,
  }
}

// The Codex thread the scope keeps on a model and effort.
async fn threads_of(ground: &Ground, scope: &str, model: &str, effort: &str) -> Result<(), Refusal> {
  run(
    &ground.db,
    r#"DELETE FROM "fallback_sessions" WHERE "scope" = $1 AND "mode" = 'codex' AND "model" = $2 AND "effort" = $3"#,
    vec![scope.into(), model.into(), effort.into()],
  )
  .await?;

  Ok(())
}

// Every login's session record of the worker on a model, which a replaced session leaves worthless.
async fn sessions_of(ground: &Ground, worker: &str, model: &str) -> Result<(), Refusal> {
  run(
    &ground.db,
    r#"DELETE FROM "worker_sessions" WHERE "worker" = $1 AND "model" = $2"#,
    vec![worker.into(), model.into()],
  )
  .await?;

  Ok(())
}

/// The session that answered kept as the one the scope holds, on the model it answered on, and the note
/// it was handed taken.
pub async fn remember(
  ground: &Ground,
  worker: &str,
  run: &SessionRun,
  reply: &Reply,
) -> Result<(), Refusal> {
  if reply.text.trim().is_empty() {
    return Ok(());
  }

  let scope = scope_of(worker);
  let answering = provider(&reply.mode);
  // What the turn read of the cache and sent fresh: its last request alone on a provider that counts its
  // requests one by one, all it spent on any other.
  let (reads, fresh) = if answering.is_some_and(|provider| provider.spends_by_last_request()) {
    reply
      .last_request_usage
      .as_ref()
      .map_or((0, 0), |last| (last.cache_read_tokens, last.input_tokens + last.cache_write_tokens))
  } else {
    (
      reply.spend.iter().map(|spend| spend.cache_read_tokens).sum(),
      reply.spend.iter().map(|spend| spend.input_tokens + spend.cache_write_tokens).sum(),
    )
  };
  // A road that keeps no session of its own is held by what was said on it, the last of it.
  let transcript = match reply.session_id {
    Some(_) => None,
    None => {
      let before = held(ground, &scope).await?.and_then(|held| held.transcript).unwrap_or_default();

      Some(last_units(
        &format!("{before}\nUSER: {}\nASSISTANT: {}", run.prompt, reply.text),
        TRANSCRIPT_UNITS,
      ))
    }
  };
  let answered = reply.model.clone().unwrap_or_else(|| run.line.model.clone());
  // A session the safeguard answered on another model stands on that model now, and the prefix held
  // for it is that model's, so the next turn on it does not read as a moved prefix.
  let mut standing = run.line.clone();

  standing.model.clone_from(&answered);

  let kept = Held {
    mode: reply.mode.clone(),
    model: answered,
    effort: run.line.effort.clone(),
    tier: run.line.tier.clone().map(Value::String),
    account: answering
      .is_some_and(|provider| provider.holds_logins())
      .then(|| run.recorded_account()),
    placement: run.placement.clone(),
    id: reply.session_id.clone(),
    prefix: prefix(ground, &standing, &reply.mode).await?,
    at: now_ms(),
    missed: run.resume.is_some() && reads == 0 && fresh > 0,
    cwd: Some(run.line.cwd.clone()),
    transcript,
    thread: run.thread.clone(),
  };
  let text =
    serde_json::to_string(&kept).map_err(|why| Refusal::bad("session-unwritten", why.to_string()))?;

  settings::set(&ground.db, &key(&scope), &text).await?;

  if pending(ground, &scope, run.thread.as_deref()).await?.is_some() {
    settings::forget(&ground.db, &pending_key(&scope)).await?;
    settings::forget(&ground.db, &pending_thread_key(&scope)).await?;
  }

  Ok(())
}
