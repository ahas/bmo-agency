//! A job's turn on its worker's own session (docs/api.md, 「Sessions」, a work turn's parts): carried by the
//! logins its work key is offered for its user, each tried in turn where one fails on the login rather
//! than on the work; on the model, the mapping and the session its runner named, behind the agent lock the
//! worker's jobs share, with the owner's waiting reset carried out, the standby claimed and the held session
//! judged before it goes; in its folder, under its run name and title, its variables beside the line's
//! own; and read back as the whole of its end — the reply, or the failure by its kind — with what it went
//! out on, where its time went and whether its session was compacted.
//!
//! Which model, which session, which login and which folder a job's turn needs is its runner's: this
//! carries that choice through the worker's records, the held session's judgement and the door, and
//! judges none of it. Where the session the turn settles on is not the one its runner composed its prompt
//! for, nothing goes in and the turn goes back to its runner with the session it would continue.
//!
//! A thrown-away session's turn — a co-work branch's or a research run's — is the same turn on a session
//! nothing keeps: on the provider, model and folder its scope names, always fresh, with nothing of a
//! worker's records read, judged, recorded or remembered for it, in the standing branch checkout its
//! folder is or in a pane of its own.

use agent_session::{
  self as contract, Apart, EngineError, Fault, FaultKind, JobMark, Outcome, Resume, SessionId, TurnEnd,
  WentOut, WorkParts,
};
use serde_json::Value;

use super::failure::{AccountFailure, Failure, RunEnding, of_unanswered};
use super::reset::carry_out_worker_reset;
use super::walk::{Attempt, Walking, on_accounts};
use crate::Refusal;
use crate::control::claude_accounts::Account;
use crate::control::harness::provider_enabled;
use crate::door::confirm::{Asking, Confirmation};
use crate::door::execution::{cancelled, execution_root};
use crate::door::record::Reply;
use crate::door::turn::{Ending, Turn, Watch, put};
use crate::keepalive::logins;
use crate::lines::Trees;
use crate::lines::place::Place;
use crate::lines::standing::{claim_standby, session_ttl_ms, worker_prefix_digest};
use crate::lines::worker::{WorkerRun, effort_of, worker_run};
use crate::places::branch::{self, BranchRun};
use crate::places::home::Home;
use crate::places::{AgentRow, Ground, locks};
use crate::queue::stops::job_stop_reason;
use crate::run_log::lines::Line;
use crate::run_log::spans::JobSpan;
use crate::run_log::{RunLog, Target};
use crate::session_questions::{Questions, TurnQuestion};
use crate::worker_session::fold::{fold, worker_session_full};
use crate::worker_session::held::{self, SessionRun};
use crate::worker_session::records::{self, fallen_model, heal, launch_model, note_carried, record};
use crate::worker_session::{
  Composed, PROVIDER, UNKNOWN_SESSION, compose, compose_at, compose_own, evidence,
};

/// The variable the agent a turn's program is a turn of goes by, so what the verbs it runs file stands
/// under that agent.
pub const TURN_AGENT: &str = "BMO_TURN_AGENT";

/// The variable that tells the verbs a turn's program runs that they work for the turn's job.
pub const TASK_LOCKED: &str = "BMO_TASK_LOCKED";

/// A job's turn as its runner asks it: the agent it runs as, the session it goes on, its work parts, the
/// job it is of, and its words.
pub struct WorkAsked<'a> {
  pub ground: &'a Ground,
  pub agent: &'a str,
  pub seat: WorkSeat<'a>,
  pub work: &'a WorkParts,
  pub job: Option<JobMark>,
  pub prompt: &'a str,
  /// The session the turn goes on, which a question the turn stops on is put to the caller under.
  pub session: &'a SessionId,
  pub questions: &'a Questions,
}

