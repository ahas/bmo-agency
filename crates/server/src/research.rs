//! Research runs (docs/research-runs.md): a search at a depth or a browse, asked of agency on a research key,
//! sent out on the road the one road resolution answers for that key, as the runner's own line run apart in a
//! pane of its own, held to the research settings the owner keeps here, asked the line's research rounds
//! after its first answer, and its result queued for the room as it ends.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
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

use crate::companion::run::NoAnswer;
use crate::control::claude_accounts::Account;
use crate::door::apart::{self, Apart, answered, new_session};
use crate::door::place_run::end_run;
use crate::door::record::Reply;
use crate::door::turn::Watch;
use crate::job_run::claude::failure::Failure;
use crate::job_run::claude::walk::{Attempt, Walking, on_accounts};
use crate::lines::claude::{Command, Runs, command_env};
use crate::lines::runner::{Browse, On, PROVIDER, Runner, RunnerRun, Search, runner_run_on};
use crate::lines::worker::{effort_of, reasoning_of};
use crate::lines::{Trees, render};
use crate::places::Ground;
use crate::places::locks::{agent_lock_path, hold};
use crate::queue::settings;
use crate::road::{self, Ask, Named, Road};
use crate::run_ledger::{self, Closing, Opening, Spent};
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::turn_pickup::{Came, Standing, close_rows, own_rows, standing_after};
use crate::{Refusal, now_ms};

/// The settings key the owner's research settings stand under.
pub const SETTINGS_KEY: &str = "research_settings";

// How long past its clock a run may go on writing its result before its pane is cut.
const WRITING_MARGIN: Duration = Duration::from_secs(10 * 60);

/// A research key's profile: a runner's key the research settings hold, on the search or browse line; the
/// translator is a runner too and no research.
pub fn research_profile(key: &str) -> Option<&'static Profile> {
  WORK_KEYS.profile(key).filter(|profile| is_research(profile))
}

/// Every research key, in the policy's order.
pub fn research_keys() -> impl Iterator<Item = &'static Profile> {
  WORK_KEYS.profile.iter().filter(|profile| is_research(profile))
}

fn is_research(profile: &Profile) -> bool {
  profile.kind == "runner" && RESEARCH.settings.keys.contains_key(&profile.key)
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

/// One research key's branches, clock and budget, as the settings keep them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
  pub branches: i64,
  pub minutes: i64,
  /// The most pages the first answer fetches; none where no ceiling holds, or where a kept row names none
  /// and the policy's stands.
  #[serde(default)]
  pub pages: Option<i64>,
  /// The most pages each branch fetches, read the same way.
  #[serde(default)]
  pub branch_pages: Option<i64>,
}

impl From<&KeySettings> for Kept {
  fn from(declared: &KeySettings) -> Self {
    Self {
      branches: declared.branches,
      minutes: declared.minutes,
      pages: declared.pages,
      branch_pages: declared.branch_pages,
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
            pages: None,
            branch_pages: None,
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
    if let Some(policy) = keys.get(&key).copied() {
      // A kept row naming no budget takes the policy's.
      keys.insert(
        key,
        Kept {
          pages: kept.pages.or(policy.pages),
          branch_pages: kept.branch_pages.or(policy.branch_pages),
          ..kept
        },
      );
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

    if key.pages.is_some_and(|pages| pages < 1) || key.branch_pages.is_some_and(|pages| pages < 1) {
      return Err(format!("{}: a budget reads one page at least", key.key));
    }

    *kept = Kept {
      branches: key.branches,
      minutes: key.minutes,
      pages: key.pages.or(kept.pages),
      branch_pages: key.branch_pages.or(kept.branch_pages),
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
          pages: kept.pages,
          branch_pages: kept.branch_pages,
        })
      })
      .collect(),
    source: if set { "set" } else { "default" }.to_owned(),
    engines,
    engines_unread,
  }
}

