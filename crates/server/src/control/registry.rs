//! The registry of models as a caller reads and changes it (docs/own-llm.md, 「Where it is set」): every
//! registration with each address's key said in one word and what names it, and a change held to the
//! policy's rules. A key given in a save is written by the harness-machine crate into a file of agency's
//! own and never answered back (docs/harness-machine.md, 「The keys」). A registration as
//! the service a chat with no tool asks, and the settings a work key asks it for, stand here for every
//! route that reads them.

use std::path::Path;
use std::sync::LazyLock;

use axum::http::{Method, StatusCode};
use axum::response::Response;
use harness_machine::engine_health::{self, EngineHealth};
use harness_machine::{gateway, keys};
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::providers::registered::REGISTERED;
use policy::registry::{REGISTRY, WholeBounds};
use policy::tiers::TIERS;
use regex::Regex;
use sea_orm::{ConnectionTrait, QueryResult, TransactionTrait};
use serde_json::{Value, json};

use super::fault::{Fault, refused};
use super::{Call, Refused, Route, answer};
use crate::now_ms;
use crate::text::{grouped, length};
use store::{all, one, run};

/// The registry's provider, the one a registration is offered and pinned under.
pub const PROVIDER: &str = "registered";

static NAME: LazyLock<Regex> = LazyLock::new(|| pattern(&REGISTRY.name_pattern));
static MODEL: LazyLock<Regex> = LazyLock::new(|| pattern(&REGISTRY.model_pattern));
static KEY_FILE: LazyLock<Regex> = LazyLock::new(|| pattern(&REGISTRY.key_file_pattern));

fn pattern(text: &str) -> Regex {
  Regex::new(text).expect("the policy's registry patterns read")
}

/// One address of a registration, as its row keeps it.
#[derive(Clone, Debug)]
pub struct Address {
  pub node: String,
  pub url: String,
  pub key_file: Option<String>,
  pub key_owned: bool,
}

/// One registration with its addresses in their order.
#[derive(Clone, Debug)]
pub struct Registration {
  pub name: String,
  pub label: String,
  pub model: String,
  pub context_tokens: i64,
  pub answer_tokens: i64,
  pub wait_seconds: i64,
  pub thinking: bool,
  pub walked: bool,
  pub keeps_research: bool,
  pub addresses: Vec<Address>,
}

fn flag(row: &QueryResult, column: &str) -> Result<bool, sea_orm::DbErr> {
  Ok(row.try_get::<i64>("", column)? == 1)
}

/// Every registration in the order they were added, each with its addresses.
pub async fn registrations<C: ConnectionTrait>(db: &C) -> Result<Vec<Registration>, Refused> {
  let rows = all(db, "SELECT * FROM registered_models ORDER BY added_at, name", vec![]).await?;
  let addresses = all(
    db,
    "SELECT * FROM registered_model_addresses ORDER BY registration, position, node",
    vec![],
  )
  .await?;
  let mut read = vec![];

  for row in rows {
    let name: String = row.try_get("", "name")?;
    let mut held = vec![];

    for address in &addresses {
      if address.try_get::<String>("", "registration")? == name {
        held.push(Address {
          node: address.try_get("", "node")?,
          url: address.try_get("", "url")?,
          key_file: address.try_get("", "key_file")?,
          key_owned: flag(address, "key_owned")?,
        });
      }
    }

    read.push(Registration {
      label: row.try_get("", "label")?,
      model: row.try_get("", "model")?,
      context_tokens: row.try_get("", "context_tokens")?,
      answer_tokens: row.try_get("", "answer_tokens")?,
      wait_seconds: row.try_get("", "wait_seconds")?,
      thinking: flag(&row, "thinking")?,
      walked: flag(&row, "walked")?,
      keeps_research: flag(&row, "keeps_research")?,
      addresses: held,
      name,
    });
  }

  Ok(read)
}

/// One registration, or none.
pub async fn registration<C: ConnectionTrait>(db: &C, name: &str) -> Result<Option<Registration>, Refused> {
  Ok(registrations(db).await?.into_iter().find(|one| one.name == name))
}

