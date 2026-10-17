//! An editor work key's runs as the session contract carries them (docs/api.md, 「Sessions」;
//! docs/launch-lines.md, 「Editor runs」): each turn of the key's editor session is one run apart of the
//! editor line under that key, its prompt the turn's, on a session of its own that nothing resumes, in the
//! brain folder the configuration names. The key's walk goes first: each stand-in it meets runs the line on
//! its own road — Codex's `exec` and dsh in the host, a registered model's Claude Code on the gateway in a
//! pane of its own — and a stand-in answering where the key walks is published as the fallback it is. Where
//! none answered and the walk need not answer, the run goes out on the login the walk of Claude Code's
//! logins offers the key. What the run spent and the windows it read are published as every run's are; its
//! answer is the turn's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use harness_providers::Machine;
use policy::providers::openrouter::OPENROUTER;
use policy::providers::registered::REGISTERED;
use policy::work_keys::WORK_KEYS;
use sea_orm::DatabaseConnection;

use crate::chat_stand_in::owner_id;
use crate::companion::run::Watched;
use crate::control::claude_accounts::Account;
use crate::control::harness::provider_enabled;
use crate::control::registry;
use crate::door::apart::{self, Apart, answered, new_session};
use crate::door::execution::{execution_root, own_unit};
use crate::door::record::{ModelSpend, Reply, UsageSpent, WindowsRead};
use crate::host_run::{self, Asked, Heard, Ran};
use crate::job_run::claude::failure::Failure;
use crate::job_run::claude::walk::{Attempt, Walking, on_accounts};
use crate::job_run::openrouter::observed;
use crate::job_run::openrouter_tap;
use crate::lines::Trees;
use crate::lines::claude::{Command, Runs, command_env};
use crate::lines::codex::{self, Shape};
use crate::lines::dsh;
use crate::lines::editor::{ALLOWED_TOOLS, EditorRun, FILE_TOOLS, LINE, PROVIDER, editor_run, mcp_config};
use crate::lines::openrouter::{self, LOCAL_URL};
use crate::lines::standing::{OnGateway, Placing, gateway_command, place_registered};
use crate::lines::worker::reasoning_of;
use crate::places::Ground;
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::stand_in::{CLAUDE, Step, Walk, announce, listed, step_of, walk_of};
use crate::worker_session::records::slot_account;
use crate::{Refusal, now_ms};

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