/// The variables a run of a research key is launched with, which the record the run's tool session is issued
/// carries under `labels.research` (`crate::lines::token`, the research record): its ledger run, where the
/// session service its research tool opens a branch through answers and the token it takes, as the
/// environment agency runs in names them, when its web tools close, the branches it may open and the role
/// they go out on, its line, who asked and the room, the fetch strategy and the gaps.
#[allow(clippy::too_many_arguments)]
pub fn variables(
  env: &BTreeMap<String, String>,
  settings: &Settings,
  key: &str,
  line: &str,
  run: i64,
  asker: Option<&str>,
  room: Option<i64>,
  closes_at: i64,
) -> BTreeMap<String, String> {
  let kept = settings.keys.get(key).copied().unwrap_or(Kept {
    branches: 0,
    minutes: 5,
    pages: None,
    branch_pages: None,
  });
  let mut set = BTreeMap::from([
    (crate::lines::token::RUN.to_owned(), run.to_string()),
    ("BMO_RESEARCH_CLOSES_AT".to_owned(), closes_at.to_string()),
    ("BMO_RESEARCH_BRANCHES".to_owned(), kept.branches.to_string()),
    ("BMO_RESEARCH_ROLE".to_owned(), settings.branch_role.clone()),
    ("BMO_RESEARCH_SOURCE".to_owned(), line.to_owned()),
    ("BMO_WEB_STRATEGY".to_owned(), settings.strategy.clone()),
  ]);

  if kept.branches > 0 {
    set.insert("BMO_RESEARCH_TOOLS".to_owned(), "research".to_owned());
  }

  for (variable, named) in [
    (crate::lines::token::SESSIONS_AT, "BMO_AGENCY_URL"),
    (crate::lines::token::SESSIONS_TOKEN, crate::verbs::CONTROL_TOKEN),
  ] {
    if let Some(value) = env.get(named).map(|value| value.trim()).filter(|value| !value.is_empty()) {
      set.insert(variable.to_owned(), value.to_owned());
    }
  }

  for (variable, pages) in
    [("BMO_RESEARCH_PAGES", kept.pages), ("BMO_RESEARCH_BRANCH_PAGES", kept.branch_pages)]
  {
    if let Some(pages) = pages {
      set.insert(variable.to_owned(), pages.to_string());
    }
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

/// Why a research ask is refused before anything is looked up: a key that is no research key, no question, or a
/// provider agency does not know.
pub fn ask_fault(ask: &ResearchRunAsk) -> Option<String> {
  if research_profile(&ask.key).is_none() {
    let keys: Vec<&str> = research_keys().map(|profile| profile.key.as_str()).collect();

    return Some(format!("{} is no research key: {}", ask.key, keys.join(", ")));
  }

  if let Some(provider) = named_of(ask).and_then(|named| named.provider)
    && harness_providers::provider(provider).is_none()
  {
    let known: Vec<&str> = harness_providers::providers().iter().map(|one| one.name()).collect();

    return Some(format!("{provider} is no provider: {}", known.join(", ")));
  }

  ask.query.trim().is_empty().then(|| "a research asks a question".to_owned())
}

// A value the ask says, none where it is left out or blank.
fn said(value: &Option<String>) -> Option<&str> {
  value.as_deref().map(str::trim).filter(|said| !said.is_empty())
}

/// The road a research ask names (docs/research-runs.md, 「The road」): its provider, its model or both, none
/// where it names neither and the road resolution answers for its key.
pub fn named_of(ask: &ResearchRunAsk) -> Option<Named<'_>> {
  let named = Named {
    provider: said(&ask.provider),
    model: said(&ask.model),
  };

  (named != Named::default()).then_some(named)
}

/// Why a research run cannot be launched now, refused before anything stands: the road resolution's sentence
/// where nothing of the key's walk can carry it.
pub enum Refused {
  Ask(String),
  Closed(String),
  Failed(String),
}

/// The settings row a run under way is kept in, by its ledger run (docs/research-runs.md, 「One run, step
/// by step」).
pub const IN_FLIGHT_KEY: &str = "research_in_flight:";

/// The outcome a run is closed with when its pane went away under it or agency's own restart ended it.
pub const CUT: &str = "cut";

/// A run under way as its settings row keeps it: what the room is told of it if it is cut, the record its
/// session writes, where what it had written is read, and for a run in places what a server started after
/// the one that sent it takes it up again by.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InFlight {
  pub key: String,
  pub query: String,
  #[serde(default)]
  pub room: Option<i64>,
  #[serde(default)]
  pub model: String,
  #[serde(default)]
  pub record: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub placed: Option<Placed>,
}

/// A run in places as it is kept before each of its turns goes in (docs/research-runs.md, 「One run, step by
/// step」, step 7): everything it went out with, and where it stands.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Placed {
  pub ask: ResearchRunAsk,
  pub taken: Road,
  /// The run its log and its turns go under.
  pub unit: String,
  pub prompt: String,
  pub closes_at: i64,
  pub bound_ms: u64,
  pub run: KeptRun,
  /// The place the turn under way goes into, and how many of the run's turns went into it before it.
  pub place: String,
  pub before: usize,
  /// The round the turn asks, by its place among the key's rounds; none for the first answer.
  #[serde(default)]
  pub round: Option<usize>,
  /// When the rounds began, in epoch milliseconds.
  #[serde(default)]
  pub rounds_began: Option<i64>,
  /// What the run's turns answered so far, the first answer first, and what they spent.
  #[serde(default)]
  pub answers: Vec<String>,
  #[serde(default)]
  pub spent: Option<Spent>,
}

/// A run's composed line as it is kept: all of it but what its runner names of itself.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeptRun {
  pub model: String,
  pub effort: String,
  pub tier: String,
  pub system_prompt: PathBuf,
  pub mcp_config: PathBuf,
  pub thinking: bool,
  pub cwd: PathBuf,
}

impl KeptRun {
  fn of(run: &RunnerRun) -> Self {
    Self {
      model: run.model.clone(),
      effort: run.effort.clone(),
      tier: run.tier.clone(),
      system_prompt: run.system_prompt.clone(),
      mcp_config: run.mcp_config.clone(),
      thinking: run.thinking,
      cwd: run.cwd.clone(),
    }
  }

  // The line again, its runner the one the run's line names.
  fn run(&self, line: &str) -> RunnerRun {
    let (name, tools_allowed) = if line == Browse.name() {
      (Browse.name(), Browse.tools_allowed())
    } else {
      (Search.name(), Search.tools_allowed())
    };

    RunnerRun {
      line: name,
      model: self.model.clone(),
      effort: self.effort.clone(),
      tier: self.tier.clone(),
      system_prompt: self.system_prompt.clone(),
      mcp_config: self.mcp_config.clone(),
      tools_allowed,
      thinking: self.thinking,
      cwd: self.cwd.clone(),
    }
  }
}

async fn keep_in_flight(db: &sea_orm::DatabaseConnection, ledger: i64, kept: &InFlight) {
  let text = serde_json::to_string(kept).unwrap_or_default();

  if let Err(why) = settings::set(db, &format!("{IN_FLIGHT_KEY}{ledger}"), &text).await {
    eprintln!("research {ledger}: the run under way was not kept: {}", why.detail);
  }
}

async fn in_flight(db: &sea_orm::DatabaseConnection, ledger: i64) -> Option<InFlight> {
  let held = settings::get(db, &format!("{IN_FLIGHT_KEY}{ledger}")).await.ok().flatten()?;

  serde_json::from_str(&held).ok()
}

/// What a run's session had written by the time it was cut: the last answer its record holds, none where it
/// holds none.
pub fn written_so_far(record: &std::path::Path) -> Option<String> {
  let records = crate::door::record::Tail::new(record).look();
  let text = crate::door::record::read_turn(&records, 0).text;

  Some(text.trim().to_owned()).filter(|text| !text.is_empty())
}