/// The health of the engine behind a registration, asked at its addresses, or none where none
/// answers or the registry holds no such registration.
pub async fn registration_health<C: ConnectionTrait>(
  db: &C,
  data_root: &Path,
  name: &str,
) -> Result<Option<EngineHealth>, Refused> {
  let Some(held) = registration(db, name).await? else {
    return Ok(None);
  };
  let addresses: Vec<engine_health::Address> = held
    .addresses
    .into_iter()
    .map(|address| engine_health::Address {
      url: address.url,
      key_file: address.key_file,
    })
    .collect();

  Ok(engine_health::health(data_root, &addresses).await)
}

/// A registration as the service a chat with no tool asks directly: the gateway, under the
/// registration's name, sized to its window. None where the registry holds no such registration with
/// an address.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnLlmService {
  pub name: &'static str,
  pub label: String,
  pub short_label: String,
  pub base_url: String,
  pub key_file: String,
  pub registration: String,
}

pub async fn service_of<C: ConnectionTrait>(db: &C, model: &str) -> Result<Option<OwnLlmService>, Refused> {
  Ok(
    registration(db, model)
      .await?
      .filter(|held| !held.addresses.is_empty())
      .map(|held| OwnLlmService {
        name: PROVIDER,
        short_label: held.label.clone(),
        label: held.label,
        base_url: gateway::url(std::env::var("BMO_LLM_GATEWAY_URL").ok().as_deref()),
        key_file: REGISTERED.presence.key_file.clone(),
        registration: held.name,
      }),
  )
}

/// The answer ceiling of a key that is no gate, where its tier names none.
pub const DEFAULT_ANSWER_TOKENS: i64 = 4_096;

/// What one work key's tier mapping on an own service asks it for, with the registration's wait and
/// window where the settings were read for a run on it.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnLlmSettings {
  pub provider: &'static str,
  pub model: String,
  pub effort: Option<String>,
  pub thinking: bool,
  pub answer_tokens: i64,
  pub request_wait_ms: Option<i64>,
  pub context_tokens: Option<i64>,
}

impl OwnLlmSettings {
  /// The settings as a screen reads them, the wait and the window only where they were read.
  pub fn view(&self) -> Value {
    let mut view = json!({
      "provider": self.provider,
      "model": self.model,
      "effort": self.effort,
      "thinking": self.thinking,
      "answerTokens": self.answer_tokens,
    });

    if let Some(wait) = self.request_wait_ms {
      view["requestWaitMs"] = json!(wait);
    }

    if let Some(window) = self.context_tokens {
      view["contextTokens"] = json!(window);
    }

    view
  }
}

/// A tier mapping as an own service reads it: its model, effort and answer ceiling, each where named.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mapping {
  pub model: String,
  pub effort: Option<String>,
  pub answer_tokens: Option<i64>,
}

impl From<&super::tiers::Mapping> for Mapping {
  fn from(held: &super::tiers::Mapping) -> Self {
    Self {
      model: held.model.clone(),
      effort: held.effort.clone(),
      answer_tokens: held.answer_tokens,
    }
  }
}

/// The settings a work key asks an own service for: the mapping's model and thinking, and its answer
/// ceiling or else the work's own default. A mapping that cannot be read leaves the model empty.
pub fn own_llm_settings(service: &OwnLlmService, mapping: Option<&Mapping>) -> OwnLlmSettings {
  OwnLlmSettings {
    provider: service.name,
    model: mapping.map(|one| one.model.clone()).unwrap_or_default(),
    effort: mapping.and_then(|one| one.effort.clone()),
    thinking: mapping.and_then(|one| one.effort.as_deref()) == Some("thinking"),
    answer_tokens: mapping.and_then(|one| one.answer_tokens).unwrap_or(DEFAULT_ANSWER_TOKENS),
    request_wait_ms: None,
    context_tokens: None,
  }
}

/// Whether a run on a registration thinks: the mapping's word where it names one, the registration's
/// default otherwise.
fn thinks(registration: &Registration, effort: Option<&str>) -> bool {
  match effort {
    Some("thinking") => true,
    Some("no-thinking") => false,
    _ => registration.thinking,
  }
}

