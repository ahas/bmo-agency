//! Research runs (docs/research-runs.md): a search at a depth or a browse, asked of agency on a research key,
//! sent out on the road the one road resolution answers for that key, as the runner's own line run apart in a
//! pane of its own, held to the research settings the owner keeps here, asked the line's research rounds
//! after its first answer, and its result queued for the room as it ends.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use bmo_agency_client::{
  ResearchKeySettings, ResearchRun, ResearchRunAsk, ResearchSettings, ResearchSettingsChange,
  SearchEngineState,
};
use policy::research::{KeySettings, RESEARCH, STRATEGIES};
use policy::work_keys::{Profile, WORK_KEYS};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::control::claude_accounts::Account;
use crate::door::apart::{self, Apart, answered, new_session};
use crate::door::record::Reply;
use crate::door::turn::Watch;
use crate::job_run::claude::failure::Failure;
use crate::job_run::claude::walk::{Attempt, Walking, on_accounts};
use crate::lines::claude::{Command, Runs, command_env};
use crate::lines::runner::{Browse, On, PROVIDER, Runner, RunnerRun, Search, runner_run_on};
use crate::lines::worker::{effort_of, reasoning_of};
use crate::lines::{Trees, render};
use crate::places::Ground;
use crate::queue::settings;
use crate::road::{self, Ask, Road};
use crate::run_ledger::{self, Closing, Opening, Spent};
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::{Refusal, now_ms};

/// The settings key the owner's research settings stand under.
pub const SETTINGS_KEY: &str = "research_settings";

// How long past its clock a run may go on writing its result before its pane is cut.
const WRITING_MARGIN: Duration = Duration::from_secs(10 * 60);

/// A research key's profile: a runner's key on the search or browse line.
pub fn research_profile(key: &str) -> Option<&'static Profile> {
  WORK_KEYS.profile(key).filter(|profile| profile.kind == "runner")
}

/// Every research key, in the policy's order.
pub fn research_keys() -> impl Iterator<Item = &'static Profile> {
  WORK_KEYS.profile.iter().filter(|profile| profile.kind == "runner")
}

/// The settings every research run is held to: the owner's where he changed them, the policy's otherwise.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
  pub branch_role: String,
  pub strategy: String,
  pub host_interval_ms: Option<i64>,
  pub engine_interval_ms: Option<i64>,
  pub keys: BTreeMap<String, Kept>,
}

/// One research key's branches and clock, as the settings keep them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
  pub branches: i64,
  pub minutes: i64,
}

impl From<&KeySettings> for Kept {
  fn from(declared: &KeySettings) -> Self {
    Self {
      branches: declared.branches,
      minutes: declared.minutes,
    }
  }
}

/// The policy's settings, a key the policy names no settings for taking none and five minutes.
pub fn defaults() -> Settings {
  let declared = &RESEARCH.settings;

  Settings {
    branch_role: declared.branch_role.clone(),
    strategy: declared.strategy.clone(),
    host_interval_ms: declared.host_interval_ms,
    engine_interval_ms: declared.engine_interval_ms,
    keys: research_keys()
      .map(|profile| {
        let kept = declared.keys.get(&profile.key).map_or(
          Kept {
            branches: 0,
            minutes: 5,
          },
          Kept::from,
        );

        (profile.key.clone(), kept)
      })
      .collect(),
  }
}

/// The settings as they stand, and whether the owner changed them: his where they are kept and read, a key
/// they leave out taking the policy's.
pub async fn settings(db: &sea_orm::DatabaseConnection) -> Result<(Settings, bool), Refusal> {
  let fallback = defaults();
  let Some(held) = settings::get(db, SETTINGS_KEY)
    .await?
    .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
  else {
    return Ok((fallback, false));
  };
  let mut keys = fallback.keys;

  for (key, kept) in held.keys {
    if keys.contains_key(&key) {
      keys.insert(key, kept);
    }
  }

  Ok((Settings { keys, ..held }, true))
}