/// The instruction the room is queued with for a cut run: what it had written, handed back as unfinished,
/// or one line saying the work was cut off.
pub fn cut_instruction(
  ledger: i64,
  key: &str,
  query: Option<&str>,
  room: Option<i64>,
  written: Option<&str>,
) -> Value {
  let asked = query
    .map(str::trim)
    .filter(|query| !query.is_empty())
    .map(|query| format!(" (\"{query}\")"));
  let said = match written {
    Some(text) => format!(
      "The research on {key}{} was cut off before it finished. What it had written:\n\n{text}\n\nHand the \
       owner what it had written, in his language, saying it is unfinished.",
      asked.unwrap_or_default()
    ),
    None => format!(
      "The research on {key}{} was cut off before it wrote anything.\n\nTell the owner in one line that the \
       work was cut off.",
      asked.unwrap_or_default()
    ),
  };

  json!({
    "room": room.filter(|room| *room != 0),
    "said": { "order": said },
    "sort": "notice",
    "kind": format!("research-ended:{ledger}"),
    "summary": format!("The research {ledger} was cut"),
    "events": [format!("research-ended:{ledger}")],
    "run": ledger,
  })
}

/// The structure lines a result's blocks open with, which end the block before them.
const BLOCK_OPENERS: [&str; 6] =
  ["VERDICT:", "SUMMARY:", "SECTION:", "UNCONFIRMED:", "===ITEM===", "SOURCE "];

// The value of a head line, such as `TITLE: <it>`, where the result writes one.
fn head_line<'a>(result: &'a str, name: &str) -> Option<&'a str> {
  result
    .lines()
    .find_map(|line| line.trim().strip_prefix(name).map(str::trim))
    .filter(|value| !value.is_empty())
}

// The block a structure line opens, to the next one, where the result writes it.
fn block(result: &str, opener: &str) -> Option<String> {
  let mut lines = result.lines().skip_while(|line| line.trim() != opener);

  lines.next()?;

  let text: Vec<&str> = lines
    .take_while(|line| !BLOCK_OPENERS.iter().any(|next| line.trim_start().starts_with(next)))
    .collect();
  let text = text.join("\n").trim().to_owned();

  Some(text).filter(|text| !text.is_empty())
}

/// The answer in brief a result gives the room: its verdict, else its summary, else its title.
pub fn brief_of(result: &str) -> Option<String> {
  block(result, "VERDICT:")
    .or_else(|| block(result, "SUMMARY:"))
    .or_else(|| head_line(result, "TITLE:").map(str::to_owned))
}

/// How long a research document's share link opens.
pub const SHARE_LIFE: Duration = Duration::from_secs(24 * 3_600);

/// Who a research document is filed by.
pub const RESEARCH_ACTOR: &str = "research";

/// A result's document filed in bmo-artifact — `filing` the document as its routes take it, `drawn` the
/// pictures it keeps — and given its share link, answering the link; none where the deployment names no
/// artifact service.
pub async fn file_result(
  vars: &BTreeMap<String, String>,
  filing: &Value,
  drawn: &crate::research_document::Drawn,
) -> Result<Option<String>, String> {
  let (Some(url), Some(token)) = (
    crate::job_end::address(vars, "BMO_ARTIFACT_URL"),
    crate::job_end::token(vars, "BMO_ARTIFACT_TOKEN", "BMO_ARTIFACT_TOKEN_FILE"),
  ) else {
    return Ok(None);
  };
  let artifact = crate::research_document::Artifact::at(url, token);
  let filed = artifact.send(reqwest::Method::POST, "/documents", filing).await?;
  let id = filed["id"].as_i64().ok_or("bmo-artifact named no document for the result")?;
  let (body, _) = crate::research_document::keep_pictures(&artifact, id, drawn).await;

  if body != drawn.body {
    artifact
      .send(
        reqwest::Method::PATCH,
        &format!("/documents/{id}"),
        &json!({ "body": body, "labels": filing["labels"] }),
      )
      .await?;
  }

  // The link ends a day after it is made; the document stays in the archive.
  let ends = now_ms() + SHARE_LIFE.as_millis() as i64;
  let shared = artifact
    .send(reqwest::Method::POST, "/shares", &json!({ "documentId": id, "expiresAt": ends }))
    .await?;

  shared["url"]
    .as_str()
    .map(|link| Some(link.to_owned()))
    .ok_or_else(|| "bmo-artifact named no link".to_owned())
}

/// The instruction the room's chat agent is queued with for an answered run whose document was filed:
/// the answer in brief and the one link, with the shape it answers in.
pub fn linked_instruction(
  ledger: i64,
  key: &str,
  query: &str,
  room: Option<i64>,
  brief: &str,
  link: &str,
) -> Value {
  let said = format!(
    "The research on {key} answered \"{}\". The answer in brief:\n\n{brief}\n\nIts document, with the \
     grounds and the sources: {link}\n\nTell the owner the answer in one or two short lines, in his \
     language and your own voice, saying what was found and never how, then this one link on a line of its \
     own. Put no grounds, sources or document text in the chat: they stand in the document.",
    query.trim()
  );

  json!({
    "room": room.filter(|room| *room != 0),
    "said": { "order": said },
    "sort": "notice",
    "kind": format!("research-ended:{ledger}"),
    "summary": format!("The research {ledger} ended"),
    "events": [format!("research-ended:{ledger}")],
    "run": ledger,
  })
}

// The research key a ledger row's line and depth name.
fn key_of(line: &str, depth: i64) -> String {
  match (line, depth) {
    ("search", 1) => "search:thorough".to_owned(),
    ("search", 2) => "search:exhaustive".to_owned(),
    ("search", _) => "search:basic".to_owned(),
    (line, _) => line.to_owned(),
  }
}

/// What a server's start did with the runs the process before it left: how many it closed as cut, and how
/// many it took up again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Swept {
  pub cut: usize,
  pub taken_up: usize,
}

