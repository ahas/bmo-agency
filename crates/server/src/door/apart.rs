//! A run in a pane of its own (docs/agents.md, 「A run in a pane of its own」): one run of a role — a job's
//! turn, a delegated task, a search, a translation, a probe, a login's renewal — as the interactive program in
//! a pane of its own, opened for the run and closed with it, on the session its role's setting gives it: a
//! new one, or the valid one the role's last run at the same path on the same provider and model left
//! (docs/sessions.md, 「A role's session」). The run's command is its line's; what it
//! is handed beside that is its own tool token and the variables its line set, and what comes back is the
//! door's reading of the program's record.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use harness_providers::claude_code::ClaudeCode;
use herdr::Herdr;
use policy::resident_turn::RESIDENT_TURN;

use super::execution::{Identity, execution_root, own_unit};
use super::guard::launch_refusal;
use super::record::Reply;
use super::surface::interrupt_of;
use super::turn::{Ending, Seat, Timings, Turn, Watch, put};
use crate::apart_sessions;
use crate::job_run::claude::failure::{Failure, RunEnding, of_errored};
use crate::lines::claude::{Command, line_environment};
use crate::lines::place::resumed_command;
use crate::lines::token;
use crate::now_ms;
use crate::places::Ground;
use crate::places::trust::{config_file, mark_onboarded};

// How long a run's tool session may stand if nothing takes its token back.
const TOKEN_BACKSTOP: Duration = Duration::from_secs(24 * 3_600);

/// One run as its caller names it: the name it runs under, the role it holds, the line and provider it goes
/// out on, what a person watching herdr reads it as, the line's command for it — its words opening `session`,
/// a new session the run resumes a kept one in place of where its role keeps sessions — the words it is told,
/// the model, effort and login its execution row and its session's key keep, and the run its execution is a
/// child of and the root it is counted under, where the caller names them rather than this process's own
/// unit. A run carries no clock: it lasts until it ends or is stopped.
pub struct Apart<'a> {
  pub agent: &'a str,
  pub role: &'a str,
  pub line: &'a str,
  pub provider: &'a str,
  pub title: String,
  pub command: Command,
  pub session: String,
  pub prompt: String,
  pub model: String,
  pub effort: String,
  pub account: Option<String>,
  pub parent: Option<String>,
  pub root: Option<String>,
}

/// A new session for a run, which a later run resumes only where the run's role keeps sessions.
pub fn new_session() -> String {
  uuid::Uuid::new_v4().to_string()
}

/// Where the program of a command keeps its record: under the configuration directory its environment
/// names, or the machine's own.
pub fn records_home(ground: &Ground, command: &Command) -> PathBuf {
  command
    .env
    .get("CLAUDE_CONFIG_DIR")
    .cloned()
    .flatten()
    .filter(|dir| !dir.is_empty())
    .map_or_else(|| ClaudeCode::machine_config_dir(&ground.machine()), PathBuf::from)
}

/// The record a run apart writes, by the session it opens.
pub fn record_of(ground: &Ground, command: &Command, session: &str) -> PathBuf {
  ClaudeCode::records_under(&records_home(ground, command), Path::new(&command.cwd), session).swap_remove(0)
}

/// What the pane of a run is handed: what a pane running its line's command is handed by name, and every
/// variable the line set for the run over the launcher's own environment — its login, its thinking, a
/// gateway's road and the run's own settings — and never the rest of that environment.
pub fn pane_environment(
  process: &BTreeMap<String, String>,
  command: &Command,
  name: &str,
) -> BTreeMap<String, String> {
  let mut env = line_environment(command, name, None);

  for (variable, value) in &command.env {
    if let Some(value) = value
      && process.get(variable) != Some(value)
    {
      env.entry(variable.clone()).or_insert_with(|| value.clone());
    }
  }

  env
}

/// One run apart, to its ending: refused before anything stands where the job guard refuses its command,
/// its tool token issued for it and taken back once it ends, its session the kept one where its role keeps
/// sessions and one stands warm, and the turn put into a pane of its own on this machine's herdr, its record
/// handed to `watch` as it is written. A run whose role keeps sessions keeps the session it answered on.
pub async fn run<W: Watch>(ground: &Ground, mut apart: Apart<'_>, watch: &mut W) -> Result<Ending, String> {
  let herdr = ground
    .herdr
    .clone()
    .ok_or_else(|| format!("{}: this machine names no herdr to open the run's pane on", apart.line))?;
  let kept = if apart_sessions::setting(&ground.db, apart.role).await.keeps() {
    Some(apart_sessions::SessionKey {
      role: apart.role.to_owned(),
      path: apart.command.cwd.clone(),
      provider: apart.provider.to_owned(),
      model: apart.model.clone(),
      effort: apart.effort.clone(),
      account: apart.account.clone(),
      command: apart_sessions::key(apart.line, &apart.command, &records_home(ground, &apart.command)),
    })
  } else {
    None
  };

  if let Some(key) = &kept {
    match apart_sessions::take(&ground.db, key, now_ms()).await {
      Ok(Some(held)) if record_of(ground, &apart.command, &held).is_file() => {
        apart.command.args = resumed_command(&apart.command.args, &held);
        apart.session = held;
      }
      Ok(_) => {}
      Err(why) => {
        eprintln!("{}: its kept session could not be read, so it opens a new one: {why:?}", apart.line)
      }
    }
  }

  let line = apart.line;
  let session = apart.session.clone();
  let opened_at = now_ms();
  let ended = run_on(ground, herdr, apart, watch).await;

  if let Some(key) = &kept {
    let kept = match &ended {
      Ok(Ending::Answered { .. }) => {
        let answered = apart_sessions::Answered {
          at: now_ms(),
          opened_at,
          ..apart_sessions::Answered::default()
        };

        apart_sessions::keep(&ground.db, key, &session, answered).await
      }
      _ => apart_sessions::give_back(&ground.db, &session).await,
    };

    if let Err(why) = kept {
      eprintln!("{line}: the session it ran on was not kept: {why:?}");
    }
  }

  ended
}

