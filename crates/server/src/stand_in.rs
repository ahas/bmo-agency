//! The stand-ins' side of the chat's road (docs/cache-fallback.md, 「While a fallback stands, the room is not
//! silent」): whether an agent answers its rooms while the rest closes Claude Code's road, read off the one
//! road resolution (`crate::road`); the step a stand-in goes out on — its model, effort and answer room —
//! resolved for its key; and the stand-in that answers marked and published once as the fallback it is.

use harness_providers::Machine;
use harness_providers::registered::Registered;
use policy::work_keys::WORK_KEYS;
use sea_orm::DatabaseConnection;

use crate::control::own_llm::{self, OwnLlmError};
use crate::control::registry::{self, Mapping};
use crate::control::tiers;
use crate::events::{Bus, Event};
use crate::lines::worker::{Reasoning, effort_of};
use crate::places::Ground;
use crate::{Refusal, now_ms};
use store::one;

/// The provider the walk stands in for.
pub const CLAUDE: &str = "claude-code";

/// Whether an agent answers while the rest closes Claude Code's road: an agent pinned to another provider
/// stands on a road the rest does not close, and any other answers where the road resolution's walk of its
/// key meets a provider other than Claude Code that can answer now.
pub async fn answers(ground: &Ground, _machine: &Machine, agent: &str) -> bool {
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

  match crate::road::resolve(ground, &crate::road::Ask::key(agent)).await {
    Ok(resolution) => resolution.open().any(|road| road.provider != CLAUDE),
    Err(why) => {
      eprintln!("stand-in {agent}: its road could not be resolved: {why}");
      false
    }
  }
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
    let thinking = mapping
      .thinking
      .unwrap_or_else(|| if provider == "deepseek" { effort != "none" } else { effort == "thinking" });

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
    thinking: mapping.thinking,
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
    thinking: None,
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
