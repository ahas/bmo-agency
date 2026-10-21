//! Whether a stand-in answers the rooms while Claude Code rests (docs/cache-fallback.md, 「While a
//! fallback stands, the room is not silent」): the walk of the chat's own key, as it stands now, meets a
//! provider that would answer before it reaches a Claude Code entry its logins have not all closed. A room
//! the rest's gate closed is taken only where one does. Any work key's walk is read the same way, and the
//! step a walk goes out on — its model, effort and answer room — resolved for that key. A login's weekly
//! share is read off the windows agency keeps for it (docs/usage-windows.md); the rests and the fallback's
//! mark are agency's own settings.

use cache_siren::{Keeping, Slot};
use harness_providers::registered::Registered;
use harness_providers::{Machine, Window};
use policy::providers::openrouter::OPENROUTER;
use policy::rest::REST;
use policy::work_keys::WORK_KEYS;
use sea_orm::DatabaseConnection;

use crate::control::harness::{enabled_providers, named};
use crate::control::own_llm::{self, OwnLlmError};
use crate::control::registry::{self, Mapping};
use crate::control::roads::{pinned_provider, walked};
use crate::control::tiers;
use crate::events::{Bus, Event};
use crate::lines::worker::{Reasoning, effort_of};
use crate::places::Ground;
use crate::{Refusal, now_ms};
use store::{all, one};

/// The provider the walk stands in for.
pub const CLAUDE: &str = "claude-code";
const CODEX: &str = "codex";
const DEEPSEEK: &str = "deepseek";
const OPENROUTER_NAME: &str = "openrouter";

/// One provider whose models are the registry's, as the walk sees it.
pub struct Service {
  pub provider: String,
  /// Whether the provider is present on this machine.
  pub present: bool,
  /// Whether the key's walk takes its registration.
  pub walked: bool,
}

/// The facts the walk reads.
pub struct Standing {
  /// The provider the key is set to, where it is set to one.
  pub route: Option<String>,
  /// The owner's order of the providers.
  pub order: Vec<String>,
  pub codex_present: bool,
  pub openrouter_present: bool,
  pub services: Vec<Service>,
  /// Whether a login's rest counts as its own limit, which it does unless the service's outage stands.
  pub limit_rest: bool,
  /// Each enabled login: whether its own rest stands, and its weekly share.
  pub accounts: Vec<(bool, Option<f64>)>,
}

async fn setting(db: &DatabaseConnection, key: &str) -> Option<String> {
  crate::queue::settings::get(db, key).await.ok().flatten()
}

fn leading(value: &str) -> Option<i64> {
  let digits: String = value
    .trim_start()
    .chars()
    .take_while(|one| one.is_ascii_digit() || *one == '-')
    .collect();

  digits.parse().ok()
}

// How long one login's own limit rest stands: its own key, or for this machine's own login the rest written
// with no login named where that is no outage.
async fn account_rest(db: &DatabaseConnection, account: &str, outage: bool) -> Option<i64> {
  if let Some(own) = setting(db, &format!("rest_until:{account}")).await.and_then(|value| leading(&value)) {
    return Some(own);
  }

  if account != crate::control::claude_accounts::SEED_ACCOUNT || outage {
    return None;
  }

  setting(db, "rest_until").await.and_then(|value| leading(&value))
}

// One login's weekly share, read off the window its provider counts as its week among the windows agency
// keeps for it, shortest first; none where it holds no reading. A keeping that cannot be read is said with
// its cause, and closes nothing.
async fn weekly(windows: &Keeping, account: &str) -> Option<f64> {
  let mut read: Vec<_> = match windows
    .read(Slot {
      mode: CLAUDE,
      account,
    })
    .await
  {
    Ok(reading) => reading.windows.into_iter().map(|kept| kept.window).collect(),
    Err(fault) => {
      eprintln!("{CLAUDE} windows of {account}: not read: {fault}");
      return None;
    }
  };

  read.sort_by(|one, other| (one.duration_minutes, &one.name).cmp(&(other.duration_minutes, &other.name)));

  let named: Vec<Window> = read
    .iter()
    .map(|window| Window {
      name: &window.name,
      duration_minutes: window.duration_minutes,
    })
    .collect();
  let week = harness_providers::provider(CLAUDE)?.headline(&named).week?;

  Some(read.get(week)?.utilization)
}