/// The session a job's turn goes on: the worker's own, by its row, or one thrown away, by its scope.
#[derive(Clone, Copy)]
pub enum WorkSeat<'a> {
  Worker(&'a AgentRow),
  Apart(&'a Apart),
}

/// Where a thrown-away session's turn goes: the standing branch checkout its folder is, under the co-work
/// task's label, or a pane of its own on this machine's herdr.
#[derive(Clone, Debug)]
pub enum ApartSeat {
  Standing(Home),
  Own(herdr::Herdr),
}

/// Where a thrown-away session's turn goes, by the folder its scope names; none where it names no
/// standing checkout and this machine names no herdr to open a pane on.
pub fn apart_seat(ground: &Ground, apart: &Apart) -> Option<ApartSeat> {
  let run = match (apart.todo, &apart.role) {
    (Some(todo), Some(role)) => Some(BranchRun {
      todo,
      role: role.clone(),
    }),
    _ => None,
  };

  match branch::home(ground, &apart.cwd, run.as_ref()) {
    Some(home) => Some(ApartSeat::Standing(home)),
    None if branch::place_of(&ground.roots.data, &apart.cwd).is_some() => None,
    None => ground.herdr.clone().map(ApartSeat::Own),
  }
}

/// Which session an attempt goes on, or that the turn goes back to its runner with the session it would
/// continue, none for a fresh one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
  Go(Option<String>),
  Back(Option<String>),
}

/// The session a turn's `resume` goes on, given the one the worker keeps for the turn's model and the login
/// carrying it: under `auto` the kept one, under `fresh` none, and under `session` the one named where it
/// is the kept one; a session named that the login carrying the turn does not keep is not the one the
/// turn's prompt was composed for.
pub fn choose(resume: &Resume, kept: Option<String>) -> Choice {
  match resume {
    Resume::Auto => Choice::Go(kept),
    Resume::Fresh => Choice::Go(None),
    Resume::Session { id } if kept.as_deref() == Some(id.as_str()) => Choice::Go(kept),
    Resume::Session { .. } => Choice::Back(kept),
  }
}

/// What a turn that did not answer on a resumed session comes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unanswered {
  /// The program no longer held the session: under `auto`, asked again once on a fresh one.
  Again,
  /// The program no longer held the session the runner named: back to the runner for a fresh one.
  Back,
  /// Read as a failure.
  Failed,
}

/// What a turn that left no answer comes to, by the session it went on and what it left.
pub fn after_unanswered(resume: &Resume, went_on: Option<&str>, evidence: &str) -> Unanswered {
  if went_on.is_none() || !evidence.contains(UNKNOWN_SESSION) {
    return Unanswered::Failed;
  }

  match resume {
    Resume::Session { .. } => Unanswered::Back,
    Resume::Auto | Resume::Fresh => Unanswered::Again,
  }
}

/// The composed turn handed what its runner named beyond the line: its variables beside the line's own,
/// the agent it is a turn of, the job marker where it has a job and none otherwise, its work key, and the
/// unit its execution is a child of and counted under, so a stop of the unit reaches it.
pub fn with_work(mut turn: Turn, agent: &str, work: &WorkParts, job: Option<JobMark>) -> Turn {
  for (name, value) in &work.variables {
    turn.env.entry(name.clone()).or_insert_with(|| value.clone());
  }

  turn.env.insert(TURN_AGENT.to_owned(), agent.to_owned());

  if job.is_some() {
    turn.env.insert(TASK_LOCKED.to_owned(), "1".to_owned());
  } else {
    turn.env.remove(TASK_LOCKED);
  }

  turn.identity.work_key = Some(work.work_key.clone());

  if let Some(unit) = &work.unit {
    turn.identity.parent = Some(unit.clone());
    turn.identity.root = Some(execution_root(unit));
  }

  turn
}

/// Whether any of the record's lines is the program compacting its session.
pub fn compacts(lines: &[String]) -> bool {
  lines.iter().any(|line| {
    serde_json::from_str::<Value>(line).is_ok_and(|event| {
      event.get("type").and_then(Value::as_str) == Some("system")
        && event.get("subtype").and_then(Value::as_str) == Some("compact_boundary")
    })
  })
}

/// What a line went out on, as a turn's end names it.
pub fn went_out_of(line: &WorkerRun) -> WentOut {
  WentOut {
    provider: PROVIDER.to_owned(),
    model: line.model.clone(),
    effort: (!line.effort.is_empty()).then(|| line.effort.clone()),
    tier: line.tier.clone(),
  }
}