/// A server's start (docs/research-runs.md, 「One run, step by step」, step 7): every run in places the process
/// before it kept under way whose place still stands taken up again, and every other kept run, and every
/// research run the ledger still holds open from before `started_at` (epoch milliseconds) but those, closed as
/// cut and told to its room.
pub async fn sweep_cut(ground: &Arc<Ground>, started_at: i64) -> Result<Swept, Refusal> {
  let db = &ground.db;
  let vars = ground.vars();
  let mut closed = 0;
  let mut taken_up = BTreeSet::new();
  let closing = Closing {
    finished_at: now_ms(),
    outcome: CUT.to_owned(),
    context_tokens: None,
    spent: None,
  };
  let tell = async |ledger: i64, body: Value| {
    match run_ledger::close(db, ledger, &closing).await {
      Ok(()) => {}
      Err(run_ledger::NotClosed::Store(why)) => return Err(why),
      Err(_) => return Ok(false),
    }

    if let Err(why) = crate::job_end::instruct(vars, &body).await {
      eprintln!("research {ledger}: its cut was not told to the room: {why}");
    }

    Ok(true)
  };

  for name in settings::named(db, IN_FLIGHT_KEY).await? {
    let Ok(ledger) = name.parse::<i64>() else { continue };
    let kept = in_flight(db, ledger).await.unwrap_or_default();

    if let Some(placed) =
      kept.placed.as_ref().filter(|placed| is_run_place(&ground.roots.data, &placed.place))
      && let Some(going) = going_of(ground, ledger, placed).await
    {
      let placed = placed.clone();

      taken_up.insert(ledger);
      tokio::spawn(async move { going.go(Some(placed)).await });
      continue;
    }

    let written = kept.record.as_deref().and_then(|record| written_so_far(std::path::Path::new(record)));
    let body = cut_instruction(ledger, &kept.key, Some(&kept.query), kept.room, written.as_deref());

    if tell(ledger, body).await? {
      closed += 1;
    }

    // A cut run's places are ended with it, where any of them still stands.
    if let Some(placed) = &kept.placed {
      let first = placed.place.trim_end_matches(ROUNDS_PLACE).to_owned();

      for place in [format!("{first}{ROUNDS_PLACE}"), first] {
        if is_run_place(&ground.roots.data, &place) {
          end_place(ground, &place).await;
        }
      }
    }

    settings::forget(db, &format!("{IN_FLIGHT_KEY}{name}")).await?;
  }

  let open = store::all(
    db,
    r#"SELECT "id", "agent", "depth", "chat_id" FROM "runs"
       WHERE "unit" LIKE 'research-%' AND "finished_at" IS NULL AND "started_at" < $1"#,
    vec![started_at.into()],
  )
  .await?;

  for row in open {
    let (Ok(ledger), Ok(line)) = (row.try_get::<i64>("", "id"), row.try_get::<String>("", "agent")) else {
      continue;
    };

    if taken_up.contains(&ledger) {
      continue;
    }

    let depth = row.try_get::<i64>("", "depth").unwrap_or_default();
    let room = row.try_get::<Option<i64>>("", "chat_id").ok().flatten();

    if tell(ledger, cut_instruction(ledger, &key_of(&line, depth), None, room, None)).await? {
      closed += 1;
    }
  }

  Ok(Swept {
    cut: closed,
    taken_up: taken_up.len(),
  })
}

// A kept run in places as it went out, on the road it went out on, for a server that takes it up again: none
// where its key is no research key any more or the research settings cannot be read.
async fn going_of(ground: &Arc<Ground>, ledger: i64, placed: &Placed) -> Option<Going> {
  let line = research_profile(&placed.ask.key)?.line.clone();
  let (settings, _) = settings(&ground.db).await.ok()?;
  let vars = variables(
    ground.env(),
    &settings,
    &placed.ask.key,
    &line,
    ledger,
    placed.ask.asker.as_deref(),
    placed.ask.room_id,
    placed.closes_at,
  );

  Some(Going {
    ground: ground.clone(),
    taken: placed.taken.clone(),
    ask: placed.ask.clone(),
    run: placed.run.run(&line),
    line,
    unit: placed.unit.clone(),
    ledger,
    vars,
    prompt: placed.prompt.clone(),
    closes_at: placed.closes_at,
    bound: Duration::from_millis(placed.bound_ms),
  })
}

/// The variables a run's rounds go out with: its own, its branches, images and rulebooks closed, so a round
/// that checks a source holds the reading tools alone.
pub fn rounds_variables(vars: &BTreeMap<String, String>) -> BTreeMap<String, String> {
  let mut rounds = vars.clone();

  for closed in ["BMO_RESEARCH_TOOLS", "BMO_RESEARCH_RULEBOOKS"] {
    rounds.remove(closed);
  }

  rounds.insert("BMO_RESEARCH_BRANCHES".to_owned(), "0".to_owned());
  rounds
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

/// The budget a run is told as its ceiling: the pages its first answer may fetch and each branch's, or nothing
/// where the key names none.
pub fn budget_line(kept: Kept) -> String {
  let Some(pages) = kept.pages else { return String::new() };
  let branches = match (kept.branches, kept.branch_pages) {
    (0, _) => String::new(),
    (count, Some(each)) => format!(", and each of the {count} branches it may open at most {each}"),
    (count, None) => format!(", beside the {count} branches it may open"),
  };

  format!(
    "Its budget, a hard ceiling: the first answer fetches at most {pages} pages{branches}. Once it is spent, \
     fetch nothing more and write with what you have; what stays open goes under UNCONFIRMED."
  )
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
    let budget = budget_line(kept);

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
        ("budget", &budget),
      ],
    )
  })
}