/// What a mapping of the registry asks a run for: its registration, thinking as the mapping names it
/// or else as the registration does, the mapping's answer ceiling or else the registration's, and the
/// registration's wait and window. A mapping naming a registration the registry does not hold asks for
/// it all the same, and the run is refused where it is launched.
pub async fn run_settings<C: ConnectionTrait>(
  db: &C,
  mapping: Option<&Mapping>,
) -> Result<OwnLlmSettings, Refused> {
  let held = match mapping {
    Some(mapping) => registration(db, &mapping.model).await?,
    None => None,
  };
  let effort = mapping.and_then(|one| one.effort.clone());

  Ok(OwnLlmSettings {
    provider: PROVIDER,
    model: mapping.map(|one| one.model.clone()).unwrap_or_default(),
    thinking: match &held {
      Some(held) => thinks(held, effort.as_deref()),
      None => effort.as_deref() == Some("thinking"),
    },
    effort,
    answer_tokens: mapping
      .and_then(|one| one.answer_tokens)
      .or(held.as_ref().map(|held| held.answer_tokens))
      .unwrap_or(DEFAULT_ANSWER_TOKENS),
    request_wait_ms: held.as_ref().map(|held| held.wait_seconds * 1_000),
    context_tokens: held.as_ref().map(|held| held.context_tokens),
  })
}

/// Whether the automatic walk of a key asks the registry: only where the registration the key's tier
/// maps to there is walked. A tier or a registration that cannot be read is no walk.
pub async fn walks(db: &sea_orm::DatabaseConnection, key: &str) -> bool {
  let Ok(mapping) = super::tiers::resolve(db, key, PROVIDER).await else {
    return false;
  };

  registration(db, &mapping.model)
    .await
    .is_ok_and(|held| held.is_some_and(|held| held.walked))
}

/// What names a registration, each thing that may name one answering for itself: the tiers whose
/// mapping on the registry names it, the workers and companions pinned to it, and the model
/// catalogue's entries served behind it.
pub async fn referrers<C: ConnectionTrait>(db: &C, name: &str) -> Result<Vec<String>, Refused> {
  let names = |rows: Vec<QueryResult>, column: &str| -> Result<Vec<String>, Refused> {
    rows
      .iter()
      .map(|row| row.try_get::<String>("", column).map_err(Refused::from))
      .collect()
  };
  let mapped = names(
    all(
      db,
      "SELECT tier FROM model_tier_mappings WHERE provider = $1 AND model = $2",
      vec![PROVIDER.into(), name.into()],
    )
    .await?,
    "tier",
  )?;
  let workers = names(
    all(
      db,
      "SELECT name FROM agents WHERE kind = 'worker' AND resting_since IS NULL AND pinned_provider = $1
       AND pinned_model = $2 ORDER BY name",
      vec![PROVIDER.into(), name.into()],
    )
    .await?,
    "name",
  )?;
  let conversational = crate::conversational::agents_where(db, |settings| settings.companion()).await?;
  let companions: Vec<String> = names(
    all(
      db,
      "SELECT name FROM agents WHERE pinned_provider = $1 AND pinned_model = $2 ORDER BY name",
      vec![PROVIDER.into(), name.into()],
    )
    .await?,
    "name",
  )?
  .into_iter()
  .filter(|pinned| conversational.contains(pinned))
  .collect();
  let entries = names(
    all(
      db,
      "SELECT name FROM model_catalogue WHERE registration = $1 ORDER BY added_at, name",
      vec![name.into()],
    )
    .await?,
    "name",
  )?;

  Ok(
    TIERS
      .ladder
      .iter()
      .filter(|tier| mapped.contains(tier))
      .map(|tier| fill_marks(&CONTROL_WORDS.registry.tier, &[("tier", tier)]))
      .chain(
        workers
          .iter()
          .map(|worker| fill_marks(&CONTROL_WORDS.registry.worker_pin, &[("worker", worker)])),
      )
      .chain(
        companions
          .iter()
          .map(|companion| fill_marks(&CONTROL_WORDS.registry.companion_pin, &[("companion", companion)])),
      )
      .chain(
        entries
          .iter()
          .map(|entry| fill_marks(&CONTROL_WORDS.registry.model_file, &[("entry", entry)])),
      )
      .collect(),
  )
}