/// A reply as the session contract carries it. The Claude Code line goes to no upstream of its own.
pub fn reply_of(reply: &Reply) -> contract::Reply {
  contract::Reply {
    mode: reply.mode.clone(),
    model: reply.model.clone(),
    effort: reply.effort.clone(),
    text: reply.text.clone(),
    session_id: reply.session_id.clone(),
    account: reply.account.clone(),
    context_ratio: reply.context_ratio,
    context_tokens: reply.context_tokens,
    session_usage: reply.session_usage.as_ref().and_then(|usage| serde_json::to_value(usage).ok()),
    spend: reply
      .spend
      .iter()
      .map(|spent| contract::Spend {
        mode: (!spent.mode.is_empty()).then(|| spent.mode.clone()),
        model: spent.model.clone(),
        tokens: spent.tokens,
        input_tokens: spent.input_tokens,
        output_tokens: spent.output_tokens,
        cache_read_tokens: spent.cache_read_tokens,
        cache_write_tokens: spent.cache_write_tokens,
        cost: Some(spent.cost),
      })
      .collect(),
    live_tasks: reply
      .live_tasks
      .iter()
      .map(|task| contract::LiveTask {
        id: task.id.clone(),
        description: task.description.clone(),
      })
      .collect(),
    workflows: reply
      .workflows
      .iter()
      .map(|workflow| contract::Workflow {
        call: workflow.call.clone(),
        name: workflow.name.clone(),
        status: workflow.status.clone(),
      })
      .collect(),
    upstream: None,
    request_usages: reply.request_usages.as_ref().map(|usages| {
      usages
        .iter()
        .map(|usage| contract::RequestUsage {
          input_tokens: usage.input_tokens,
          output_tokens: usage.output_tokens,
          cache_read_tokens: usage.cache_read_tokens,
          cache_write_tokens: usage.cache_write_tokens,
        })
        .collect()
    }),
  }
}

/// A failed turn as the session contract carries it. A walk's own endings, which a single turn never
/// reaches, are read as errors by their words.
pub fn failure_of(failure: &Failure) -> contract::Failure {
  match failure {
    Failure::Limit {
      resets_at,
      account,
      silenced,
    } => contract::Failure::Limit {
      resets_at: *resets_at,
      account: account.clone(),
      silenced: *silenced,
    },
    Failure::Account { failure, detail } => contract::Failure::Account {
      failure: match failure {
        AccountFailure::Limit => contract::AccountFailure::Limit,
        AccountFailure::Login => contract::AccountFailure::Login,
        AccountFailure::Outage => contract::AccountFailure::Outage,
        AccountFailure::NodeToken => contract::AccountFailure::NodeToken,
      },
      detail: detail.clone(),
    },
    Failure::Died {
      ending,
      said,
      message,
    } => contract::Failure::Died {
      ending: match ending {
        RunEnding::Exit { code } => contract::RunEnding::Exit { code: *code },
      },
      said: said.clone(),
      message: message.clone(),
    },
    Failure::Person => contract::Failure::Person,
    Failure::Untrusted { folder, config } => contract::Failure::Untrusted {
      folder: folder.clone(),
      config: config.clone(),
    },
    Failure::Stopped => contract::Failure::Stopped,
    Failure::Transport { message } => contract::Failure::Transport {
      message: message.clone(),
    },
    Failure::Refused { message } => contract::Failure::Refused {
      message: message.clone(),
    },
    Failure::Unavailable {
      scope,
      mode,
      reason,
    } => contract::Failure::Unavailable {
      scope: scope.clone(),
      mode: mode.clone(),
      reason: reason.clone(),
    },
    Failure::Error { name, message } => contract::Failure::Error {
      name: name.clone(),
      message: message.clone(),
    },
    Failure::Attempt | Failure::Exhausted { .. } => contract::Failure::Error {
      name: "Error".to_owned(),
      message: failure.message(),
    },
  }
}

