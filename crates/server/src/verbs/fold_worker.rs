//! The job workers' sessions folded, which is what a change to the prefix they were opened on asks for
//! (docs/agent-control.md): with no flag every session whose prefix has moved — the word written on its row
//! when it last answered, against the prefix as it stands now — and the rest left alone and said so; with
//! --all every session found, for a change the word cannot see, the CLI's own version above all.
//!
//! Every fold takes its checkout's job lock without waiting, so a session a job is driving is left to that
//! job and said so: this verb never folds the session it is run from. A fold is the run the procedure after
//! a prefix change requires, and a job is who runs it after a deployment: it says so before it opens
//! anything, which is what lets the guard open it from inside a job (docs/live-claude-guard.md).

use std::path::Path;

use policy::fill_marks;
use policy::verb_words::{VERB_WORDS, WorkerFoldOutcomesWords as Outcomes};

use super::{Answer, VerbError, service};
use crate::Refusal;
use crate::door::guard::declare_required_run;
use crate::lines::Trees;
use crate::lines::standing::worker_prefix_digest;
use crate::places::{AgentRow, Ground, locks};
use crate::worker_session::fold::{Folded, fold};
use store::all;

/// What the verb was asked to fold.
#[derive(Debug, Default)]
pub struct FoldAsked {
  pub worker: Option<String>,
  pub model: Option<String>,
  pub all: bool,
  pub dry_run: bool,
}

// One session a worker holds, with the prefix word its row carries.
struct Standing {
  row: AgentRow,
  cwd: String,
  model: String,
  account: String,
  session: String,
  prefix: Option<String>,
}

fn line(text: &str) -> String {
  fill_marks(&VERB_WORDS.worker_fold.line, &[("line", text)])
}

fn word(words: &Outcomes, folded: Folded) -> &str {
  match folded {
    Folded::Compacted => &words.compacted,
    Folded::TooShort => &words.too_short,
    Folded::Busy => &words.busy,
    Folded::Failed => &words.failed,
  }
}

/// Whether a session was opened on a prefix that is no longer what the machine would assemble now. A
/// session recorded before the word was kept carries none and is left alone, and so is one whose prefix
/// cannot be read now: nothing about either says the prefix moved, and folding on a comparison nobody
/// could make would throw a session away for no reason.
pub fn prefix_moved(kept: Option<&str>, now: Option<&str>) -> bool {
  matches!((kept, now), (Some(kept), Some(now)) if kept != now)
}

/// One line about one fold, in the words the outcome actually means.
pub fn fold_wording(worker: &str, model: &str, folded: Folded) -> String {
  fill_marks(
    word(&VERB_WORDS.worker_fold.outcomes, folded),
    &[("name", &format!("{worker} / {model}"))],
  )
}

/// The last line of a fold run: what became of every session it touched and nothing that did not happen;
/// a run that found none says so, and the sessions left because their prefix had not moved are counted.
pub fn fold_tally(outcomes: &[Folded], left: usize) -> String {
  let tally = &VERB_WORDS.worker_fold.tally;
  let standing = (left > 0).then(|| fill_marks(&tally.left, &[("count", &left.to_string())]));

  if outcomes.is_empty() {
    return match standing {
      None => tally.none.clone(),
      Some(standing) => format!("{} · {standing}", tally.none),
    };
  }

  let parts: Vec<String> = [Folded::Compacted, Folded::TooShort, Folded::Busy, Folded::Failed]
    .into_iter()
    .map(|kind| (kind, outcomes.iter().filter(|one| **one == kind).count()))
    .filter(|(_, count)| *count > 0)
    .map(|(kind, count)| {
      fill_marks(&tally.part, &[("word", word(&tally.words, kind)), ("count", &count.to_string())])
    })
    .chain(standing)
    .collect();

  fill_marks(
    &tally.sessions,
    &[("count", &outcomes.len().to_string()), ("parts", &parts.join(" · "))],
  )
}