/// Every registration as the screen shows it, each address's key said in one word, and the gateway as
/// its launcher last wrote of itself.
pub async fn view<C: ConnectionTrait>(db: &C, data_root: &Path) -> Result<Value, Refused> {
  let mut shown = vec![];

  for one in registrations(db).await? {
    let addresses: Vec<Value> = one
      .addresses
      .iter()
      .map(|address| {
        json!({
          "node": address.node,
          "url": address.url,
          "key": keys::state(data_root, address.key_file.as_deref()),
        })
      })
      .collect();

    shown.push(json!({
      "name": one.name,
      "label": one.label,
      "model": one.model,
      "contextTokens": one.context_tokens,
      "answerTokens": one.answer_tokens,
      "waitSeconds": one.wait_seconds,
      "thinking": one.thinking,
      "walked": one.walked,
      "keepsResearch": one.keeps_research,
      "addresses": addresses,
      "referrers": referrers(db, &one.name).await?,
    }));
  }

  Ok(json!({ "registrations": shown, "gateway": gateway::status(data_root) }))
}

// A text the body gives, trimmed, or an empty one.
fn text(body: &Value, key: &str) -> String {
  body.get(key).and_then(Value::as_str).map(str::trim).unwrap_or_default().to_owned()
}

// A field the body names: left out and null both fall through to what stands.
fn given<'a>(body: &'a Value, key: &str) -> Option<&'a Value> {
  body.get(key).filter(|value| !value.is_null())
}

fn checked_name(name: &str) -> Result<(), Fault> {
  if NAME.is_match(name) {
    return Ok(());
  }

  refused("name", CONTROL_WORDS.registry.name.as_str())
}

fn checked_node(node: &str) -> Result<(), Fault> {
  if NAME.is_match(node) {
    return Ok(());
  }

  refused("node", CONTROL_WORDS.registry.node.as_str())
}

/// The address as the URL parser reads it: an http or https address whose path ends in the chat
/// completions path, with no query, fragment or credentials.
pub fn checked_url(address: &str) -> Result<String, Fault> {
  let Ok(parsed) = url::Url::parse(address) else {
    return refused("url", CONTROL_WORDS.registry.url_unread.as_str());
  };

  if !matches!(parsed.scheme(), "http" | "https")
    || !parsed.path().ends_with("/chat/completions")
    || parsed.query().is_some_and(|query| !query.is_empty())
    || parsed.fragment().is_some_and(|fragment| !fragment.is_empty())
    || !parsed.username().is_empty()
    || parsed.password().is_some()
  {
    return refused("url", CONTROL_WORDS.registry.url_shape.as_str());
  }

  Ok(parsed.to_string())
}

pub fn checked_key_file(key_file: Option<&str>) -> Result<(), Fault> {
  match key_file {
    Some(file) if !KEY_FILE.is_match(file) || file.split('/').any(|part| part == "." || part == "..") => {
      refused("key", CONTROL_WORDS.registry.key_file.as_str())
    }
    _ => Ok(()),
  }
}

// A whole number in its bounds, however it was written: a number with nothing after its point counts.
fn whole(field: &str, value: Option<&Value>, bounds: &WholeBounds, label: &str) -> Result<i64, Fault> {
  let held = value.and_then(Value::as_number).and_then(|number| {
    number
      .as_i64()
      .or_else(|| number.as_f64().filter(|float| float.fract() == 0.0).map(|float| float as i64))
  });

  match held {
    Some(held) if bounds.holds(held) => Ok(held),
    _ => refused(
      field,
      fill_marks(
        &CONTROL_WORDS.registry.whole,
        &[
          ("label", label),
          ("min", &grouped(bounds.min as usize)),
          ("max", &grouped(bounds.max as usize)),
        ],
      ),
    ),
  }
}