// The run on the session settled for it.
async fn run_on<W: Watch>(
  ground: &Ground,
  herdr: Herdr,
  apart: Apart<'_>,
  watch: &mut W,
) -> Result<Ending, String> {
  let name = format!("{}-{}", apart.line, apart.session.chars().take(8).collect::<String>());
  let mut env = pane_environment(ground.env(), &apart.command, &name);

  if let Some(why) = launch_refusal(apart.line, &apart.command.args, &env) {
    return Err(why);
  }

  // A pane handed a folder that is not there starts its program in the home instead, whose record is not
  // the one the run is read from.
  if !Path::new(&apart.command.cwd).is_dir() {
    return Err(format!("{}: the run's folder {} is not there", apart.line, apart.command.cwd));
  }

  let resident = harness_providers::provider("claude-code")
    .and_then(|declared| declared.declared().resident.as_ref())
    .ok_or_else(|| "Claude Code stands no resident program".to_owned())?;
  let deadline = u64::try_from(now_ms()).unwrap_or_default() + TOKEN_BACKSTOP.as_millis() as u64;
  let token = token::issue_for_run(ground, apart.line, apart.agent, &apart.command.cwd, &env, deadline)
    .await
    .map_err(|why| why.detail)?;

  if let Some(token) = &token {
    env.insert(token::TOKEN_VARIABLE.to_owned(), token.clone());
  }

  // A program brought up on a configuration directory nobody has opened before stops on its first-run
  // screens before it reads a word; the machine's own login keeps its configuration in the home.
  let configured = apart
    .command
    .env
    .get("CLAUDE_CONFIG_DIR")
    .cloned()
    .flatten()
    .filter(|dir| !dir.is_empty());

  if let Some(dir) = configured {
    mark_onboarded(&config_file(Path::new(&dir)));
  }

  let own = own_unit(ground.env());
  let turn = Turn {
    name,
    title: apart.title,
    seat: Seat::Own(herdr),
    cwd: apart.command.cwd.clone(),
    command: apart.command.args.clone(),
    env,
    prompt: apart.prompt,
    session: apart.session,
    identity: Identity {
      agent: None,
      root: apart.root.or_else(|| Some(execution_root(own.as_deref().unwrap_or_default()))),
      parent: apart.parent.or(own),
      role: Some(apart.role.to_owned()),
      model: Some(apart.model),
      effort: (!apart.effort.is_empty()).then_some(apart.effort),
    },
    line: apart.line.to_owned(),
    provider: apart.provider.to_owned(),
    lead: RESIDENT_TURN.apart_lead.clone(),
    // A run apart works in a folder of its line's own or one made for it, and a line told to ask no
    // permission there trusts the folder by the same decision.
    trusts_folder: true,
    clock: None,
    interrupt: interrupt_of(resident),
    timings: Timings::default(),
  };
  let ended = put(ground, &turn, watch).await;

  if let Some(token) = &token {
    token::revoke(ground, token).await;
  }

  ended
}

/// How a run ended, as its run log's end says it: its answer where it gave one, why its words never became
/// a turn where they did not, and otherwise the word its ending goes by or why it never got under way.
pub fn said(ended: &Result<Ending, String>) -> Result<&str, &str> {
  match ended {
    Ok(Ending::Answered { reply, .. }) => Ok(&reply.text),
    Ok(Ending::Errored { error, .. }) => Err(error),
    Ok(Ending::Unsubmitted { why, .. })
    | Ok(Ending::ProgramLost { why, .. })
    | Ok(Ending::SilentPeer { why, .. }) => Err(why),
    Ok(other) => Err(other.word()),
    Err(why) => Err(why),
  }
}

/// A run's ending in the words a walk of logins reads: its reply where it answered, and otherwise what kept
/// it from answering — the program's own error read as a login's failure where it is one, a stop, the pane
/// or the program failing under it, a person's question, or a run that ended with no answer. `key` names the
/// run, which a gone login's failure names.
pub fn answered(ending: Ending, key: &str) -> Result<Reply, Failure> {
  let died = |message: String| Failure::Died {
    ending: RunEnding::Exit { code: 1 },
    said: None,
    message,
  };

  match ending {
    Ending::Answered { reply, .. } => Ok(*reply),
    Ending::Errored { error, .. } => Err(of_errored(&error, key)),
    Ending::Stopped { .. } => Err(Failure::Stopped),
    Ending::Vanished { .. } => Err(Failure::Transport {
      message: format!("{key}: the run's pane went away under it"),
    }),
    // The program the run was put into failed under it, as a pane gone does: the road and the work are
    // not what failed.
    Ending::ProgramLost { why, .. } | Ending::SilentPeer { why, .. } => Err(Failure::Transport {
      message: format!("{key}: {why}"),
    }),
    Ending::Blocked { .. } => Err(Failure::Person),
    Ending::Untrusted { folder, config, .. } => Err(Failure::Untrusted { folder, config }),
    Ending::Overran { .. } => Err(died(format!("{key}: the run was cut on its caller's verdict"))),
    Ending::Silent { .. } => {
      Err(died(format!("{key}: the run came back to rest with no answer in its record")))
    }
    Ending::Unsubmitted { why, .. } => {
      Err(died(format!("{key}: the program never took the run's words: {why}")))
    }
  }
}
