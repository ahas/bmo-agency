//! A worker's pin as the providers that would run it read it (docs/own-llm.md, 「A worker pinned to a
//! provider」): `auto` for none, a provider, or `<provider>::<model>` on a provider that takes a model of
//! its own. Each provider answers for itself which lines it carries and whether it takes a model; the
//! registrations a model names are read from the store here.

use harness_providers::{ARCHIVE_PROVIDERS, Machine};
use policy::fill_marks;
use policy::queue_words::QUEUE_WORDS;
use sea_orm::ConnectionTrait;

use super::routes::job_line;
use crate::control::Refused;
use crate::control::harness::named;
use crate::control::registry;

/// What a worker is pinned to: a provider and one model of it, or neither for no pin.
#[derive(Debug, PartialEq, Eq)]
pub struct Pin {
  pub provider: Option<String>,
  pub model: Option<String>,
}

/// The pin a value names, or the sentence saying why nothing could run on it.
pub async fn pin_choice<C: ConnectionTrait>(
  archive: &C,
  machine: &Machine,
  value: &str,
) -> Result<Result<Pin, String>, Refused> {
  if value == "auto" {
    return Ok(Ok(Pin {
      provider: None,
      model: None,
    }));
  }

  // The provider and the piece after it, whatever follows a second separator left aside.
  let mut pieces = value.split("::");
  let name = pieces.next().unwrap_or_default();
  let model = pieces.next();

  if !ARCHIVE_PROVIDERS.contains(&name) {
    return Ok(Err(fill_marks(
      &QUEUE_WORDS.pins.unknown,
      &[("providers", &ARCHIVE_PROVIDERS.join(", "))],
    )));
  }

  let provider = named(name)?;
  let label = &provider.declared().label;

  if !provider.declared().carries(job_line()) {
    return Ok(Err(fill_marks(&QUEUE_WORDS.pins.no_line, &[("label", label)])));
  }

  let Some(model) = model else {
    return Ok(Ok(Pin {
      provider: Some(name.to_owned()),
      model: None,
    }));
  };
  let Some(not_registered) = provider.not_registered() else {
    return Ok(Err(fill_marks(&QUEUE_WORDS.pins.no_model, &[("label", label)])));
  };

  // The registry's models are read from the store; a provider whose models its policy declares answers
  // for itself.
  if provider.unregistered().is_some() {
    let held = registry::registration(archive, model).await?;

    if held.is_none_or(|held| held.addresses.is_empty()) {
      return Ok(Err(format!("{not_registered}: {model}")));
    }
  } else if !provider.holds_model(machine, model) {
    return Ok(Err(format!("{not_registered}: {model}")));
  }

  Ok(Ok(Pin {
    provider: Some(name.to_owned()),
    model: Some(model.to_owned()),
  }))
}