async fn standing(ground: &Ground, machine: &Machine, key: &str) -> Option<Standing> {
  let db = &ground.db;
  let now = now_ms() / 1000;
  let line = WORK_KEYS.profile(key).map(|profile| profile.line.clone());
  let mapped = |provider: &'static str| async move { tiers::resolve(db, key, provider).await.ok() };
  let outage = setting(db, "rest_kind").await.as_deref() == Some("outage");
  let unnamed = setting(db, "rest_until").await.and_then(|value| leading(&value));
  let codex = harness_providers::provider(CODEX);
  let mut services = Vec::new();

  for provider in harness_providers::providers() {
    // A road whose runs are a service of its own: the registry's, whose models are the archive's, and
    // DeepSeek's, whose models its policy declares.
    let service = provider.unregistered().is_some() || provider.name() == DEEPSEEK;
    if !service || line.as_deref().is_some_and(|line| !provider.declared().carries(line)) {
      continue;
    }

    let chosen = mapped(provider.name()).await.is_some();
    let addressed = one(db, r#"SELECT 1 AS "one" FROM "registered_model_addresses" LIMIT 1"#, vec![])
      .await
      .ok()
      .flatten()
      .is_some();
    let present =
      chosen && provider.missing(machine).is_none() && (provider.unregistered().is_none() || addressed);

    services.push(Service {
      provider: provider.name().to_owned(),
      present,
      walked: match &line {
        None => true,
        Some(line) => walked(db, provider, key, line).await.ok()?,
      },
    });
  }

  let logins = all(
    db,
    r#"SELECT "id" FROM "claude_accounts" WHERE "enabled" <> 0 ORDER BY "place", "rowid""#,
    vec![],
  )
  .await
  .ok()?;
  let mut accounts = Vec::new();

  for login in &logins {
    let id: String = login.try_get("", "id").ok()?;
    let resting = account_rest(db, &id, outage).await.is_some_and(|until| until > now);

    accounts.push((resting, weekly(&ground.windows, &id).await));
  }

  Some(Standing {
    route: pinned_provider(db, key).await.ok()?,
    order: enabled_providers(db).await.ok()?,
    codex_present: mapped(CODEX).await.is_some()
      && codex.is_some_and(|codex| codex.missing(machine).is_none()),
    openrouter_present: ground.vars().get(&OPENROUTER.presence.key_env).is_some_and(|key| !key.is_empty())
      && mapped(OPENROUTER_NAME).await.is_some(),
    services,
    limit_rest: !(outage && unnamed.is_some_and(|until| until > now)),
    accounts,
  })
}

/// Whether a provider offers a stand-in now.
pub fn steps(standing: &Standing, provider: &str) -> bool {
  match provider {
    CODEX => standing.codex_present,
    OPENROUTER_NAME => standing.openrouter_present,
    _ => standing.services.iter().any(|service| service.provider == provider && service.present),
  }
}

// Whether every login is closed: its own limit rest standing and its weekly share past the ceiling.
fn claude_closed(standing: &Standing) -> bool {
  standing.limit_rest
    && !standing.accounts.is_empty()
    && standing
      .accounts
      .iter()
      .all(|(resting, weekly)| *resting && weekly.is_some_and(|weekly| weekly >= REST.weekly_threshold))
}

/// Whether an agent answers while the rest closes Claude Code's road: an agent pinned to another provider
/// stands on a road the rest does not close, and any other answers where its key's walk meets a stand-in
/// now.
pub async fn answers(ground: &Ground, machine: &Machine, agent: &str) -> bool {
  let pinned = one(
    &ground.db,
    r#"SELECT "pinned_provider" FROM "agents" WHERE "name" = $1"#,
    vec![agent.into()],
  )
  .await
  .ok()
  .flatten()
  .and_then(|row| row.try_get::<Option<String>>("", "pinned_provider").ok().flatten());

  if pinned.is_some_and(|provider| provider != CLAUDE) {
    return true;
  }

  standing(ground, machine, agent).await.is_some_and(|standing| meets(&standing))
}

/// The stand-ins a turn of the chat asks, in the owner's order, whether it must answer on one of them
/// rather than fall to Claude Code, and the road the key is set to — Claude Code's own where the key walks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Walk {
  pub steps: Vec<String>,
  pub required: bool,
  pub chosen: String,
}