/// The settings with a change laid over them, or why the change cannot stand: a strategy outside the web tools'
/// three, a gap below nothing, a key that is no research key, a branch count below nothing, a clock of no
/// minutes, or a role no co-worker holds.
pub fn changed(mut settings: Settings, change: &ResearchSettingsChange) -> Result<Settings, String> {
  if let Some(strategy) = &change.strategy {
    if !STRATEGIES.contains(&strategy.as_str()) {
      return Err(format!("{strategy} is no fetch strategy: {}", STRATEGIES.join(", ")));
    }

    settings.strategy.clone_from(strategy);
  }

  if let Some(role) = &change.branch_role {
    let key = format!("role:{role}");

    if !policy::seeds::SEEDS.key.iter().any(|one| one.key == key) {
      return Err(format!("{role} is no co-worker role"));
    }

    settings.branch_role.clone_from(role);
  }

  for (gap, given) in [
    (&mut settings.host_interval_ms, change.host_interval_ms),
    (&mut settings.engine_interval_ms, change.engine_interval_ms),
  ] {
    if let Some(given) = given {
      if given.is_some_and(|ms| ms < 0) {
        return Err("a gap is a number of milliseconds, never below nothing".to_owned());
      }

      *gap = given;
    }
  }

  for key in change.keys.iter().flatten() {
    let Some(kept) = settings.keys.get_mut(&key.key) else {
      return Err(format!("{} is no research key", key.key));
    };

    if key.branches < 0 || key.minutes < 1 {
      return Err(format!(
        "{}: branches are never below nothing, and a clock runs a minute at least",
        key.key
      ));
    }

    *kept = Kept {
      branches: key.branches,
      minutes: key.minutes,
    };
  }

  Ok(settings)
}

/// The settings written as the owner's.
pub async fn keep(db: &sea_orm::DatabaseConnection, settings: &Settings) -> Result<(), Refusal> {
  settings::set(db, SETTINGS_KEY, &serde_json::to_string(settings).unwrap_or_default()).await
}

/// The search engines' state, as the web tools answer it through the tool host's call of
/// `explorer/search_engines`, or why it could not be read.
pub async fn engines(ground: &Ground) -> Result<Vec<SearchEngineState>, String> {
  let read = crate::lines::token::system_call(ground, "explorer", "search_engines", Map::new()).await?;
  let listed = read.get("engines").and_then(Value::as_array).ok_or("the web tools named no engines")?;
  let text = |engine: &Value, name: &str| engine.get(name).and_then(Value::as_str).map(str::to_owned);
  let number = |engine: &Value, name: &str| engine.get(name).and_then(Value::as_i64);

  Ok(
    listed
      .iter()
      .map(|engine| SearchEngineState {
        name: text(engine, "name").unwrap_or_default(),
        category: text(engine, "category"),
        state: text(engine, "state").unwrap_or_else(|| "unknown".to_owned()),
        reason: text(engine, "reason"),
        observed_at: number(engine, "observed_at"),
        suspended_until: number(engine, "suspended_until"),
      })
      .collect(),
  )
}

/// The settings as the routes answer them, with the engines' state where it is to be read.
pub fn view(
  settings: &Settings,
  set: bool,
  engines: Result<Vec<SearchEngineState>, String>,
) -> ResearchSettings {
  let (engines, engines_unread) = match engines {
    Ok(engines) => (Some(engines), None),
    Err(why) => (None, Some(why)),
  };

  ResearchSettings {
    branch_role: settings.branch_role.clone(),
    strategy: settings.strategy.clone(),
    host_interval_ms: settings.host_interval_ms,
    engine_interval_ms: settings.engine_interval_ms,
    keys: research_keys()
      .filter_map(|profile| {
        settings.keys.get(&profile.key).map(|kept| ResearchKeySettings {
          key: profile.key.clone(),
          branches: kept.branches,
          minutes: kept.minutes,
        })
      })
      .collect(),
    source: if set { "set" } else { "default" }.to_owned(),
    engines,
    engines_unread,
  }
}