async fn standing(ground: &Ground, asked: &FoldAsked) -> Result<Vec<Standing>, Refusal> {
  let mut found = vec![];

  for row in AgentRow::workers(&ground.db).await? {
    if asked.worker.as_deref().is_some_and(|only| only != row.name) {
      continue;
    }

    let cwd = ground.checkout_of(&row);
    let rows = all(
      &ground.db,
      r#"SELECT "model", "account", "claude_session_id", "prefix_digest" FROM "worker_sessions"
         WHERE "worker" = $1 ORDER BY "last_reply_at""#,
      vec![row.name.clone().into()],
    )
    .await?;

    for session in rows {
      let model: String = session.try_get("", "model")?;

      if asked.model.as_deref().is_some_and(|only| only != model) {
        continue;
      }

      found.push(Standing {
        row: row.clone(),
        cwd: cwd.clone(),
        model,
        account: session.try_get("", "account")?,
        session: session.try_get("", "claude_session_id")?,
        prefix: session.try_get("", "prefix_digest")?,
      });
    }
  }

  Ok(found)
}

/// The sessions folded as asked, each said in a line as it is reached, and the run's tally; a failed fold
/// ends the run with a failure.
pub async fn fold_worker(
  ground: &Ground,
  data_root: &Path,
  asked: &FoldAsked,
  say: &mut impl FnMut(String),
) -> Result<Answer, VerbError> {
  declare_required_run("fold-worker: the worker session fold a prefix change requires");

  let trees = Trees::of(ground);
  let mut outcomes = vec![];
  let (mut left, mut due) = (0, 0);

  for one in standing(ground, asked).await? {
    let name = format!("{} / {}", one.row.name, one.model);
    // A session whose prefix word moved against the prefix as it stands now was opened on something gone.
    let now = match &one.prefix {
      Some(_) if !asked.all => worker_prefix_digest(ground, &trees, &one.cwd, &one.model, None).await.ok(),
      _ => None,
    };
    let moved = prefix_moved(one.prefix.as_deref(), now.as_deref());

    if !asked.all && !moved {
      left += 1;

      let untouched = if one.prefix.is_none() {
        &VERB_WORDS.worker_fold.untouched.no_record
      } else {
        &VERB_WORDS.worker_fold.untouched.same
      };

      say(line(&fill_marks(untouched, &[("name", &name)])));
      continue;
    }

    due += 1;

    if asked.dry_run {
      say(line(&fill_marks(
        &VERB_WORDS.worker_fold.due,
        &[("name", &name), ("session", &one.session)],
      )));
      continue;
    }

    // The checkout's own lock, taken without waiting: a session a job is driving is that job's.
    let folded = match locks::try_hold(&locks::lock_path(data_root, &one.cwd)) {
      None => Folded::Busy,
      Some(_held) => fold(ground, &one.row, &one.model, &one.account, &one.session).await,
    };

    outcomes.push(folded);
    say(line(&fold_wording(&one.row.name, &one.model, folded)));
  }

  let last = if asked.dry_run {
    let tally = &VERB_WORDS.worker_fold.tally;
    let head = if due == 0 {
      tally.none.clone()
    } else {
      fill_marks(&tally.due, &[("count", &due.to_string())])
    };
    let standing = if left == 0 {
      String::new()
    } else {
      format!(" · {}", fill_marks(&tally.left, &[("count", &left.to_string())]))
    };

    format!("{head}{standing}")
  } else {
    fold_tally(&outcomes, left)
  };

  Ok(Answer {
    lines: vec![line(&last)],
    code: u8::from(outcomes.contains(&Folded::Failed)),
  })
}

/// The ground a fold stands on in a process beside the server: the service on its store, this machine and
/// the herdr the workers' panes stand in.
pub async fn ground_of(data_root: &Path) -> Result<Ground, VerbError> {
  let service = service(data_root).await?;
  let machine = harness_providers::Machine::of_process(data_root);
  let herdr =
    herdr::socket_path(machine.var("XDG_CONFIG_HOME"), machine.var("HOME")).map(herdr::Herdr::there);

  Ok(Ground::new(&service, &machine, herdr))
}