/// One work key's walk now, or none where Claude Code is the road it stands on.
pub async fn walk_of(ground: &Ground, machine: &Machine, key: &str) -> Option<Walk> {
  walk_over(&standing(ground, machine, key).await?)
}

/// The road a stand-in step goes out on: its provider, the model and effort the walk chose, and on a
/// registered model the room its answer is given and whether it thinks.
pub struct Step {
  pub provider: String,
  pub model: String,
  pub effort: String,
  pub thinking: bool,
  pub answer_tokens: Option<i64>,
}

/// A step's model and effort, as the key is mapped on the provider with its agent's own thinking and
/// effort; on a registered model, the registration's own run settings with the answer room the key's work
/// is given — never below the floor its own work declares, nor, where the run thinks, below its thinking
/// floor as far as a quarter of the registration's window allows.
pub async fn step_of(
  ground: &Ground,
  machine: &Machine,
  key: &str,
  provider: &str,
  reasoning: &Reasoning,
) -> Result<Step, String> {
  let db = &ground.db;
  let mapping = tiers::resolve_execution(db, key, provider, None).await?;
  let effort =
    effort_of(ground, machine, provider, &mapping.model, mapping.effort.as_deref(), Some(reasoning))
      .await
      .map_err(|why| why.detail)?
      .unwrap_or_default();

  if provider != registry::PROVIDER {
    let thinking = if provider == "deepseek" { effort != "none" } else { effort == "thinking" };

    return Ok(Step {
      provider: provider.to_owned(),
      model: mapping.model,
      thinking,
      effort,
      answer_tokens: None,
    });
  }

  let asked = Mapping {
    model: mapping.model.clone(),
    effort: Some(effort).filter(|effort| !effort.is_empty()),
    answer_tokens: mapping.answer_tokens,
  };

  registered_step(db, key, &asked).await
}

/// A step on the registration a caller names rather than the one the key maps to: the registration's own
/// run settings, with the effort the key's agent's own reasoning makes of it, and the answer room the key's
/// work is given, as the key's mapped step is.
pub async fn registration_step(
  ground: &Ground,
  key: &str,
  registration: &str,
  reasoning: &Reasoning,
) -> Result<Step, String> {
  let effort =
    effort_of(ground, &ground.machine(), registry::PROVIDER, registration, None, Some(reasoning))
      .await
      .map_err(|why| why.detail)?;
  let asked = Mapping {
    model: registration.to_owned(),
    effort: effort.filter(|effort| !effort.is_empty()),
    answer_tokens: None,
  };

  registered_step(&ground.db, key, &asked).await
}

// A step on a registered model: its run settings, never below the answer floor the key's work declares.
async fn registered_step(db: &DatabaseConnection, key: &str, asked: &Mapping) -> Result<Step, String> {
  let settings = registry::run_settings(db, Some(asked)).await.map_err(|why| why.error)?;
  let profile = WORK_KEYS.profile(key);
  let thinking_floor = match (
    settings.thinking,
    profile.and_then(|one| one.thinking_answer_floor),
    settings.context_tokens,
  ) {
    (true, Some(floor), Some(window)) => floor.min(window / 4),
    _ => 0,
  };
  let floor = profile.and_then(|one| one.answer_floor).unwrap_or(0).max(thinking_floor);

  Ok(Step {
    provider: registry::PROVIDER.to_owned(),
    model: settings.model.clone(),
    effort: settings.effort.clone().unwrap_or_default(),
    thinking: settings.thinking,
    answer_tokens: Some(settings.answer_tokens.max(floor)),
  })
}

