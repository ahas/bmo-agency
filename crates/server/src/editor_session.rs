//! An editor work key's runs as the session contract carries them (docs/api.md, 「Sessions」;
//! docs/launch-lines.md, 「Editor runs」): each turn of the key's editor session is one run apart of the
//! editor line under that key, its prompt the turn's, on a session of its own that nothing resumes, on the
//! login the walk of Claude Code's logins offers the key, in the brain folder the configuration names. What
//! the run spent and the windows it read are published as every run's are; its answer is the turn's.

use std::time::Instant;

use policy::work_keys::WORK_KEYS;

use crate::companion::run::Watched;
use crate::control::claude_accounts::Account;
use crate::door::apart::{self, Apart, answered, new_session};
use crate::door::record::{Reply, UsageSpent, WindowsRead};
use crate::job_run::claude::failure::Failure;
use crate::job_run::claude::walk::{Attempt, Walking, on_accounts};
use crate::lines::Trees;
use crate::lines::claude::{Command, Runs, command_env};
use crate::lines::editor::{EditorRun, LINE, PROVIDER, editor_run};
use crate::now_ms;
use crate::places::Ground;
use crate::run_log::{RunLog, Target};
use crate::worker_session::records::slot_account;

// The kind of work key whose runs an editor session carries.
const EDITOR_KIND: &str = "editor";

/// Whether a work key is an editor's, whose runs an editor session carries.
pub fn is_editor(key: &str) -> bool {
  WORK_KEYS.profile(key).is_some_and(|profile| profile.kind == EDITOR_KIND)
}

/// The key's run as it would go out now, or why it would not: the key resolving to no model on Claude Code,
/// or the brain folder no repository yet.
pub async fn composed(ground: &Ground, key: &str) -> Result<EditorRun, String> {
  let trees = Trees::of(ground);
  let brain = ground.config.brain_root(&ground.roots.data);

  editor_run(ground, &trees, &brain, key).await.map_err(|why| why.detail)?.ok_or_else(|| {
    format!(
      "{key} has no road to run on: its work resolves to no model on Claude Code, or the brain at {} is no \
       repository yet",
      brain.display()
    )
  })
}

// One login's run of the editor line: composed with the login's variables, run apart in the brain folder,
// its record fed into the turn's log as it is written.
struct OnLogin<'a> {
  ground: &'a Ground,
  editor: &'a EditorRun,
  prompt: &'a str,
  run: &'a str,
  log: &'a mut RunLog,
}

impl Attempt<Reply> for OnLogin<'_> {
  async fn attempt(&mut self, account: Option<Account>) -> Result<Reply, Failure> {
    let ground = self.ground;
    let editor = self.editor;
    let session = new_session();
    let args = editor.args(&session).map_err(|why| Failure::error(why.detail))?;
    let env = command_env(ground.env(), account.as_ref(), &Runs::Here, editor.thinking)
      .map_err(|why| Failure::error(why.detail))?;
    let command = Command {
      args,
      cwd: editor.cwd.to_string_lossy().into_owned(),
      env,
    };
    let mut watched = Watched {
      ground,
      log: &mut *self.log,
      source: PROVIDER,
      run: self.run.to_owned(),
      guard: None,
      tally: None,
      started: Instant::now(),
      last_tally: Instant::now(),
      cut: None,
    };
    let ended = apart::run(
      ground,
      Apart {
        agent: &editor.key,
        line: LINE,
        provider: PROVIDER,
        title: format!("{} run", editor.key),
        command,
        session,
        prompt: self.prompt.to_owned(),
        work_key: editor.key.clone(),
        model: editor.model.clone(),
        effort: editor.effort.clone(),
        tier: Some(editor.tier.clone()),
        bound: None,
      },
      &mut watched,
    )
    .await
    .map_err(Failure::error)?;
    let reply = answered(ended, &editor.key)?;

    Ok(Reply {
      account: account.map(|account| account.id),
      ..reply
    })
  }
}

/// One turn of a key's editor session: its run carried on the logins its key is offered, what it spent and
/// the windows it read published, and its answer's text; why it gave none otherwise.
pub async fn editor_turn(ground: &Ground, key: &str, run: &str, prompt: &str) -> Result<String, String> {
  let editor = composed(ground, key).await?;
  let mut log = RunLog::new(Target {
    agent: key.to_owned(),
    run: run.to_owned(),
    chat_id: None,
    job_id: None,
  });
  let mut attempt = OnLogin {
    ground,
    editor: &editor,
    prompt,
    run,
    log: &mut log,
  };
  let reply = on_accounts(&Walking { ground }, key, None, &mut attempt)
    .await
    .map_err(|failure| failure.message())?;
  let at = now_ms() / 1000;
  let account = slot_account(&reply.mode, reply.account.as_deref());

  if let Some(spent) = UsageSpent::of(key, Some(run), at, &reply.spend, |_| account.clone()) {
    ground.events.publish(spent);
  }

  if let Some(read) = WindowsRead::of_reply(&reply, at) {
    ground.events.publish(read);
  }

  if reply.text.trim().is_empty() {
    return Err(format!("{key}: the run ended without a readable answer"));
  }

  Ok(reply.text)
}