/// Where one part of a turn's time went, as the session contract carries it.
pub fn span_of(span: &JobSpan) -> contract::Span {
  contract::Span {
    kind: span.kind.clone(),
    name: span.name.clone(),
    seconds: span.seconds,
    times: span.times,
    tokens: span.tokens,
    input_tokens: span.input_tokens,
    output_tokens: span.output_tokens,
    cache_read_tokens: span.cache_read_tokens,
    cache_write_tokens: span.cache_write_tokens,
  }
}

/// What a job's turn shows of itself while it goes, to whoever reads its session: what each attempt went
/// out on, and the words its program writes of its answer, an attempt's going out beginning them again.
pub trait Shows: Sync {
  fn went_out(&self, went_out: &WentOut);

  fn wrote(&self, texts: &[String]);
}

/// Every text block the program's answers wrote in the record's lines, in order: what a turn stopped from
/// outside had written.
pub fn assistant_texts(lines: &[String]) -> Vec<String> {
  lines
    .iter()
    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    .filter(|event| event.get("type").and_then(Value::as_str) == Some("assistant"))
    .flat_map(|event| {
      event.pointer("/message/content").and_then(Value::as_array).cloned().unwrap_or_default()
    })
    .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
    .filter_map(|block| block.get("text").and_then(Value::as_str).map(str::to_owned))
    .collect()
}

/// What a carried turn settled on: its answer, or its going back to its runner with the session it would
/// continue.
#[derive(Debug)]
pub enum Settled {
  Answered(Box<Reply>),
  Back(Option<String>),
}

/// What a turn came to beside its answer or its failure: what it went out on, where its time went, whether
/// its session was compacted, and the error its engine ended it with where it ended with one.
#[derive(Debug, Default)]
pub struct Beside {
  pub went_out: Option<WentOut>,
  pub spans: Vec<contract::Span>,
  pub compacted: bool,
  pub engine_error: Option<EngineError>,
}

/// A carried turn's end under its run: an answer, a turn back to its runner, a person waited on, a stop,
/// or a failure with its words and its kind.
pub fn ended(run: &str, walked: Result<Settled, Failure>, beside: Beside) -> TurnEnd {
  let end = match walked {
    Ok(Settled::Answered(reply)) => TurnEnd {
      reply: Some(Box::new(reply_of(&reply))),
      ..TurnEnd::answered(run, reply.text.clone())
    },
    Ok(Settled::Back(continues)) => TurnEnd {
      continues,
      ..TurnEnd::of(run, Outcome::Reprepared)
    },
    Err(Failure::Person) => TurnEnd {
      failure: Some(contract::Failure::Person),
      ..TurnEnd::of(run, Outcome::Blocked)
    },
    Err(Failure::Stopped) => TurnEnd {
      failure: Some(contract::Failure::Stopped),
      ..TurnEnd::of(run, Outcome::Stopped)
    },
    Err(failure) => TurnEnd {
      failure: Some(failure_of(&failure)),
      ..TurnEnd::failed(run, failure.message(), beside.engine_error.clone())
    },
  };

  TurnEnd {
    went_out: beside.went_out,
    spans: beside.spans,
    compacted: beside.compacted,
    ..end
  }
}

/// One job's turn carried on its session to its end. A turn on a provider other than Claude Code, or naming
/// a user that is no user's id, is refused; every other way the turn ends is its end.
pub async fn carry(asked: &WorkAsked<'_>, shown: &dyn Shows) -> Result<TurnEnd, Fault> {
  let work = asked.work;
  let provider = match asked.seat {
    WorkSeat::Worker(_) => &work.provider,
    WorkSeat::Apart(apart) => &apart.provider,
  };

  if provider != PROVIDER {
    return Err(Fault::new(
      FaultKind::Refused,
      format!("{}: a job's turn goes out on {PROVIDER}, not {provider}", asked.agent),
    ));
  }

  let user = match work.user.as_deref().map(str::trim).filter(|user| !user.is_empty()) {
    None => None,
    Some(user) => Some(
      user
        .parse::<i64>()
        .map_err(|_| Fault::new(FaultKind::Refused, format!("{user} is not a user's id")))?,
    ),
  };
  let mut log = RunLog::new(Target {
    agent: asked.agent.to_owned(),
    run: work.unit.clone().unwrap_or_else(|| work.name.clone()),
    chat_id: Some(work.chat),
    job_id: asked.job.map(|job| job.job),
  });
  let mut attempting = Attempting {
    asked,
    log: &mut log,
    shown,
    beside: Beside::default(),
  };
  let walked = on_accounts(
    &Walking {
      ground: asked.ground,
    },
    &work.work_key,
    user,
    &mut attempting,
  )
  .await;
  let mut beside = attempting.beside;

  beside.spans = log.spans().iter().map(span_of).collect();

  Ok(ended(&work.name, walked, beside))
}

