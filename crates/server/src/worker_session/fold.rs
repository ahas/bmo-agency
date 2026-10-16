//! A worker's full session folded inside its own resident program (docs/agent-control.md, 「Folding
//! travels with the resident program」): one more turn of it, told the fold word, on the prefix every
//! other turn stands on and on the login and model the session was opened on. The record says whether
//! it folded: the program writes its compacted marker after the fold word and keeps the session's id.
//! Nothing here fails the caller: the fold is upkeep after the turn it follows is done, and a fold that
//! fails is a line in the log and a record left as it stands.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use harness_providers::claude_code::ClaudeCode;
use policy::door_words::DOOR_WORDS;
use policy::resident_turn::RESIDENT_TURN;
use regex::Regex;
use serde_json::Value;

use super::held::{self, MODE, scope_of};
use super::records::{self, note_spend, record};
use super::told::SessionFolded;
use super::{Composed, account_named, compose};
use crate::Refusal;
use crate::control::claude_accounts::SEED_ACCOUNT;
use crate::control::harness::provider_enabled;
use crate::door::record::Tail;
use crate::door::turn::{Ending, Quiet, put};
use crate::lines::standing::worker_prefix_digest;
use crate::lines::worker::{configured, worker_run};
use crate::lines::{Trees, fault};
use crate::places::{AgentRow, Ground};

/// How long a fold is given.
pub const FOLD_BOUND: Duration = Duration::from_secs(5 * 60);

/// The fold word Claude Code takes.
pub const FOLD_WORD: &str = "/compact";

/// Whether a worker session is full once its turn is over: past the fold ratio of its model's window
/// (docs/sessions.md, 「Session compaction」). A turn that reports no share is not full.
pub fn worker_session_full(context_ratio: Option<f64>) -> bool {
  context_ratio.is_some_and(|ratio| ratio >= RESIDENT_TURN.fold_ratio)
}

// What the program answers in its own words for a session with too little in it to fold.
static TOO_SHORT: LazyLock<Regex> =
  LazyLock::new(|| Regex::new("(?i)not enough messages to compact").expect("a pattern"));

/// What became of a fold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Folded {
  Compacted,
  TooShort,
  Busy,
  Failed,
}

impl Folded {
  pub fn word(self) -> &'static str {
    match self {
      Self::Compacted => "compacted",
      Self::TooShort => "too-short",
      Self::Busy => "busy",
      Self::Failed => "failed",
    }
  }
}

// What a consumer sent a session to hold — a co-work result among it — is folded away with the detail,
// so the fold that reached it is published for the consumer to let its own record of it go.
fn forget_loaded(ground: &Ground, session: &str) {
  ground.events.publish(SessionFolded {
    session: session.to_owned(),
  });
}

// The record a session's turns are read out of, under the login its program goes out on.
fn record_of(
  ground: &Ground,
  env: &std::collections::BTreeMap<String, String>,
  cwd: &str,
  session: &str,
) -> PathBuf {
  let dir = env
    .get("CLAUDE_CONFIG_DIR")
    .filter(|dir| !dir.is_empty())
    .map_or_else(|| ClaudeCode::machine_config_dir(&ground.machine()), PathBuf::from);

  ClaudeCode::records_under(&dir, Path::new(cwd), session).swap_remove(0)
}

/// The worker's session on `model` and the login `account` names, folded; what came of it, also told to
/// the machine's log.
pub async fn fold(ground: &Ground, row: &AgentRow, model: &str, account: &str, session: &str) -> Folded {
  let worker = &row.name;
  let (outcome, detail) = match folded(ground, row, model, account, session).await {
    Ok(done) => done,
    Err(why) => {
      eprintln!("compact {worker} {model}: failed — {}", why.detail);
      // A fold that broke off may have reached the session all the same.
      forget_loaded(ground, session);

      return Folded::Failed;
    }
  };

  println!(
    "compact {worker} {model}: {}{}",
    outcome.word(),
    if detail.is_empty() { String::new() } else { format!(" — {detail}") }
  );

  outcome
}