/// Every field a registration holds apart from its name and its addresses, checked together, since
/// the answer size is held to the window.
struct Fields {
  label: String,
  model: String,
  context_tokens: i64,
  answer_tokens: i64,
  wait_seconds: i64,
  thinking: bool,
  walked: bool,
  keeps_research: bool,
}

impl Fields {
  fn checked(fields: &Value) -> Result<Self, Fault> {
    let label = match fields.get("label").and_then(Value::as_str) {
      Some(label) if !label.trim().is_empty() && length(label) <= REGISTRY.label_most => {
        label.trim().to_owned()
      }
      _ => {
        return refused(
          "label",
          fill_marks(&CONTROL_WORDS.registry.label, &[("most", &REGISTRY.label_most.to_string())]),
        );
      }
    };
    let model = match fields.get("model").and_then(Value::as_str) {
      Some(model) if MODEL.is_match(model) => model.to_owned(),
      _ => return refused("model", CONTROL_WORDS.registry.model.as_str()),
    };
    let context_tokens = whole(
      "contextTokens",
      fields.get("contextTokens"),
      &REGISTRY.context_tokens,
      &CONTROL_WORDS.registry.context_label,
    )?;
    let answer_tokens = whole(
      "answerTokens",
      fields.get("answerTokens"),
      &REGISTRY.answer_tokens,
      &CONTROL_WORDS.registry.answer_label,
    )?;
    let wait_seconds = whole(
      "waitSeconds",
      fields.get("waitSeconds"),
      &REGISTRY.wait_seconds,
      &CONTROL_WORDS.registry.wait_label,
    )?;

    if answer_tokens >= context_tokens {
      return refused("answerTokens", CONTROL_WORDS.registry.answer_under_window.as_str());
    }

    let (Some(thinking), Some(walked)) = (
      fields.get("thinking").and_then(Value::as_bool),
      fields.get("walked").and_then(Value::as_bool),
    ) else {
      return refused("thinking", CONTROL_WORDS.registry.switches.as_str());
    };
    let Some(keeps_research) = fields.get("keepsResearch").and_then(Value::as_bool) else {
      return refused("keepsResearch", CONTROL_WORDS.registry.keeps_research.as_str());
    };

    Ok(Self {
      label,
      model,
      context_tokens,
      answer_tokens,
      wait_seconds,
      thinking,
      walked,
      keeps_research,
    })
  }

  fn values(&self) -> Vec<sea_orm::Value> {
    vec![
      self.label.clone().into(),
      self.model.clone().into(),
      self.context_tokens.into(),
      self.answer_tokens.into(),
      self.wait_seconds.into(),
      i64::from(self.thinking).into(),
      i64::from(self.walked).into(),
      i64::from(self.keeps_research).into(),
    ]
  }
}

/// One change of the registry, made in one transaction; the key files it leaves behind are taken away
/// once it has committed.
struct Change<'a, C> {
  db: &'a C,
  data_root: &'a Path,
  // The key files agency wrote that the change leaves no address naming.
  gone: Vec<Address>,
}