/// What an answered job turn says about usage, recorded under its agent and the run it is logged under: the
/// agent's last answer written, the windows its reply read kept, and what it spent published.
pub async fn noted(ground: &Ground, agent: &str, run: &str, reply: &Reply) -> Result<(), Refusal> {
  records::note_job_usage(&ground.db, &ground.windows, &ground.events, agent, reply, Some(run)).await
}

fn refused(why: Refusal) -> Failure {
  Failure::error(why.detail)
}

// What one primary run left: its answer, a turn back to its runner, or the text a turn that did not
// answer left behind.
enum Primary {
  Answered(Box<Reply>),
  Back(Option<String>),
  Unanswered(String),
}

// The turn on the logins the walk offers, and what it learned beside its end.
struct Attempting<'r, 'a> {
  asked: &'r WorkAsked<'a>,
  log: &'r mut RunLog,
  shown: &'r dyn Shows,
  beside: Beside,
}

impl Attempt<Settled> for Attempting<'_, '_> {
  // On a work node the login is the account's token for the nodes (docs/work-nodes.md, 「The logins on a
  // node」): one marked gone is passed over without a run, and a run there that fails on its login has
  // found that token gone rather than the account's login here.
  async fn attempt(&mut self, account: Option<Account>) -> Result<Settled, Failure> {
    let ground = self.asked.ground;
    let data = &ground.roots.data;
    let on_node = self.on_node();

    if on_node
      && let Some(held) = &account
      && logins::node_token_gone(&ground.db, held, data).await.map_err(refused)?
    {
      return Err(Failure::Account {
        failure: AccountFailure::NodeToken,
        detail: format!("claude account {}: its token for the work nodes is gone", held.id),
      });
    }

    match self.on_login(account).await {
      Err(failed) if on_node && failed.account_failure() == Some(AccountFailure::Login) => {
        Err(Failure::Account {
          failure: AccountFailure::NodeToken,
          detail: failed.message(),
        })
      }
      tried => tried,
    }
  }

  // A turn into a program on a work node never goes out on an endpoint account, whose key stays on this
  // machine (docs/work-nodes.md).
  fn takes(&self, account: &Account) -> bool {
    account.credential.runs_on_nodes() || !self.on_node()
  }
}

impl Attempting<'_, '_> {
  // Whether the turn goes into a standing program on a work node.
  fn on_node(&self) -> bool {
    let data = &self.asked.ground.roots.data;

    self.standing_agent().is_some_and(|agent| Place::of(data, &agent).node().is_some())
  }

  // The agent whose standing program the turn goes into: the worker, or the standing branch checkout a
  // thrown-away session's folder is; none for a pane of the turn's own.
  fn standing_agent(&self) -> Option<String> {
    match self.asked.seat {
      WorkSeat::Worker(row) => Some(row.name.clone()),
      WorkSeat::Apart(apart) => match apart_seat(self.asked.ground, apart) {
        Some(ApartSeat::Standing(home)) => Some(home.agent),
        _ => None,
      },
    }
  }