/// One research run launched (docs/research-runs.md, 「One run, step by step」): its road resolved, on the road
/// the ask names where it names one, its ledger run opened, and the run itself sent on in the background; what
/// it was launched on answered at once.
pub async fn launch(ground: Arc<Ground>, ask: ResearchRunAsk) -> Result<ResearchRun, Refused> {
  if let Some(why) = ask_fault(&ask) {
    return Err(Refused::Ask(why));
  }

  let profile = research_profile(&ask.key).expect("a research key has its profile");
  let line = profile.line.clone();
  let named = named_of(&ask);
  let asked = Ask {
    named,
    ..Ask::key(&ask.key)
  };
  let resolution = road::resolve(&ground, &asked).await.map_err(Refused::Failed)?;
  // A run on a named road is no move of its key's road, and tells none.
  let told = named.is_none();
  let Some(taken) = resolution.road().cloned() else {
    if told {
      road::went(&ground, &resolution, None, &[]).await;
    }

    return Err(Refused::Closed(resolution.sentence()));
  };

  if told {
    road::went(&ground, &resolution, Some(&taken), &[]).await;
  }

  let (settings, _) = settings(&ground.db).await.map_err(|why| Refused::Failed(why.detail))?;
  let kept = settings.keys.get(&ask.key).copied().unwrap_or(Kept {
    branches: 0,
    minutes: 5,
    pages: None,
    branch_pages: None,
  });
  let closes_at = now_ms() + kept.minutes * 60_000;
  let trees = Trees::of(&ground);
  let prompt = request(&trees, &line, &ask, kept).map_err(|why| Refused::Failed(why.detail))?;
  let run = composed(&ground, &trees, &line, &ask.key, &taken).await?;
  let unit = format!("research-{}", now_ms());
  let ledger = run_ledger::open(
    &ground.db,
    &Opening {
      agent: line.clone(),
      chat_id: ask.room_id,
      unit: unit.clone(),
      depth: ledger_depth(&ask.key),
      cap_ms: Some(kept.minutes * 60_000),
      started_at: now_ms(),
      started_by: ask.asker.clone(),
      search_id: None,
      round: None,
      message_id: None,
    },
  )
  .await
  .map_err(|why| Refused::Failed(why.detail))?;
  let vars = variables(
    ground.env(),
    &settings,
    &ask.key,
    &line,
    ledger,
    ask.asker.as_deref(),
    ask.room_id,
    closes_at,
  );
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
    taken,
    ask,
    line,
    unit,
    ledger,
    run,
    vars,
    prompt,
    closes_at,
    bound: Duration::from_secs(u64::try_from(kept.minutes * 60).unwrap_or(300)) + WRITING_MARGIN,
  };

  tokio::spawn(async move { going.go(None).await });

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
  // On a program that carries its own turns the road's step reads the effort itself.
  let effort = if taken.provider == PROVIDER {
    effort_of(
      ground,
      &ground.machine(),
      PROVIDER,
      &taken.model,
      mapping.effort.as_deref(),
      Some(&reasoning),
    )
    .await
    .map_err(failed)?
    .unwrap_or_default()
  } else {
    mapping.effort.clone().unwrap_or_default()
  };
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

/// One turn of a research run in a place of its own as the place's program is handed it
/// (docs/research-runs.md, 「The road」): the line's standing instructions, the run's MCP servers and no tool
/// of the program's own, on the model the road names. Whether it thinks is the line's own switch where it
/// names one and the road's mapping otherwise; its effort is the road's mapping alone.
pub fn place_run(
  run: &RunnerRun,
  model: &str,
  thinking: Option<bool>,
  place: &str,
  prompt: &str,
  env: BTreeMap<String, String>,
) -> crate::lines::chat::ChatRun {
  crate::lines::chat::ChatRun {
    line: run.line,
    agent: place.to_owned(),
    model: model.to_owned(),
    effort: run.effort.clone(),
    tier: Some(run.tier.clone()),
    reasoning: crate::lines::worker::Reasoning {
      thinking,
      level: None,
    },
    thinking: run.thinking,
    system_path: run.system_prompt.clone(),
    system_parts: None,
    room: crate::lines::chat::RoomContext {
      chat_id: 0,
      text: String::new(),
      mark: String::new(),
    },
    tools_shown: String::new(),
    tools_allowed: run.tools_allowed.to_owned(),
    mcp_config: run.mcp_config.clone(),
    mcp_tools: None,
    prompt: prompt.to_owned(),
    persona_unused: None,
    fold: false,
    ping: false,
    resume: None,
    cwd: run.cwd.to_string_lossy().into_owned(),
    env,
  }
}

// A run under way: everything it was launched with.
struct Going {
  ground: Arc<Ground>,
  /// The road the run went out on.
  taken: Road,
  ask: ResearchRunAsk,
  line: String,
  unit: String,
  ledger: i64,
  run: RunnerRun,
  vars: BTreeMap<String, String>,
  prompt: String,
  /// When its web tools close, in epoch milliseconds.
  closes_at: i64,
  bound: Duration,
}

// Where a run in places stands as a turn of it goes in: how many of its turns went into the place before it,
// the round it asks and when the rounds began, and what the turns before it answered and spent.
struct At<'a> {
  before: usize,
  round: Option<usize>,
  rounds_began: Option<i64>,
  answers: &'a [String],
  spent: Option<Spent>,
}

// Where the rounds of a run in places go from: the round, how many turns went into their place before it,
// when they began, and whether its turn is one a server before this one put in, to be taken up.
struct RoundsFrom {
  round: usize,
  before: usize,
  began: i64,
  taking_up: bool,
}