impl<C: ConnectionTrait> Change<'_, C> {
  async fn required(&self, name: &str) -> Result<Registration, Fault> {
    match registration(self.db, name).await? {
      Some(held) => Ok(held),
      None => refused("name", fill_marks(&CONTROL_WORDS.registry.unknown, &[("name", name)])),
    }
  }

  async fn save(&mut self, body: &Value, name: &str) -> Result<(), Fault> {
    let held = registration(self.db, name).await?;
    let stands = |of: fn(&Registration) -> Value| held.as_ref().map(of).unwrap_or(Value::Null);
    let or_held =
      |key: &str, of: fn(&Registration) -> Value| given(body, key).cloned().unwrap_or_else(|| stands(of));
    let fields = json!({
      // A new registration shown under its name until it is given a label.
      "label": body.get("label").filter(|label| label.is_string()).cloned()
        .unwrap_or_else(|| held.as_ref().map_or_else(|| json!(name), |held| json!(held.label))),
      "model": body.get("model").and_then(Value::as_str).map(|model| json!(model.trim())).unwrap_or_else(|| stands(|held| json!(held.model))),
      "contextTokens": or_held("contextTokens", |held| json!(held.context_tokens)),
      "answerTokens": or_held("answerTokens", |held| json!(held.answer_tokens)),
      "waitSeconds": or_held("waitSeconds", |held| json!(held.wait_seconds)),
      "thinking": given(body, "thinking").cloned().unwrap_or_else(|| json!(held.as_ref().is_some_and(|held| held.thinking))),
      "walked": given(body, "walked").cloned().unwrap_or_else(|| json!(held.as_ref().is_some_and(|held| held.walked))),
      "keepsResearch": given(body, "keepsResearch").cloned()
        .unwrap_or_else(|| json!(held.as_ref().is_some_and(|held| held.keeps_research))),
    });

    if held.is_none() {
      checked_name(name)?;

      let fields = Fields::checked(&fields)?;
      let mut values = vec![name.into()];

      values.extend(fields.values());
      values.push((now_ms() / 1000).into());
      run(
        self.db,
        "INSERT INTO registered_models (name, label, model, context_tokens, answer_tokens, wait_seconds, thinking,
         walked, keeps_research, added_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        values,
      )
      .await?;
    } else {
      let mut values = Fields::checked(&fields)?.values();

      values.push(name.into());
      run(
        self.db,
        "UPDATE registered_models SET label = $1, model = $2, context_tokens = $3, answer_tokens = $4,
         wait_seconds = $5, thinking = $6, walked = $7, keeps_research = $8 WHERE name = $9",
        values,
      )
      .await?;
    }

    Ok(())
  }

  // An address added, or the one of the same node changed in place.
  async fn set_address(
    &self,
    name: &str,
    node: &str,
    url: &str,
    key_file: Option<&str>,
    key_owned: bool,
  ) -> Result<(), Fault> {
    self.required(name).await?;
    checked_node(node)?;

    let url = checked_url(url)?;

    checked_key_file(key_file)?;

    let last = one(
      self.db,
      "SELECT MAX(position) AS position FROM registered_model_addresses WHERE registration = $1",
      vec![name.into()],
    )
    .await?
    .map(|row| row.try_get::<Option<i64>>("", "position"))
    .transpose()?
    .flatten();

    run(
      self.db,
      "INSERT INTO registered_model_addresses (registration, node, position, url, key_file, key_owned)
       VALUES ($1, $2, $3, $4, $5, $6)
       ON CONFLICT (registration, node) DO UPDATE SET url = excluded.url, key_file = excluded.key_file,
       key_owned = excluded.key_owned",
      vec![
        name.into(),
        node.into(),
        (last.unwrap_or(0) + 1).into(),
        url.into(),
        key_file.map(str::to_owned).into(),
        i64::from(key_file.is_some() && key_owned).into(),
      ],
    )
    .await?;

    Ok(())
  }

  async fn address(&mut self, body: &Value, name: &str) -> Result<(), Fault> {
    let held = self.required(name).await?;
    let node = text(body, "node");
    let url = text(body, "url");
    let standing = held.addresses.iter().find(|one| one.node == node).cloned();
    let key = body.get("key");

    if key.is_some_and(|key| !key.is_null() && !key.is_string()) {
      return refused("key", CONTROL_WORDS.registry.key_text.as_str());
    }

    // The address is checked before a key file is written, so a refused address leaves no file
    // behind, and the row is written once.
    checked_node(&node)?;
    checked_url(&url)?;

    match key {
      Some(Value::String(key)) => {
        let key_file = keys::write(self.data_root, name, &node, key)?;

        self.set_address(name, &node, &url, Some(&key_file), true).await?;
        self
          .gone
          .extend(standing.filter(|standing| standing.key_file.as_deref() != Some(&key_file)));
      }
      Some(Value::Null) => {
        self.set_address(name, &node, &url, None, false).await?;
        self.gone.extend(standing);
      }
      _ => {
        let (key_file, owned) =
          standing.map_or((None, false), |standing| (standing.key_file, standing.key_owned));

        self.set_address(name, &node, &url, key_file.as_deref(), owned).await?;
      }
    }

    Ok(())
  }

  async fn refused_while_named(&self, name: &str, what: &str) -> Result<(), Fault> {
    let naming = referrers(self.db, name).await?;

    if naming.is_empty() {
      return Ok(());
    }

    refused(
      "name",
      fill_marks(&CONTROL_WORDS.registry.in_use, &[("what", what), ("names", &naming.join(", "))]),
    )
  }

  // One address taken out; the last address of a registration something names is refused, since the
  // registration would then be offered nowhere while it is named.
  async fn remove_address(&mut self, name: &str, node: &str) -> Result<(), Fault> {
    if registration(self.db, name).await?.is_some_and(|held| held.addresses.len() == 1) {
      self
        .refused_while_named(name, &fill_marks(&CONTROL_WORDS.registry.last_address, &[("name", name)]))
        .await?;
    }

    let held = self.required(name).await?;
    let Some(address) = held.addresses.into_iter().find(|one| one.node == node) else {
      return refused(
        "node",
        fill_marks(&CONTROL_WORDS.registry.no_address, &[("name", name), ("node", node)]),
      );
    };

    run(
      self.db,
      "DELETE FROM registered_model_addresses WHERE registration = $1 AND node = $2",
      vec![name.into(), node.into()],
    )
    .await?;
    self.gone.push(address);

    Ok(())
  }

  async fn remove(&mut self, name: &str) -> Result<(), Fault> {
    self
      .refused_while_named(name, &fill_marks(&CONTROL_WORDS.registry.the_registration, &[("name", name)]))
      .await?;

    let held = self.required(name).await?;

    run(
      self.db,
      "DELETE FROM registered_model_addresses WHERE registration = $1",
      vec![name.into()],
    )
    .await?;
    run(self.db, "DELETE FROM registered_models WHERE name = $1", vec![name.into()]).await?;
    self.gone.extend(held.addresses);

    Ok(())
  }
}