/// The variables a run of a research key is launched with, which the record the run's tool session is issued
/// carries under `labels.research` (`crate::lines::token`, the research record): when its web tools close, the
/// branches it may open and the role they go out on, its line, who asked and the room, the fetch strategy and
/// the gaps.
pub fn variables(
  settings: &Settings,
  key: &str,
  line: &str,
  asker: Option<&str>,
  room: Option<i64>,
  closes_at: i64,
) -> BTreeMap<String, String> {
  let kept = settings.keys.get(key).copied().unwrap_or(Kept {
    branches: 0,
    minutes: 5,
  });
  let mut set = BTreeMap::from([
    ("BMO_RESEARCH_CLOSES_AT".to_owned(), closes_at.to_string()),
    ("BMO_RESEARCH_BRANCHES".to_owned(), kept.branches.to_string()),
    ("BMO_RESEARCH_ROLE".to_owned(), settings.branch_role.clone()),
    ("BMO_RESEARCH_SOURCE".to_owned(), line.to_owned()),
    ("BMO_WEB_STRATEGY".to_owned(), settings.strategy.clone()),
  ]);

  if kept.branches > 0 {
    set.insert("BMO_RESEARCH_TOOLS".to_owned(), "research_branch".to_owned());
  }

  if let Some(asker) = asker.map(str::trim).filter(|asker| !asker.is_empty()) {
    set.insert("BMO_RESEARCH_OWNER".to_owned(), asker.to_owned());
  }

  if let Some(room) = room {
    set.insert("BMO_RESEARCH_CHAT".to_owned(), room.to_string());
  }

  for (variable, gap) in [
    ("BMO_WEB_HOST_INTERVAL_MS", settings.host_interval_ms),
    ("BMO_WEB_ENGINE_INTERVAL_MS", settings.engine_interval_ms),
  ] {
    if let Some(gap) = gap {
      set.insert(variable.to_owned(), gap.to_string());
    }
  }

  set
}

/// Why a research ask is refused before anything is looked up: a key that is no research key, or no question.
pub fn ask_fault(ask: &ResearchRunAsk) -> Option<String> {
  if research_profile(&ask.key).is_none() {
    let keys: Vec<&str> = research_keys().map(|profile| profile.key.as_str()).collect();

    return Some(format!("{} is no research key: {}", ask.key, keys.join(", ")));
  }

  ask.query.trim().is_empty().then(|| "a research asks a question".to_owned())
}

/// Why a research run cannot be launched now, refused before anything stands: the road resolution's sentence
/// where nothing of the key's walk can carry it.
pub enum Refused {
  Ask(String),
  Closed(String),
  Failed(String),
}

// The depth a search key names, as the request words it; a browse names none.
fn depth_of(key: &str) -> &str {
  key.split_once(':').map_or(key, |(_, depth)| depth)
}

// The words of a ledger depth: basic first.
fn ledger_depth(key: &str) -> u32 {
  match depth_of(key) {
    "thorough" => 1,
    "exhaustive" => 2,
    _ => 0,
  }
}

// The request a run is put to, its line's template filled with the ask.
fn request(trees: &Trees, line: &str, ask: &ResearchRunAsk, kept: Kept) -> Result<String, Refusal> {
  let template = crate::lines::read_required(&trees.agent_dir(line).join("request.md"))?;
  let asked_at = chrono::Utc::now().to_rfc3339();

  Ok(if line == "browse" {
    let synthesis = (kept.minutes / 5).max(1);
    let research = (kept.minutes - synthesis).max(1);

    render(
      &template,
      &[
        ("task", ask.query.trim()),
        ("budget", &format!("{} minutes", kept.minutes)),
        ("research", &format!("{research} minutes")),
        ("synthesis", &format!("{synthesis} minutes")),
      ],
    )
  } else {
    let branches = match kept.branches {
      0 => "no pooled branches".to_owned(),
      1 => "one pooled branch".to_owned(),
      many => format!("up to {many} pooled branches"),
    };

    render(
      &template,
      &[
        ("query", ask.query.trim()),
        (
          "asker",
          ask
            .asker
            .as_deref()
            .map(str::trim)
            .filter(|asker| !asker.is_empty())
            .unwrap_or("the owner"),
        ),
        ("askedAt", &asked_at),
        (
          "options",
          ask.options.as_deref().map(str::trim).filter(|said| !said.is_empty()).unwrap_or("none"),
        ),
        ("depth", depth_of(&ask.key)),
        ("branches", &branches),
      ],
    )
  })
}