async fn folded(
  ground: &Ground,
  row: &AgentRow,
  model: &str,
  account: &str,
  session: &str,
) -> Result<(Folded, String), Refusal> {
  let worker = &row.name;
  let named = account_named(ground, account).await?;
  let recorded = named.as_ref().map_or_else(|| SEED_ACCOUNT.to_owned(), |account| account.id.clone());

  if !provider_enabled(&ground.db, MODE).await.map_err(|refused| fault(refused.error))? {
    return Ok((Folded::Busy, String::new()));
  }

  // The fold operates on the session's own configuration, as its held record names it, and goes back to
  // the login it was opened under.
  let cwd = ground.checkout_of(row);
  let trees = Trees::of(ground);
  let mut line = worker_run(ground, &trees, Some(worker), model, &cwd).await?;
  let mut login = named;

  if let Some(kept) = held::held(ground, &scope_of(worker)).await? {
    if kept.id.as_deref() != Some(session) || kept.mode != MODE {
      return Err(fault("Compaction session identity changed"));
    }

    line.model = kept.model;
    line.effort = kept.effort;
    line.tier = kept.tier.and_then(|tier| tier.as_str().map(str::to_owned));

    if let Some(id) = kept.account.filter(|id| !id.is_empty()) {
      login = account_named(ground, &id).await?;
    }
  }

  // A login that is off, at its limit or waiting for a person would only fail the fold, which waits for it.
  if let Some(held) = &login
    && !crate::control::claude_accounts::account_open(
      crate::control::claude_accounts::Judging::of(ground),
      &held.id,
      crate::now_ms(),
    )
    .await
    .map_err(|refused| fault(refused.error))?
  {
    return Ok((Folded::Busy, String::new()));
  }

  if line.tier.is_none() {
    line = configured(ground, &ground.machine(), line, MODE).await?;
  }

  let turn = compose(
    ground,
    row,
    &line,
    login.as_ref(),
    Some(session),
    Composed {
      name: format!("bmo-fold-{worker}"),
      title: DOOR_WORDS.fold_title.clone(),
      prompt: FOLD_WORD.to_owned(),
      bound: Some(FOLD_BOUND),
    },
  )
  .await?;
  let mut tail = Tail::new(record_of(ground, &turn.env, &turn.cwd, session));
  let from = tail.count();
  let ending = put(ground, &turn, &mut Quiet).await.map_err(fault)?;
  let written: Vec<Value> = tail.look().into_iter().skip(from).collect();
  let compacted = written.iter().any(|line| {
    line.get("type").and_then(Value::as_str) == Some("system")
      && line.get("subtype").and_then(Value::as_str) == Some("compact_boundary")
  });
  let thin = written.iter().any(|line| {
    TOO_SHORT
      .is_match(&line.pointer("/message/content").map_or_else(|| "\"\"".to_owned(), Value::to_string))
  });
  let (spend, answered_by) = match &ending {
    Ending::Answered { reply, .. } => (reply.spend.clone(), reply.account.clone()),
    _ => (vec![], None),
  };

  note_spend(&ground.events, "compact", &spend, &answered_by.unwrap_or_else(|| recorded.clone()));

  let (outcome, detail) = if compacted {
    (Folded::Compacted, String::new())
  } else if thin {
    (Folded::TooShort, String::new())
  } else {
    let detail = match &ending {
      Ending::Answered { .. } => "the fold word left no compacted marker".to_owned(),
      other => format!("the fold turn ended {}", other.word()),
    };

    (
      if matches!(ending, Ending::Blocked { .. } | Ending::Stopped { .. }) {
        Folded::Busy
      } else {
        Folded::Failed
      },
      detail,
    )
  };

  if !matches!(outcome, Folded::Busy | Folded::TooShort) {
    forget_loaded(ground, session);
  }

  if outcome == Folded::Compacted {
    let digest = worker_prefix_digest(ground, &trees, &cwd, &line.model, Some(&line.effort)).await?;
    // The fold ran on the model the session stands on, so a mark it carried still holds.
    let answered = records::sessions(&ground.db, worker)
      .await?
      .into_iter()
      .find(|one| one.model == model && one.account == recorded)
      .and_then(|one| one.answered_model);

    record(&ground.db, worker, model, &recorded, session, Some(&digest), answered.as_deref()).await?;
  }

  Ok((outcome, detail))
}