/// Every registration as a caller shows it.
pub struct RegistryView;

impl Route for RegistryView {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/registry";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    Ok(answer(StatusCode::OK, view(call.store(), &call.data_root()).await?))
  }
}

/// One change: a registration made or saved, an address set or removed, or a registration removed,
/// answered with the registry as it then stands.
pub struct RegistryChange;

impl Route for RegistryChange {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/registry";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;

    if !body.is_object() && !body.is_array() {
      return Err(Refused::new(StatusCode::BAD_REQUEST, CONTROL_WORDS.request.action.as_str()));
    }

    let data_root = call.data_root();
    let db = call.store();

    if let Err(fault) = made(db, &data_root, &body).await {
      return fault.answer();
    }

    Ok(answer(StatusCode::OK, view(db, &data_root).await?))
  }
}

// One change made in its transaction, and the key files it left nobody naming taken away once it has
// committed.
async fn made(db: &sea_orm::DatabaseConnection, data_root: &Path, body: &Value) -> Result<(), Fault> {
  let written = db.begin().await?;
  let mut change = Change {
    db: &written,
    data_root,
    gone: vec![],
  };
  let name = text(body, "name");

  match body.get("action").and_then(Value::as_str) {
    Some("save") => change.save(body, &name).await,
    Some("set-address") => change.address(body, &name).await,
    Some("remove-address") => change.remove_address(&name, &text(body, "node")).await,
    Some("remove") => change.remove(&name).await,
    _ => refused("action", CONTROL_WORDS.request.action.as_str()),
  }?;

  let gone = std::mem::take(&mut change.gone);

  written.commit().await?;

  for address in gone {
    keys::remove(data_root, address.key_file.as_deref(), address.key_owned);
  }

  Ok(())
}

/// One change of the registry as a route makes it, from a body of the same shape: done, or the
/// sentence refusing it.
pub async fn change(
  db: &sea_orm::DatabaseConnection,
  data_root: &Path,
  body: &Value,
) -> Result<Result<(), String>, Refused> {
  match made(db, data_root, body).await {
    Ok(()) => Ok(Ok(())),
    Err(Fault::Field(_, error)) => Ok(Err(error)),
    Err(Fault::Failed(failed)) => Err(failed),
  }
}