/// One research run launched (docs/research-runs.md, 「One run, step by step」): its road resolved, its ledger
/// run opened, and the run itself sent on in the background; what it was launched on answered at once.
pub async fn launch(ground: Arc<Ground>, ask: ResearchRunAsk) -> Result<ResearchRun, Refused> {
  if let Some(why) = ask_fault(&ask) {
    return Err(Refused::Ask(why));
  }

  let profile = research_profile(&ask.key).expect("a research key has its profile");
  let line = profile.line.clone();
  let resolution = road::resolve(&ground, &Ask::key(&ask.key)).await.map_err(Refused::Failed)?;
  let Some(taken) = resolution.road().cloned() else {
    road::went(&ground, &resolution, None, &[]).await;

    return Err(Refused::Closed(resolution.sentence()));
  };

  road::went(&ground, &resolution, Some(&taken), &[]).await;

  let (settings, _) = settings(&ground.db).await.map_err(|why| Refused::Failed(why.detail))?;
  let kept = settings.keys.get(&ask.key).copied().unwrap_or(Kept {
    branches: 0,
    minutes: 5,
  });
  let closes_at = now_ms() + kept.minutes * 60_000;
  let trees = Trees::of(&ground);
  let prompt = request(&trees, &line, &ask, kept).map_err(|why| Refused::Failed(why.detail))?;
  let run = composed(&ground, &trees, &line, &ask.key, &taken).await?;
  let vars = variables(&settings, &ask.key, &line, ask.asker.as_deref(), ask.room_id, closes_at);
  let unit = format!("research-{}", now_ms());
  let ledger = run_ledger::open(
    &ground.db,
    &Opening {
      agent: line.clone(),
      chat_id: ask.room_id,
      unit: unit.clone(),
      depth: ledger_depth(&ask.key),
      cap_ms: Some(kept.minutes * 60_000),
      started_at: now_ms().div_euclid(1000),
      started_by: ask.asker.clone(),
      search_id: None,
      round: None,
      message_id: None,
    },
  )
  .await
  .map_err(|why| Refused::Failed(why.detail))?;
  let launched = ResearchRun {
    run_id: ledger,
    key: ask.key.clone(),
    provider: taken.provider.clone(),
    model: taken.model.clone(),
    account: taken.account.clone(),
    closes_at,
  };
  let going = Going {
    ground: ground.clone(),
    ask,
    line,
    unit,
    ledger,
    run,
    vars,
    prompt,
    bound: Duration::from_secs(u64::try_from(kept.minutes * 60).unwrap_or(300)) + WRITING_MARGIN,
  };

  tokio::spawn(async move { going.go().await });

  Ok(launched)
}

// The runner's line composed on the road taken: its instructions, its configuration, the model the road names
// and the effort the key's tier maps there, laid under the runner's own thinking and effort.
async fn composed(
  ground: &Ground,
  trees: &Trees,
  line: &str,
  key: &str,
  taken: &Road,
) -> Result<RunnerRun, Refused> {
  let failed = |why: Refusal| Refused::Failed(why.detail);
  let mapping = crate::control::tiers::resolve_execution(&ground.db, key, &taken.provider, None)
    .await
    .map_err(Refused::Failed)?;
  let reasoning = reasoning_of(ground, line).await.map_err(failed)?;
  let effort = effort_of(
    ground,
    &ground.machine(),
    PROVIDER,
    &taken.model,
    mapping.effort.as_deref(),
    Some(&reasoning),
  )
  .await
  .map_err(failed)?
  .unwrap_or_default();
  let on = On {
    model: taken.model.clone(),
    effort,
    tier: mapping.tier,
    thinking: reasoning.thinking.unwrap_or(true),
  };
  let unread = || {
    Refused::Failed(format!(
      "{line}: the web namespace's listing cannot be had, so the line is not composed"
    ))
  };

  if line == "browse" {
    let instructions = Browse.instructions(ground, trees).await.map_err(failed)?.ok_or_else(unread)?;

    runner_run_on(ground, trees, &Browse, &instructions, on).await.map_err(failed)
  } else {
    let instructions = Search.instructions(ground, trees).await.map_err(failed)?.ok_or_else(unread)?;

    runner_run_on(ground, trees, &Search, &instructions, on).await.map_err(failed)
  }
}