/// One turn of a key's editor session: its run carried on the first stand-in of the key's walk that answers,
/// else on the logins its key is offered, what it spent and the windows it read published, and its answer's
/// text; why it gave none otherwise.
pub async fn editor_turn(ground: &Ground, key: &str, run: &str, prompt: &str) -> Result<String, String> {
  let brain = brain_of(ground)?;
  let mut log = RunLog::new(Target {
    agent: key.to_owned(),
    run: run.to_owned(),
    chat_id: None,
    job_id: None,
  });
  let reply = match walked(ground, key, prompt, &brain, &mut log).await? {
    Some(reply) => reply,
    None => on_claude(ground, key, run, prompt, &mut log).await?,
  };
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

// The brain folder an editor's run works in, where it is a repository.
fn brain_of(ground: &Ground) -> Result<PathBuf, String> {
  let brain = ground.config.brain_root(&ground.roots.data);

  if !brain.join(".git").exists() {
    return Err(format!("the brain at {} is no repository yet", brain.display()));
  }

  Ok(brain)
}

// The run on Claude Code's line, carried on the logins its key is offered.
async fn on_claude(
  ground: &Ground,
  key: &str,
  run: &str,
  prompt: &str,
  log: &mut RunLog,
) -> Result<Reply, String> {
  if !provider_enabled(&ground.db, CLAUDE).await.map_err(|why| why.error)? {
    return Err(format!("Claude Code is turned off, so the {key} work had no road to run on"));
  }

  let editor = composed(ground, key).await?;
  let mut attempt = OnLogin {
    ground,
    editor: &editor,
    prompt,
    run,
    log,
  };

  on_accounts(&Walking { ground }, key, None, &mut attempt)
    .await
    .map_err(|failure| failure.message())
}

/// The key's walk over its stand-ins for one run (docs/cache-fallback.md): none where Claude Code is the road
/// it stands on, or no stand-in answered and Claude Code may still be asked; the first stand-in's reply that
/// answered; and why where the walk had to answer and none did.
pub async fn walked(
  ground: &Ground,
  key: &str,
  prompt: &str,
  brain: &Path,
  log: &mut RunLog,
) -> Result<Option<Reply>, String> {
  let machine = ground.machine();
  let Some(Walk {
    steps,
    required,
    chosen,
  }) = walk_of(ground, ground.peers.cache_siren.as_ref(), &machine, key).await
  else {
    return Ok(None);
  };
  let db = &ground.db;
  let reasoning = reasoning_of(ground, key).await.map_err(|why| why.detail)?;
  let mut last = None;

  for provider in &steps {
    if !provider_enabled(db, provider).await.map_err(|why| why.error)? {
      continue;
    }

    let step = match step_of(ground, &machine, key, provider, &reasoning).await {
      Ok(step) => step,
      Err(why) => {
        last = Some(why);
        continue;
      }
    };

    match stand_in(ground, &machine, key, &step, prompt, brain, log).await {
      Ok(reply) => {
        if provider != &chosen
          && let Err(why) = announce(ground, &ground.events, owner_id(ground), provider, &step.model).await
        {
          eprintln!("{key}: its answering on {provider} was not marked: {}", why.detail);
        }

        return Ok(Some(reply));
      }
      Err(why) => last = Some(why),
    }
  }

  if required || !provider_enabled(db, CLAUDE).await.map_err(|why| why.error)? {
    return Err(format!(
      "The provider chosen for the {key} work ({chosen}) did not answer, and this work never moves to another \
       provider on its own{}",
      last.map(|why| format!(": {why}")).unwrap_or_default()
    ));
  }

  Ok(None)
}

fn noted(step: &Step, kind: &'static str, text: String, state: Option<&'static str>) -> Line {
  Line {
    kind,
    state,
    source: Some(step.provider.clone()),
    ..Line::note(text)
  }
}

// One stand-in step's run of the editor line on its road, its start and end written into the log under the
// road's own name.
async fn stand_in(
  ground: &Ground,
  machine: &Machine,
  key: &str,
  step: &Step,
  prompt: &str,
  brain: &Path,
  log: &mut RunLog,
) -> Result<Reply, String> {
  let db = &ground.db;

  log
    .write(db, noted(step, "start", format!("{} · {}", step.provider, step.model), Some("running")))
    .await;
  log.launched(db, None, prompt, &[]).await;

  let road = Road {
    ground,
    machine,
    key,
    step,
    prompt,
    brain,
  };
  let answered = match step.provider.as_str() {
    codex::PROVIDER => road.on_codex(log).await,
    openrouter::PROVIDER => road.on_openrouter(log).await,
    _ => road.on_registration(log).await,
  };
  let (said, state) = match &answered {
    Ok(_) => (format!("{} · done", step.provider), "succeeded"),
    Err(why) => {
      log
        .write(
          db,
          noted(
            step,
            "note",
            format!("{} · {} did not answer — {why}", step.provider, step.model),
            Some("failed"),
          ),
        )
        .await;

      (format!("{} · error", step.provider), "failed")
    }
  };

  log.write(db, noted(step, "end", said, Some(state))).await;
  answered
}

// Whoever reads what a stand-in's program prints as it prints it: the turn's log, under the road's name.
struct Fed<'a> {
  db: &'a DatabaseConnection,
  log: &'a mut RunLog,
  source: &'a str,
  sessions: Option<&'a Path>,
}