  // One login's attempt: the owner's reset and the standby first, the session the turn's resume settles on,
  // and the turn asked again on a fresh session where the program has lost the one it resumed. A
  // thrown-away session's turn reads none of a worker's sessions and resumes nothing.
  async fn on_login(&mut self, account: Option<Account>) -> Result<Settled, Failure> {
    let ground = self.asked.ground;
    let work = self.asked.work;
    let worker = self.asked.agent;

    if let WorkSeat::Apart(_) = self.asked.seat {
      return match self.primary(None, account).await? {
        Primary::Answered(reply) => Ok(Settled::Answered(reply)),
        Primary::Back(continues) => Ok(Settled::Back(continues)),
        Primary::Unanswered(left) => Err(of_unanswered(&left, &work.work_key)),
      };
    }
    let recorded = account.as_ref().map_or_else(
      || crate::control::claude_accounts::SEED_ACCOUNT.to_owned(),
      |account| account.id.clone(),
    );

    if carry_out_worker_reset(ground, worker).await.map_err(refused)? {
      println!("{worker}: its sessions are cleared as the owner asked, so this job opens a new one");
    }

    claim_standby(ground, worker, &Place::of(&ground.roots.data, worker))
      .await
      .map_err(refused)?;

    let ttl = session_ttl_ms(ground, &work.model).await;
    let kept = records::resumable(&ground.db, worker, &work.model, &recorded, ttl)
      .await
      .map_err(refused)?;
    let resume = match choose(&work.resume, kept) {
      Choice::Go(resume) => resume,
      Choice::Back(continues) => return Ok(Settled::Back(continues)),
    };
    let left = match self.primary(resume.clone(), account.clone()).await? {
      Primary::Answered(reply) => return Ok(Settled::Answered(reply)),
      Primary::Back(continues) => return Ok(Settled::Back(continues)),
      Primary::Unanswered(left) => left,
    };

    match after_unanswered(&work.resume, resume.as_deref(), &left) {
      Unanswered::Failed => Err(of_unanswered(&left, &work.work_key)),
      Unanswered::Back => {
        records::forget(&ground.db, worker, &work.model, Some(&recorded))
          .await
          .map_err(refused)?;

        Ok(Settled::Back(None))
      }
      Unanswered::Again => {
        records::forget(&ground.db, worker, &work.model, Some(&recorded))
          .await
          .map_err(refused)?;

        match self.primary(None, account).await? {
          Primary::Answered(reply) => Ok(Settled::Answered(reply)),
          Primary::Back(continues) => Ok(Settled::Back(continues)),
          Primary::Unanswered(left) => Err(of_unanswered(&left, &work.work_key)),
        }
      }
    }
  }