// A run under way: everything it was launched with.
struct Going {
  ground: Arc<Ground>,
  ask: ResearchRunAsk,
  line: String,
  unit: String,
  ledger: i64,
  run: RunnerRun,
  vars: BTreeMap<String, String>,
  prompt: String,
  bound: Duration,
}

// A run's pane watched for nothing but its end.
struct Quiet;

impl Watch for Quiet {}

// One turn of the run on one login: the line's command on a session of its own, or the same session resumed
// for a round, in a pane of its own.
struct OnLogin<'a> {
  going: &'a Going,
  session: String,
  resume: bool,
  prompt: String,
  account: Option<Account>,
}

impl Attempt<Reply> for OnLogin<'_> {
  async fn attempt(&mut self, account: Option<Account>) -> Result<Reply, Failure> {
    let going = self.going;
    let ground = &going.ground;
    let mut args = going.run.args(&self.session).map_err(|why| Failure::error(why.detail))?;

    if self.resume
      && let Some(at) = args.iter().position(|arg| arg == "--session-id")
    {
      args[at] = "--resume".to_owned();
    }

    let mut env = command_env(ground.env(), account.as_ref(), &Runs::Here, going.run.thinking)
      .map_err(|why| Failure::error(why.detail))?;

    for (name, value) in &going.vars {
      env.insert(name.clone(), Some(value.clone()));
    }

    let command = Command {
      args,
      cwd: going.run.cwd.to_string_lossy().into_owned(),
      env,
    };
    let ending = apart::run(
      ground,
      Apart {
        agent: &going.line,
        line: going.run.line,
        provider: PROVIDER,
        title: format!("{} · {}", going.ask.key, going.run.model),
        command,
        session: self.session.clone(),
        prompt: self.prompt.clone(),
        work_key: going.ask.key.clone(),
        model: going.run.model.clone(),
        effort: going.run.effort.clone(),
        tier: Some(going.run.tier.clone()),
        bound: Some(going.bound),
        parent: None,
        root: None,
      },
      &mut Quiet,
    )
    .await
    .map_err(|why| Failure::Transport { message: why })?;
    let reply = answered(ending, &going.ask.key)?;

    self.account = account;

    Ok(reply)
  }

  // A run on an endpoint account goes out like any other: the research line asks nothing of a login but its
  // answer.
  fn takes(&self, _account: &Account) -> bool {
    true
  }
}

fn spent_of(reply: &Reply) -> Spent {
  reply.spend.iter().fold(
    Spent {
      input_tokens: 0,
      output_tokens: 0,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
    },
    |sum, one| Spent {
      input_tokens: sum.input_tokens + one.input_tokens,
      output_tokens: sum.output_tokens + one.output_tokens,
      cache_read_tokens: sum.cache_read_tokens + one.cache_read_tokens,
      cache_write_tokens: sum.cache_write_tokens + one.cache_write_tokens,
    },
  )
}

fn sum(spent: Option<Spent>, more: Spent) -> Option<Spent> {
  Some(match spent {
    None => more,
    Some(spent) => Spent {
      input_tokens: spent.input_tokens + more.input_tokens,
      output_tokens: spent.output_tokens + more.output_tokens,
      cache_read_tokens: spent.cache_read_tokens + more.cache_read_tokens,
      cache_write_tokens: spent.cache_write_tokens + more.cache_write_tokens,
    },
  })
}

impl Going {
  async fn note(&self, log: &mut RunLog, text: String) {
    log.write(&self.ground.db, Line::note(text)).await;
  }