/// The registration held to the gateway's list, which must offer it as the service's own.
pub async fn listed(ground: &Ground, service: &registry::OwnLlmService, model: &str) -> Result<(), String> {
  let held = async {
    let key = own_llm::credential(&Registered::key_file(&ground.machine()))?;

    own_llm::offers(service, &own_llm::models(service, &key).await?, model)
  };

  held.await.map_err(|why: OwnLlmError| why.said(service))
}

/// A stand-in has started answering, or a standing fallback has moved to another (docs/api.md, 「Events」):
/// the owner it is told to, the stand-in that answered before where a fallback already stood, and the
/// provider and model answering now. What the owner is told, and on which road, is the consumer's.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackAnswering {
  pub owner_id: i64,
  pub previous: Option<String>,
  pub provider: String,
  pub model: String,
}

impl Event for FallbackAnswering {
  const NAME: &'static str = "fallback.changed";
}

/// The stand-in that now answers marked and published, once for each move: the mark of the road answering
/// set with the moment the fallback began, and `FallbackAnswering` published on the bus.
pub async fn announce(
  ground: &Ground,
  events: &Bus,
  owner_id: i64,
  provider: &str,
  model: &str,
) -> Result<(), Refusal> {
  let db = &ground.db;
  let previous = crate::control::fallback::fallback_provider(db)
    .await
    .map_err(|why| crate::lines::fault(why.error))?;

  if previous.as_deref() == Some(provider) {
    return Ok(());
  }

  let since = crate::queue::settings::get(db, "fallback_since").await?;

  if since.is_none() {
    crate::queue::settings::set(db, "fallback_since", &(now_ms() / 1000).to_string()).await?;
  }

  crate::queue::settings::set(db, "fallback_provider", provider).await?;
  println!("fallback: answering on {model} through {provider}");
  events.publish(FallbackAnswering {
    owner_id,
    previous: previous.filter(|_| since.is_some()),
    provider: provider.to_owned(),
    model: model.to_owned(),
  });

  Ok(())
}

/// The walk over the standing: a key set to Claude Code walks nothing where Claude Code is on; a key set to
/// another provider asks that one alone and must answer there; a key that walks asks each stand-in above the
/// Claude Code entry, and those below it only once every login is closed, a registry's provider whose
/// registration the key does not walk passed by.
pub fn walk_over(standing: &Standing) -> Option<Walk> {
  let has_claude = standing.order.iter().any(|provider| provider == CLAUDE);

  match standing.route.as_deref() {
    Some(CLAUDE) => {
      return (!has_claude).then(|| Walk {
        steps: vec![],
        required: true,
        chosen: CLAUDE.to_owned(),
      });
    }
    Some(route) => {
      let offers = standing.order.iter().any(|provider| provider == route) && steps(standing, route);

      return Some(Walk {
        steps: if offers { vec![route.to_owned()] } else { vec![] },
        required: true,
        chosen: route.to_owned(),
      });
    }
    None => {}
  }

  let mut walked = vec![];

  for provider in &standing.order {
    if provider == CLAUDE {
      if !claude_closed(standing) {
        break;
      }

      continue;
    }

    if standing.services.iter().any(|service| &service.provider == provider && !service.walked) {
      continue;
    }

    if named(provider).is_ok() && steps(standing, provider) {
      walked.push(provider.clone());
    }
  }

  (!walked.is_empty() || !has_claude).then(|| Walk {
    steps: walked,
    required: !has_claude,
    chosen: "auto".to_owned(),
  })
}

/// Whether the walk over the standing meets a stand-in: the one provider a key is set to, where it is not
/// Claude Code and offers one; otherwise the first in the owner's order before a Claude Code entry its
/// logins have not all closed, a registry's provider whose registration the key does not walk passed by.
pub fn meets(standing: &Standing) -> bool {
  if let Some(route) = &standing.route {
    return route != CLAUDE && standing.order.contains(route) && steps(standing, route);
  }

  for provider in &standing.order {
    if provider == CLAUDE {
      if !claude_closed(standing) {
        return false;
      }

      continue;
    }

    if standing.services.iter().any(|service| &service.provider == provider && !service.walked) {
      continue;
    }

    if named(provider).is_ok() && steps(standing, provider) {
      return true;
    }
  }

  false
}