  // The run on the worker line with the turn's mapping laid on it, delivered under the agent's lock: the
  // held session judged, the turn put in, and the session that answered kept.
  async fn primary(
    &mut self,
    resume: Option<String>,
    account: Option<Account>,
  ) -> Result<Primary, Failure> {
    let ground = self.asked.ground;
    let db = &ground.db;
    let work = self.asked.work;
    let seat = self.asked.seat;
    let worker = self.asked.agent;
    let trees = Trees::of(ground);
    let machine = ground.machine();
    // A thrown-away session goes out on the model and in the folder its scope names.
    let (kept_on, model, cwd) = match seat {
      WorkSeat::Worker(_) => (&work.model, &work.mapping.model, &work.cwd),
      WorkSeat::Apart(apart) => (&apart.model, &apart.model, &apart.cwd),
    };
    let mut line = worker_run(ground, &trees, Some(worker), kept_on, cwd).await.map_err(refused)?;

    line.effort = effort_of(
      ground,
      &machine,
      PROVIDER,
      model,
      work.mapping.effort.as_deref(),
      line.reasoning.as_ref(),
    )
    .await
    .map_err(refused)?
    .unwrap_or_default();
    line.model.clone_from(model);
    line.tier.clone_from(&work.mapping.tier);

    // A kept session the safeguard refused goes out on the model that answered it, whatever the mapping
    // names; a fresh one goes out on its own model.
    let own = line.model.clone();

    if let Some(held) = &resume {
      let mark = records::sessions(db, worker)
        .await
        .map_err(refused)?
        .into_iter()
        .find(|one| &one.session_id == held)
        .and_then(|one| one.answered_model);

      line.model = launch_model(db, worker, mark.as_deref(), line.model).await.map_err(refused)?;
    }

    let lock = locks::agent_lock_path(&ground.roots.data, &held::scope_of(worker));
    let held_lock = locks::hold(lock).await.map_err(|why| Failure::error(format!("agent lock: {why}")))?;
    let session = SessionRun {
      mode: held::MODE,
      line,
      prompt: self.asked.prompt.to_owned(),
      resume,
      account,
      placement: None,
      thread: work.thread.clone(),
    };
    // A session thrown away is no session the worker holds, and is judged by nothing.
    let prepared = match seat {
      WorkSeat::Worker(_) => held::prepare(ground, worker, session).await.map_err(refused)?,
      WorkSeat::Apart(_) => session,
    };

    if !provider_enabled(db, PROVIDER).await.map_err(|why| Failure::error(why.error))? {
      return Err(Failure::Unavailable {
        scope: held::scope_of(worker),
        mode: PROVIDER.to_owned(),
        reason: "Claude Code is switched off".to_owned(),
      });
    }

    // The session the runner named, replaced by the judging, is not the one its prompt was composed for.
    if let (WorkSeat::Worker(_), Resume::Session { id }) = (seat, &work.resume)
      && prepared.resume.as_deref() != Some(id.as_str())
    {
      return Ok(Primary::Back(prepared.resume.clone()));
    }

    let mut sent = prepared.clone();

    // A session the preparation replaced is a fresh one, and a fresh session carries no mark.
    if sent.resume.is_none() && sent.line.model != own {
      sent.line.model.clone_from(&own);
    }

    let went_out = went_out_of(&sent.line);

    self.shown.went_out(&went_out);
    self.beside.went_out = Some(went_out);
    self
      .log
      .write(
        db,
        Line {
          source: Some(PROVIDER.to_owned()),
          state: Some("running"),
          ..Line::note(format!("{PROVIDER} · {}", sent.line.model))
        },
      )
      .await;

    let composing = Composed {
      name: work.name.clone(),
      title: work.title.clone(),
      prompt: sent.prompt.clone(),
      bound: None,
    };
    let account = sent.account.as_ref();
    let composed = match seat {
      WorkSeat::Worker(row) => {
        compose(ground, row, &sent.line, account, sent.resume.as_deref(), composing).await
      }
      WorkSeat::Apart(apart) => match apart_seat(ground, apart) {
        Some(ApartSeat::Standing(home)) => {
          let agent = home.agent.clone();

          compose_at(ground, &agent, home, &sent.line, account, None, composing).await
        }
        Some(ApartSeat::Own(herdr)) => compose_own(ground, herdr, &sent.line, account, composing),
        None => {
          return Err(Failure::Transport {
            message: format!("{worker}: this machine names no herdr for the place {cwd} stands in"),
          });
        }
      },
    }
    .map_err(refused)?;
    let turn = with_work(composed, worker, work, self.asked.job);
    let mut watch = WorkWatch {
      ground,
      log: self.log,
      compacted: &mut self.beside.compacted,
      session: self.asked.session,
      run: &work.name,
      questions: self.asked.questions,
      shown: self.shown,
    };
    let ending = put(ground, &turn, &mut watch).await.map_err(Failure::error)?;
    let primary = match ending {
      Ending::Blocked { .. } => Err(Failure::Person),
      Ending::Untrusted { folder, config, .. } => Err(Failure::Untrusted { folder, config }),
      Ending::Answered { reply, .. } => {
        noted(ground, worker, work.unit.as_deref().unwrap_or(&work.name), &reply)
          .await
          .map_err(refused)?;

        // A session thrown away keeps no record of what it was sent or of the model it fell to.
        if let WorkSeat::Worker(row) = seat {
          held::remember(ground, worker, &prepared, &reply).await.map_err(refused)?;
          self.kept(row, &sent, &own, &reply).await.map_err(refused)?;
        }

        Ok(Primary::Answered(reply))
      }
      other => {
        // Stopped from outside while the turn ran: the ending is the stop, however the pane looked.
        if self.stopped().await {
          Err(Failure::Stopped)
        } else {
          if let Ending::Errored { kind, .. } = &other {
            self.beside.engine_error = Some(EngineError { kind: kind.clone() });
          }

          Ok(Primary::Unanswered(evidence(&other).to_owned()))
        }
      }
    };

    drop(held_lock);
    primary
  }