impl Heard for Fed<'_> {
  async fn heard(&mut self, lines: &[String]) {
    match self.sessions {
      Some(sessions) => self.log.feed_codex(self.db, lines, self.source, sessions).await,
      None => {
        self.log.feed_stream(self.db, lines, self.source).await;
      }
    }
  }
}

// What one stand-in step's run stands on.
struct Road<'a> {
  ground: &'a Ground,
  machine: &'a Machine,
  key: &'a str,
  step: &'a Step,
  prompt: &'a str,
  brain: &'a Path,
}

impl Road<'_> {
  fn cwd(&self) -> String {
    self.brain.to_string_lossy().into_owned()
  }

  fn mcp(&self) -> Result<PathBuf, String> {
    mcp_config(self.ground, &Trees::of(self.ground)).map_err(|why: Refusal| why.detail)
  }

  // One process of the road in the host, its output fed into the log as it prints.
  async fn hosted(
    &self,
    log: &mut RunLog,
    provider: &str,
    command: host_run::Command,
    sessions: Option<&Path>,
  ) -> Result<Ran, String> {
    let own = own_unit(self.ground.env());
    let mut fed = Fed {
      db: &self.ground.db,
      log,
      source: provider,
      sessions,
    };

    host_run::run_heard(
      self.ground,
      Asked {
        agent: self.key,
        line: LINE,
        provider,
        command,
        root: Some(execution_root(own.as_deref().unwrap_or_default())),
        parent: own.clone(),
        work_key: Some(self.key.to_owned()),
        model: Some(self.step.model.clone()),
        effort: (!self.step.effort.is_empty()).then(|| self.step.effort.clone()),
        tier: None,
        session: None,
        timeout: None,
      },
      &mut fed,
    )
    .await
    .map_err(|failure| failure.said())
  }

  async fn on_codex(&self, log: &mut RunLog) -> Result<Reply, String> {
    let step = self.step;
    let shape = Shape::of_line(
      LINE,
      &step.model,
      &step.effort,
      step.thinking,
      &self.cwd(),
      None,
      Some(self.mcp()?),
      FILE_TOOLS,
      ALLOWED_TOOLS,
    );
    let command = codex::exec_command_of(&shape, false, None, self.prompt, self.ground.env())
      .map_err(|why| why.detail)?;
    let sessions = PathBuf::from(self.machine.var("HOME").unwrap_or_default())
      .join(".codex")
      .join("sessions");
    let ran = self.hosted(log, codex::PROVIDER, command, Some(&sessions)).await?;
    let reply =
      codex::exec_reply(&ran.stdout, ran.exit_code, &step.model, &step.effort, false, None, &sessions);

    if reply.text.is_empty() {
      return Err(codex::exec_failure(&ran.stdout, &ran.stderr, ran.exit_code));
    }

    Ok(reply)
  }

  async fn on_openrouter(&self, log: &mut RunLog) -> Result<Reply, String> {
    let step = self.step;
    let api = OPENROUTER.gateway.api_on(self.ground.vars()).to_owned();
    let tap = openrouter_tap::open(None, &api)
      .await
      .map_err(|why| format!("the OpenRouter tap: {why}"))?;
    let mut env: BTreeMap<String, String> = self.ground.env().clone();

    if let Some(key) = self.ground.vars().get(&OPENROUTER.presence.key_env) {
      env.insert(OPENROUTER.presence.key_env.clone(), key.clone());
    }

    env.insert(LOCAL_URL.to_owned(), tap.url.clone());

    let cwd = self.cwd();
    let mcp = self.mcp();
    let ran = match mcp {
      Ok(mcp) => {
        let composed = dsh::Composed {
          line: LINE,
          system: None,
          rules: None,
          shown: Some(FILE_TOOLS),
          allowed: Some(ALLOWED_TOOLS),
          cwd: &cwd,
          model: &step.model,
          effort: &step.effort,
          chosen: false,
          thinking: step.thinking,
          mcp: Some(&mcp),
        };

        match dsh::line_command(&composed, self.prompt, &env, &Trees::of(self.ground).agents) {
          Ok(command) => self.hosted(log, openrouter::PROVIDER, command, None).await,
          Err(why) => Err(why.detail),
        }
      }
      Err(why) => Err(why),
    };
    let seen = tap.observed();

    tap.close();

    let ran = ran?;
    let reply = dsh::reply(&ran.stdout, ran.exit_code, &step.model);

    if reply.text.is_empty() {
      return Err(dsh::failure(&ran.stderr, ran.exit_code));
    }

    Ok(Reply {
      mode: openrouter::PROVIDER.to_owned(),
      model: Some(step.model.clone()),
      effort: None,
      ..observed(reply, &step.model, &seen)
    })
  }

  // On a registered model, the editor line as Claude Code on the gateway, run apart in a pane of its own.
  async fn on_registration(&self, log: &mut RunLog) -> Result<Reply, String> {
    let (ground, step) = (self.ground, self.step);
    let service = registry::service_of(&ground.db, &step.model)
      .await
      .map_err(|why| why.error)?
      .ok_or_else(|| format!("{}: {}", REGISTERED.presence.not_registered, step.model))?;

    listed(ground, &service, &step.model).await?;

    let placement = place_registered(ground, &step.model).await.map_err(|why| why.detail)?;
    let cwd = self.cwd();
    let mcp = self.mcp()?;
    let session = new_session();
    let on = OnGateway {
      agent: self.key,
      model: &step.model,
      effort: &step.effort,
      cwd: &cwd,
      settings: serde_json::Map::new(),
      compose_typed: false,
      shell_guard: false,
      sampling: None,
    };
    let (command, _) = gateway_command(
      ground,
      self.machine,
      on,
      step.answer_tokens,
      Placing::At(placement),
      |name, effort, thinking, settings| {
        let run = EditorRun {
          key: self.key.to_owned(),
          model: name.to_owned(),
          effort: effort.to_owned(),
          tier: String::new(),
          mcp_config: mcp.clone(),
          thinking,
          cwd: self.brain.to_path_buf(),
        };
        let mut args = run.args(&session)?;

        args.splice(1..1, ["--settings".to_owned(), settings.to_string_lossy().into_owned()]);

        Ok(args)
      },
    )
    .await
    .map_err(|why| why.detail)?;
    let mut watched = Watched {
      ground,
      log,
      source: registry::PROVIDER,
      run: session.clone(),
      guard: None,
      tally: None,
      started: Instant::now(),
      last_tally: Instant::now(),
      cut: None,
    };
    let ended = apart::run(
      ground,
      Apart {
        agent: self.key,
        line: LINE,
        provider: registry::PROVIDER,
        title: format!("{} run", self.key),
        command: Command {
          args: command.args,
          cwd,
          env: command.env.into_iter().map(|(name, value)| (name, Some(value))).collect(),
        },
        session,
        prompt: self.prompt.to_owned(),
        work_key: self.key.to_owned(),
        model: step.model.clone(),
        effort: step.effort.clone(),
        tier: None,
        bound: None,
      },
      &mut watched,
    )
    .await?;
    let reply = answered(ended, self.key).map_err(|failure| failure.message())?;

    // On a registration every row is the registry's, and costs nothing.
    Ok(Reply {
      mode: registry::PROVIDER.to_owned(),
      model: Some(step.model.clone()),
      effort: step.thinking.then(|| "thinking".to_owned()),
      spend: reply
        .spend
        .iter()
        .cloned()
        .map(|row| ModelSpend {
          mode: registry::PROVIDER.to_owned(),
          cost: 0.0,
          ..row
        })
        .collect(),
      ..reply
    })
  }
}