// Why a run taken up again ends with no answer when its place's program went away under the turn waited on.
const PLACE_GONE: &str =
  "the program in the run's place went away while the run was taken up again after agency restarted";

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
  /// The run variables this turn goes out with: the run's own, or the rounds' narrower ones.
  vars: &'a BTreeMap<String, String>,
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

    for (name, value) in self.vars {
      env.insert(name.clone(), Some(value.clone()));
    }

    let command = Command {
      args,
      cwd: going.run.cwd.to_string_lossy().into_owned(),
      env,
    };

    // Where this login's session writes, so a run cut under it still hands back what it had written.
    keep_in_flight(
      &ground.db,
      going.ledger,
      &InFlight {
        record: Some(apart::record_of(ground, &command, &self.session).to_string_lossy().into_owned()),
        ..going.in_flight()
      },
    )
    .await;

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
  // What the run's settings row keeps of it before a session writes anything.
  fn in_flight(&self) -> InFlight {
    InFlight {
      key: self.ask.key.clone(),
      query: self.ask.query.clone(),
      room: self.ask.room_id,
      model: self.run.model.clone(),
      record: None,
      placed: None,
    }
  }

  // Where the run stands kept before a turn goes into a place, the place marked as the run's first, so a server
  // started after this one finds the place and the turn that went into it.
  async fn keep_placed(&self, place: &str, at: At<'_>) -> Result<(), NoAnswer> {
    mark_place(&self.ground, place, self.ledger).map_err(NoAnswer::Failed)?;

    let placed = Placed {
      ask: self.ask.clone(),
      taken: self.taken.clone(),
      unit: self.unit.clone(),
      prompt: self.prompt.clone(),
      closes_at: self.closes_at,
      bound_ms: u64::try_from(self.bound.as_millis()).unwrap_or(u64::MAX),
      run: KeptRun::of(&self.run),
      place: place.to_owned(),
      before: at.before,
      round: at.round,
      rounds_began: at.rounds_began,
      answers: at.answers.to_vec(),
      spent: at.spent,
    };

    keep_in_flight(
      &self.ground.db,
      self.ledger,
      &InFlight {
        placed: Some(placed),
        ..self.in_flight()
      },
    )
    .await;

    Ok(())
  }

  async fn note(&self, log: &mut RunLog, text: String) {
    log.write(&self.ground.db, Line::note(text)).await;
  }

  // The run to its end: its first answer on the logins its key's walk offers, then each of the line's rounds
  // on the same session and login, then its ledger run closed and its result queued for the room.
  //
  // A run in places a server before this one sent is taken up again where it stood.
  async fn go(self, taken_up: Option<Placed>) {
    let ground = self.ground.clone();
    let mut log = RunLog::new(Target {
      agent: self.line.clone(),
      run: self.unit.clone(),
      chat_id: self.ask.room_id,
      job_id: None,
    });

    if taken_up.is_some() {
      self
        .note(&mut log, "The research was taken up again after agency restarted.".to_owned())
        .await;
    } else {
      self
        .note(
          &mut log,
          format!("Research on {} · {}: {}", self.ask.key, self.run.model, self.ask.query.trim()),
        )
        .await;

      keep_in_flight(&ground.db, self.ledger, &self.in_flight()).await;
    }

    let (outcome, result, spent) =
      if taken_up.is_some() || crate::chat_stand_in::carries_own_turns(&self.taken.provider) {
        self.on_places(&mut log, taken_up.as_ref()).await
      } else {
        self.on_claude(&mut log).await
      };

    self.ended(outcome, result, spent, &mut log).await;
  }

  // The run on Claude Code: its first answer on the logins its key's walk offers, then each of the line's
  // rounds on a session of their own and the same login.
  async fn on_claude(&self, log: &mut RunLog) -> (&'static str, Result<String, String>, Option<Spent>) {
    let ground = self.ground.clone();
    let session = new_session();
    let mut first = OnLogin {
      going: self,
      session: session.clone(),
      resume: false,
      prompt: self.prompt.clone(),
      account: None,
      vars: &self.vars,
    };
    let answered = on_accounts(&Walking { ground: &ground }, &self.ask.key, None, &mut first).await;
    let account = first.account.clone();
    match answered {
      Err(Failure::Transport { message }) => (CUT, Err(message), None),
      Err(failure) => ("failed", Err(failure.message()), None),
      Ok(reply) => {
        let mut spent = sum(None, spent_of(&reply));
        let mut answers = vec![reply.text.clone()];

        self.rounds(&session, account, &mut answers, &mut spent, log).await;
        ("answered", Ok(answers.pop().unwrap_or_default()), spent)
      }
    }
  }

  // The run on a program that carries its own turns (docs/research-runs.md, 「The road」): its first answer in a
  // place of the run's own, each of the line's rounds in a second, both ended with the run. A run taken up again
  // goes on from the turn it stood on.
  async fn on_places(
    &self,
    log: &mut RunLog,
    taken_up: Option<&Placed>,
  ) -> (&'static str, Result<String, String>, Option<Spent>) {
    let first_place = format!("{}-{}", self.line, self.ledger);
    let rounds_place = format!("{first_place}{ROUNDS_PLACE}");
    let ended = match taken_up.and_then(|placed| placed.round.map(|round| (placed, round))) {
      Some((placed, round)) => {
        let mut answers = placed.answers.clone();
        let mut spent = placed.spent;
        let from = RoundsFrom {
          round,
          before: placed.before,
          began: placed.rounds_began.unwrap_or_else(now_ms),
          taking_up: true,
        };

        self.place_rounds(&rounds_place, from, &mut answers, &mut spent, log).await;
        ("answered", Ok(answers.pop().unwrap_or_default()), spent)
      }
      None => {
        let at = At {
          before: 0,
          round: None,
          rounds_began: None,
          answers: &[],
          spent: None,
        };
        let first = match self.keep_placed(&first_place, at).await {
          Ok(()) => self.turn(&first_place, 0, taken_up.is_some(), &self.prompt, &self.vars, log).await,
          Err(no) => Err(no),
        };

        match first {
          Err(NoAnswer::Cut(why)) => (CUT, Err(why), None),
          Err(no) => ("failed", Err(crate::herdr_backend::failure_of(no).0), None),
          Ok(reply) => {
            let mut spent = crate::herdr_backend::spent_of(&reply.spend);
            let mut answers = vec![reply.text.clone()];
            let from = RoundsFrom {
              round: 0,
              before: 0,
              began: now_ms(),
              taking_up: false,
            };

            self.place_rounds(&rounds_place, from, &mut answers, &mut spent, log).await;
            ("answered", Ok(answers.pop().unwrap_or_default()), spent)
          }
        }
      }
    };

    for place in [&first_place, &rounds_place] {
      end_place(&self.ground, place).await;
    }

    ended
  }

  // A turn of the run in a place: where it is being taken up, the turn a server before this one put in waited on,
  // or put in now where that server never put it in; otherwise put in.
  async fn turn(
    &self,
    place: &str,
    before: usize,
    taking_up: bool,
    prompt: &str,
    vars: &BTreeMap<String, String>,
    log: &mut RunLog,
  ) -> Result<crate::companion::run::Reply, NoAnswer> {
    if taking_up && let Some(came) = self.taken_up(place, before).await {
      return came;
    }

    self.in_place(place, prompt, vars, log).await
  }

  // The turn a server before this one put into a place, after as many of the run's turns as `before`, waited on
  // under the place's lock until the place's record holds its end; none where the place holds no such turn, it
  // never having gone in.
  async fn taken_up(
    &self,
    place: &str,
    before: usize,
  ) -> Option<Result<crate::companion::run::Reply, NoAnswer>> {
    let ground = &self.ground;
    let data = &ground.roots.data;
    let _held = match hold(agent_lock_path(data, place)).await {
      Ok(held) => held,
      Err(why) => return Some(Err(NoAnswer::Failed(why.to_string()))),
    };
    let at = crate::lines::place::Place::of(data, place);
    let every = Duration::from_millis(policy::resident_turn::RESIDENT_TURN.inbox_poll_ms);
    let mut unseen = 0u32;

    own_rows(ground, &self.unit).await;

    loop {
      let came = match standing_after(&at, &self.unit, before) {
        Standing::Lost => {
          close_rows(ground, &self.unit, false).await;

          return None;
        }
        Standing::Ended(came) => came,
        Standing::Going => {
          unseen = if at.program_alive().is_some() { 0 } else { unseen + 1 };

          if unseen < 40 {
            tokio::time::sleep(every).await;
            continue;
          }

          Came::Failed(PLACE_GONE.to_owned())
        }
      };
      let answered = matches!(came, Came::Answered(_));

      close_rows(ground, &self.unit, answered).await;

      let _ = end_run(&at.path, now_ms());

      return Some(match came {
        Came::Answered(reply) => Ok(self.reply_of(*reply)),
        Came::Failed(why) if why == PLACE_GONE => Err(NoAnswer::Cut(why)),
        Came::Failed(why) => Err(NoAnswer::Failed(why)),
      });
    }
  }

  // A turn's end as a place's record holds it, as a turn put in answers it.
  fn reply_of(&self, reply: Reply) -> crate::companion::run::Reply {
    crate::companion::run::Reply {
      text: reply.text,
      session_id: reply.session_id.unwrap_or_default(),
      mode: reply.mode,
      model: reply.model.unwrap_or_else(|| self.taken.model.clone()),
      effort: None,
      spend: reply.spend,
      context_ratio: reply.context_ratio,
      context_tokens: reply.context_tokens,
      account: None,
    }
  }

  // One turn of the run in a place of its own, on the road the run went out on, its program composed as a
  // worker's job on that road is.
  async fn in_place(
    &self,
    place: &str,
    prompt: &str,
    vars: &BTreeMap<String, String>,
    log: &mut RunLog,
  ) -> Result<crate::companion::run::Reply, NoAnswer> {
    let ground = &self.ground;
    let mut env = ground.env().clone();

    mark_place(ground, place, self.ledger).map_err(NoAnswer::Failed)?;
    env.extend(vars.iter().map(|(name, value)| (name.clone(), value.clone())));

    let thinking = crate::lines::worker::reasoning_of(ground, &self.line)
      .await
      .map_err(|why| NoAnswer::Failed(why.detail))?
      .thinking;
    let run = place_run(&self.run, &self.taken.model, thinking, place, prompt, env);

    crate::chat_stand_in::job_turn_on(ground, log, &run, &self.ask.key, &self.taken, now_ms(), None).await
  }

  // The line's rounds in their own place, each a turn of the same session there: as on Claude Code, past the
  // line's ceiling the final one asked at once or the run returning what it has, and a round that does not
  // answer leaving the last answer standing. Each round is kept before it goes in; a round taken up again is
  // waited on whatever the ceiling.
  async fn place_rounds(
    &self,
    place: &str,
    from: RoundsFrom,
    answers: &mut Vec<String>,
    spent: &mut Option<Spent>,
    log: &mut RunLog,
  ) {
    let db = &self.ground.db;
    let Ok(Some(process)) = crate::research_draft::process_of(db, &self.ask.key).await else {
      return;
    };
    let rounds_vars = rounds_variables(&self.vars);
    let mut asking = from.round;
    let mut put = from.before;
    let mut taking_up = from.taking_up;

    while asking < process.rounds.len() {
      let name = process.rounds[asking].clone();
      let past = (now_ms() - from.began) / 1000 >= process.ceiling_seconds;

      if past && !taking_up && name.as_str() != policy::research::Draft::FINAL {
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
        taking_up = false;
        continue;
      };
      let prompt = crate::research_draft::prompt(&question, &self.prompt, answers);
      let at = At {
        before: put,
        round: Some(asking),
        rounds_began: Some(from.began),
        answers: answers.as_slice(),
        spent: *spent,
      };
      let turn = match self.keep_placed(place, at).await {
        Ok(()) => self.turn(place, put, taking_up, &prompt, &rounds_vars, log).await,
        Err(no) => Err(no),
      };

      taking_up = false;
      put += 1;

      match turn {
        Ok(reply) => {
          if let Some(more) = crate::herdr_backend::spent_of(&reply.spend) {
            *spent = sum(spent.take(), more);
          }

          answers.push(reply.text);
        }
        Err(no) => {
          let why = crate::herdr_backend::failure_of(no).0;

          self.note(log, format!("The round {name} did not answer: {why}")).await;
          return;
        }
      }

      if name.as_str() == policy::research::Draft::FINAL {
        return;
      }

      asking += 1;
    }
  }

  // The run closed in the ledger, its result filed and queued for the room, and its run under way let go.
  async fn ended(
    &self,
    outcome: &'static str,
    result: Result<String, String>,
    spent: Option<Spent>,
    log: &mut RunLog,
  ) {
    let ground = self.ground.clone();
    let closing = Closing {
      finished_at: now_ms(),
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
      Ok(_) => self.note(log, "The research answered.".to_owned()).await,
      Err(why) => self.note(log, format!("The research ended with no answer: {why}")).await,
    }

    let told = if let Ok(text) = &result
      && outcome != CUT
    {
      let trees = Trees::of(&ground);
      let language = crate::research_document::language_of(&self.ask.query);
      let drawn = crate::research_document::words(&trees, language)
        .map(|words| crate::research_document::draw(text, &words));
      let filed = match &drawn {
        Ok(drawn) => {
          if drawn.narration.is_some() {
            self
              .note(log, "The narration was lifted out of the document for the voice path.".to_owned())
              .await;
          }

          let filing = crate::research_document::filing(
            drawn,
            self.ledger,
            &self.ask.key,
            &self.ask.query,
            self.ask.room_id,
          );

          file_result(ground.vars(), &filing, drawn).await
        }
        Err(why) => Err(why.clone()),
      };

      match filed {
        Ok(Some(link)) => {
          self.note(log, format!("The result was filed as a document: {link}")).await;

          let brief = brief_of(text).unwrap_or_else(|| text.clone());

          linked_instruction(self.ledger, &self.ask.key, &self.ask.query, self.ask.room_id, &brief, &link)
        }
        Ok(None) => {
          self
            .note(log, "No artifact service is named, so the result goes to the room whole.".to_owned())
            .await;
          self.instruction(&result)
        }
        Err(why) => {
          self
            .note(log, format!("The result was not filed, so it goes to the room whole: {why}"))
            .await;
          self.instruction(&result)
        }
      }
    } else if outcome == CUT {
      let written = in_flight(&ground.db, self.ledger)
        .await
        .and_then(|kept| kept.record)
        .and_then(|record| written_so_far(std::path::Path::new(&record)));

      cut_instruction(
        self.ledger,
        &self.ask.key,
        Some(&self.ask.query),
        self.ask.room_id,
        written.as_deref(),
      )
    } else {
      self.instruction(&result)
    };

    if let Err(why) = crate::job_end::instruct(ground.vars(), &told).await {
      eprintln!("research {}: its result was not told to the room: {why}", self.ledger);
    }

    if let Err(why) = settings::forget(&ground.db, &format!("{IN_FLIGHT_KEY}{}", self.ledger)).await {
      eprintln!("research {}: the run under way was not let go: {}", self.ledger, why.detail);
    }
  }

  // The line's rounds after the first answer, each on the same session and login, past the line's ceiling the
  // final one asked at once or the run returning what it has, as the policy says; a round that does not answer
  // leaves the last answer standing.
  async fn rounds(
    &self,
    _first: &str,
    account: Option<Account>,
    answers: &mut Vec<String>,
    spent: &mut Option<Spent>,
    log: &mut RunLog,
  ) {
    let db = &self.ground.db;
    let Ok(Some(process)) = crate::research_draft::process_of(db, &self.ask.key).await else {
      return;
    };
    let rounds_session = new_session();
    let rounds_vars = rounds_variables(&self.vars);
    let mut asked_any = false;
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
      // The rounds go to a session of their own, opened by the first of them, so no round reads again the
      // pages the search session fetched: each carries the task and the last answer.
      let mut round = OnLogin {
        going: self,
        session: rounds_session.clone(),
        resume: asked_any,
        prompt: crate::research_draft::prompt(&question, &self.prompt, answers),
        account: None,
        vars: &rounds_vars,
      };

      match round.attempt(account.clone()).await {
        Ok(reply) => {
          asked_any = true;
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

// How long the end of a run's place waits for a turn still holding it.
const PLACE_END_WAIT: Duration = Duration::from_secs(30);

// A place a run stood in ended with it: its program released, its workspace closed and its folder taken away,
// under its lock.
async fn end_place(ground: &Ground, name: &str) {
  let data = &ground.roots.data;
  let Some(_held) =
    crate::places::locks::hold_within(crate::places::locks::agent_lock_path(data, name), PLACE_END_WAIT)
      .await
  else {
    return eprintln!("research place {name}: a turn still holds it, so it was not ended");
  };

  let place = crate::lines::place::Place::of(data, name);

  // The session token its program carried is taken back with it, so the run's tools end with the run.
  if let Some(token) = place.release().await.as_ref().and_then(crate::lines::place::token_of) {
    crate::lines::token::revoke(ground, token).await;
  }

  if let Some(home) = ground.home_of(name, name) {
    let _ = home.close().await;
  }

  let _ = std::fs::remove_dir_all(&place.path);
}

// What the place a run's rounds go into is named by after the run's first place.
const ROUNDS_PLACE: &str = "-rounds";

/// The record a research run's own place keeps while the run stands in it, naming the run, so a turn into the
/// place stands under no agent's row (docs/research-runs.md, 「The road」).
pub const RUN_PLACE: &str = "research-place.json";

/// Whether a name is a research run's own place, as its record says.
pub fn is_run_place(data: &std::path::Path, name: &str) -> bool {
  agent_host::disk::read_json::<Value>(&crate::lines::place::Place::of(data, name).path.join(RUN_PLACE))
    .is_some_and(|record| record["place"] == name)
}

/// Every research run's own place standing now, by its name, which no sweep of leftovers takes down.
pub fn run_places(data: &std::path::Path) -> Vec<String> {
  let Ok(entries) = std::fs::read_dir(crate::lines::place::places_dir(data)) else {
    return vec![];
  };

  entries
    .filter_map(|entry| entry.ok())
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .filter(|name| is_run_place(data, name))
    .collect()
}

// A research run's place made ready for its turns: its folder and the record that names it.
fn mark_place(ground: &Ground, name: &str, ledger: i64) -> Result<(), String> {
  let path = crate::lines::place::Place::of(&ground.roots.data, name).path;

  std::fs::create_dir_all(&path).map_err(|why| format!("{}: {why}", path.display()))?;
  std::fs::write(path.join(RUN_PLACE), json!({ "place": name, "run": ledger }).to_string())
    .map_err(|why| format!("{}: {why}", path.display()))
}