  // The session that answered recorded for the turn's model and login — unless it answered on a lesser
  // model, when its record is dropped — with the context it carried, and folded once it is full.
  async fn kept(&self, row: &AgentRow, sent: &SessionRun, own: &str, reply: &Reply) -> Result<(), Refusal> {
    let ground = self.asked.ground;
    let work = self.asked.work;
    let worker = row.name.as_str();
    let Some(session_id) = reply.session_id.clone() else {
      println!("{worker}: the job's turn named no session, so there is none to hold open");

      return Ok(());
    };
    let account = sent.recorded_account();
    let job = self.asked.job.map(|job| job.job);
    let fell = heal(ground, worker, &work.model, &sent.line.model, &account, &reply.spend, job).await?;

    if fell.dropped {
      println!("{worker} {}: dropped {session_id}, the next job opens a fresh one", work.model);

      return Ok(());
    }

    let digest = worker_prefix_digest(
      ground,
      &Trees::of(ground),
      &work.cwd,
      &sent.line.model,
      Some(&sent.line.effort),
    )
    .await?;
    let fallen = fallen_model(own, &sent.line.model, &fell.models);

    record(&ground.db, worker, &work.model, &account, &session_id, Some(&digest), fallen.as_deref())
      .await?;
    note_carried(&ground.db, worker, &work.model, &account, sent.resume.is_none(), reply).await?;

    if worker_session_full(reply.context_ratio) {
      println!("{worker} {}: {session_id} is full, folding it", work.model);
      fold(ground, row, &work.model, &account, &session_id).await;
    }

    Ok(())
  }

  // Whether the turn's job was stopped: the owner's stop of its job, or a cancellation over its run or the
  // unit it runs under.
  async fn stopped(&self) -> bool {
    let db = &self.asked.ground.db;
    let work = self.asked.work;

    if let Some(job) = self.asked.job
      && job_stop_reason(db, job.job).await.ok().flatten().is_some()
    {
      return true;
    }

    let names = [work.name.as_str(), work.unit.as_deref().unwrap_or("")];

    cancelled(db, &names).await.unwrap_or(false)
  }
}

// What a job's turn is watched with: the run log it feeds, and whether the program compacted its session.
// A question the program stops on is put to the turn's caller over the session it goes on.
struct WorkWatch<'w> {
  ground: &'w Ground,
  log: &'w mut RunLog,
  compacted: &'w mut bool,
  session: &'w SessionId,
  run: &'w str,
  questions: &'w Questions,
  shown: &'w dyn Shows,
}

impl Watch for WorkWatch<'_> {
  async fn note(&mut self, text: &str) {
    self.log.write(&self.ground.db, Line::note(text)).await;
  }

  async fn record(&mut self, lines: &[String]) {
    let texts = assistant_texts(lines);

    if !texts.is_empty() {
      self.shown.wrote(&texts);
    }

    *self.compacted |= compacts(lines);
    self.log.feed(&self.ground.db, lines, PROVIDER).await;
  }

  fn puts_questions(&self) -> bool {
    true
  }

  async fn confirm(&mut self, asked: &Confirmation) -> Option<Asking> {
    let (question, asking) = self.questions.put(self.session, asked);

    self.ground.events.publish(TurnQuestion {
      session: self.session.0.clone(),
      run: self.run.to_owned(),
      question,
    });

    Some(asking)
  }
}

/// The harness session a worker's next turn on a model would continue now, as its record and the first
/// login its walk offers hold it, none for a fresh one.
pub async fn continues(ground: &Ground, worker: &str, model: &str) -> Result<Option<String>, Refusal> {
  let account = crate::worker_session::walked_account(ground)
    .await?
    .map_or_else(|| crate::control::claude_accounts::SEED_ACCOUNT.to_owned(), |account| account.id);
  let ttl = session_ttl_ms(ground, model).await;

  records::resumable(&ground.db, worker, model, &account, ttl).await
}