  // The run to its end: its first answer on the logins its key's walk offers, then each of the line's rounds
  // on the same session and login, then its ledger run closed and its result queued for the room.
  async fn go(self) {
    let ground = self.ground.clone();
    let mut log = RunLog::new(Target {
      agent: self.line.clone(),
      run: self.unit.clone(),
      chat_id: self.ask.room_id,
      job_id: None,
    });

    self
      .note(
        &mut log,
        format!("Research on {} · {}: {}", self.ask.key, self.run.model, self.ask.query.trim()),
      )
      .await;

    let session = new_session();
    let mut first = OnLogin {
      going: &self,
      session: session.clone(),
      resume: false,
      prompt: self.prompt.clone(),
      account: None,
    };
    let answered = on_accounts(&Walking { ground: &ground }, &self.ask.key, None, &mut first).await;
    let account = first.account.clone();
    let (outcome, result, spent) = match answered {
      Err(failure) => ("failed", Err(failure.message()), None),
      Ok(reply) => {
        let mut spent = sum(None, spent_of(&reply));
        let mut answers = vec![reply.text.clone()];

        self.rounds(&session, account, &mut answers, &mut spent, &mut log).await;
        ("answered", Ok(answers.pop().unwrap_or_default()), spent)
      }
    };

    let closing = Closing {
      finished_at: now_ms().div_euclid(1000),
      outcome: outcome.to_owned(),
      context_tokens: None,
      spent,
    };

    if let Err(run_ledger::NotClosed::Store(why)) =
      run_ledger::close(&ground.db, self.ledger, &closing).await
    {
      eprintln!("research {}: its run was not closed in the ledger: {}", self.ledger, why.detail);
    }

    match &result {
      Ok(_) => self.note(&mut log, "The research answered.".to_owned()).await,
      Err(why) => self.note(&mut log, format!("The research ended with no answer: {why}")).await,
    }

    if let Err(why) = crate::job_end::instruct(ground.vars(), &self.instruction(&result)).await {
      eprintln!("research {}: its result was not told to the room: {why}", self.ledger);
    }
  }

  // The line's rounds after the first answer, each on the same session and login, past the line's ceiling the
  // final one asked at once or the run returning what it has, as the policy says; a round that does not answer
  // leaves the last answer standing.
  async fn rounds(
    &self,
    session: &str,
    account: Option<Account>,
    answers: &mut Vec<String>,
    spent: &mut Option<Spent>,
    log: &mut RunLog,
  ) {
    let db = &self.ground.db;
    let Ok(Some(process)) = crate::research_draft::process_of(db, &self.line).await else {
      return;
    };
    let began = std::time::Instant::now();
    let mut asking = 0;

    while asking < process.rounds.len() {
      let name = process.rounds[asking].clone();
      let past = began.elapsed().as_secs() as i64 >= process.ceiling_seconds;

      if past && name.as_str() != policy::research::Draft::FINAL {
        match process.past_ceiling {
          policy::research::PastCeiling::Last => return,
          policy::research::PastCeiling::Final => {
            match process.rounds.iter().position(|one| one.as_str() == policy::research::Draft::FINAL) {
              Some(at) if at >= asking => {
                asking = at;
                continue;
              }
              _ => return,
            }
          }
        }
      }

      let Ok(Some(question)) = crate::research_draft::question_of(db, &name).await else {
        asking += 1;
        continue;
      };
      let mut round = OnLogin {
        going: self,
        session: session.to_owned(),
        resume: true,
        prompt: crate::research_draft::prompt(&question, &self.prompt, answers),
        account: None,
      };

      match round.attempt(account.clone()).await {
        Ok(reply) => {
          *spent = sum(spent.take(), spent_of(&reply));
          answers.push(reply.text);
        }
        Err(failure) => {
          self.note(log, format!("The round {name} did not answer: {}", failure.message())).await;
          return;
        }
      }

      if name.as_str() == policy::research::Draft::FINAL {
        return;
      }

      asking += 1;
    }
  }

  // The instruction the room's chat agent is queued with as the run ends: its result whole, or why there is
  // none.
  fn instruction(&self, result: &Result<String, String>) -> Value {
    let id = self.ledger;
    let said = match result {
      Ok(text) => format!(
        "The research on {} ({}) answered: \"{}\".\n\nIts result, as the run wrote it:\n\n{text}\n\nTell the \
         owner this result in his language, whole, keeping every figure, source and address as it stands.",
        self.ask.key,
        self.run.model,
        self.ask.query.trim()
      ),
      Err(why) => format!(
        "The research on {} (\"{}\") ended with no answer: {why}\n\nTell the owner what it could not do, \
         and what was tried.",
        self.ask.key,
        self.ask.query.trim()
      ),
    };

    json!({
      "room": self.ask.room_id.filter(|room| *room != 0),
      "said": { "order": said },
      "sort": "notice",
      "kind": format!("research-ended:{id}"),
      "summary": format!("The research {id} ended"),
      "events": [format!("research-ended:{id}")],
      "run": id,
    })
  }
}
