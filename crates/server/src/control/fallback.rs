//! The fallback settings (docs/cache-fallback.md): every work key with its road, its Claude login and the
//! roads it is offered; the provider order and the Claude accounts; and the agent serving the chat now
//! with what it has used, its windows read off agency's own keeping. What a caller may know of a
//! credential is the path it is reached by, and of the OpenRouter key whether there is one. A gate's own
//! options are its consumer's, kept beside its own judgement.

use std::collections::BTreeMap;

use axum::http::{Method, StatusCode};
use axum::response::Response;
use cache_siren::{Keeping, Slot};
use harness_providers::{ARCHIVE_PROVIDERS, Machine, Window, providers};
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::work_keys::{Profile, WORK_KEYS};
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::claude_account_changes::{self as changes, BasePromptGiven, CredentialGiven, account_named};
use super::claude_accounts::{
  self, Account, CredentialKind, EndpointModels, Judging, SEED_ACCOUNT, accounts, leading_integer, walk_for,
};
use super::harness::{
  OrderEntry, is_default_order, missing, named, order_shape_reason, order_to_save, provider_enabled,
  provider_order, set_provider_order,
};
use super::roads::{
  LAYA, OfferedRoad, is_route_mode, mode_refusal, offered_roads, pinned_provider, profile, route_row,
  set_account, set_mode,
};
use super::{Call, Refused, Route, answer, registry, setting, tiers};
use crate::now_ms;

// When the fallback began answering, absent while Claude is answering, and the stand-in it answers on.
const SINCE_KEY: &str = "fallback_since";
const PROVIDER_KEY: &str = "fallback_provider";

/// When the fallback began, or none while it is not standing.
async fn fallback_since(db: &DatabaseConnection) -> Result<Option<i64>, Refused> {
  Ok(setting(db, SINCE_KEY).await?.and_then(|value| leading_integer(&value)))
}

/// The stand-in answering while the fallback stands: the provider its setting names where that is one
/// other than Claude Code, OpenRouter where it names none.
pub async fn fallback_provider(db: &DatabaseConnection) -> Result<Option<String>, Refused> {
  if fallback_since(db).await?.is_none() {
    return Ok(None);
  }

  let named = setting(db, PROVIDER_KEY).await?;

  Ok(Some(match named {
    Some(name) if name != "claude-code" && ARCHIVE_PROVIDERS.contains(&name.as_str()) => name,
    _ => "openrouter".to_owned(),
  }))
}

/// The work key the deployment's chat runs under: the profile of its own kind.
pub fn chat_key() -> &'static str {
  WORK_KEYS
    .profile
    .iter()
    .find(|one| one.kind == "bmo")
    .map_or("bmo", |one| one.key.as_str())
}

/// One slot's two headline windows, the session's and the week's, as the provider picks them out of every
/// window agency keeps under the slot; null where the slot holds no such window. A slot with no login is
/// read under the empty account. A keeping that cannot be read is a fault, answered with its cause rather
/// than read as a slot with no windows.
pub(super) async fn headline_windows(
  windows: &Keeping,
  name: &str,
  account: Option<&str>,
) -> Result<(Value, Value), Refused> {
  let provider = named(name)?;
  let read = windows
    .read(Slot {
      mode: name,
      account: account.unwrap_or_default(),
    })
    .await
    .map_err(|fault| {
      eprintln!("{name} windows of {}: not read: {fault}", account.unwrap_or("no login"));
      Refused::from(fault)
    })?
    .windows;
  let own: Vec<Value> = read
    .iter()
    .map(|kept| {
      let window = &kept.window;

      json!({
        "name": window.name,
        "account": account,
        "mode": name,
        "utilization": window.utilization,
        "resetsAt": window.resets_at,
        "seenAt": kept.seen_at,
        "durationMinutes": window.duration_minutes,
        "limitName": window.limit_name,
      })
    })
    .collect();
  let read: Vec<Window> = own
    .iter()
    .map(|window| Window {
      name: window["name"].as_str().unwrap_or_default(),
      duration_minutes: window["durationMinutes"].as_i64(),
    })
    .collect();
  let headline = provider.headline(&read);
  let at = |place: Option<usize>| place.map_or(Value::Null, |place| own[place].clone());

  Ok((at(headline.session), at(headline.week)))
}

/// An account's own two headline windows, the session's and the week's, as the Claude account reader and
/// its runs keep them under it; null for an endpoint account, which holds no windows, and where the account
/// holds no such window.
pub(super) async fn account_headlines(
  windows: &Keeping,
  account: &Account,
) -> Result<(Value, Value), Refused> {
  if account.credential.window_credential().is_none() {
    return Ok((Value::Null, Value::Null));
  }

  let id = if account.id.is_empty() { SEED_ACCOUNT } else { account.id.as_str() };

  headline_windows(windows, cache_siren::claude::MODE, Some(id)).await
}

/// The agent serving the chat now and what it has used: the provider its road answers on, the login
/// it walks to where the provider holds logins, and that slot's two headline windows.
pub(super) async fn active_agent_usage(judging: Judging<'_>) -> Result<Value, Refused> {
  let db = judging.db;
  let key = chat_key();
  let name = match pinned_provider(db, key).await? {
    Some(pinned) => pinned,
    None => fallback_provider(db).await?.unwrap_or_else(|| "claude-code".to_owned()),
  };
  let provider = named(&name)?;
  let account = if provider.holds_logins() {
    walk_for(judging, key).await?.into_iter().next()
  } else {
    None
  };
  let id = account.as_ref().map(|one| one.id.clone());
  let (session, week) = headline_windows(judging.windows, &name, id.as_deref()).await?;
  let held = if provider.holds_logins() {
    accounts(db).await?.iter().filter(|one| one.enabled).count()
  } else {
    0
  };

  Ok(json!({
    "provider": name,
    "account": id,
    "accountName": account.map_or_else(|| provider.declared().label.clone(), |one| one.name),
    "accounts": held,
    "session": session,
    "week": week,
  }))
}

/// One work key as the screen reads it: its road and login, what its tier resolves to on each
/// provider or why it cannot, what it asks each own service for, the roads it is offered, and why it has
/// no road at all where it has none.
async fn profile_view(
  db: &DatabaseConnection,
  machine: &Machine,
  profile: &Profile,
) -> Result<Value, Refused> {
  let key = profile.key.as_str();
  let row = route_row(db, key).await?;
  let mut choices = BTreeMap::new();
  let mut errors = BTreeMap::new();

  for provider in ARCHIVE_PROVIDERS {
    match tiers::resolve(db, key, provider).await {
      Ok(mapping) => {
        choices.insert(provider, mapping);
      }
      Err(error) => {
        errors.insert(provider, error);
      }
    }
  }

  let mut services = Vec::new();

  for provider in providers() {
    if provider.unregistered().is_none() || !provider.declared().carries(&profile.line) {
      continue;
    }

    let mapping = choices.get(provider.name()).map(registry::Mapping::from);

    services.push(registry::run_settings(db, mapping.as_ref()).await?.view());
  }

  let route_mode = row
    .as_ref()
    .map_or_else(|| profile.default_route_mode.clone(), |row| row.route_mode.clone());
  let offered = offered_roads(db, machine, key).await?;
  let available = if route_mode == LAYA {
    offered.iter().any(|one| one.name == LAYA && one.reason.is_none())
  } else {
    let mut any = false;

    for one in &offered {
      if one.name != LAYA
        && one.reason.is_none()
        && !errors.contains_key(one.name.as_str())
        && provider_enabled(db, &one.name).await?
      {
        any = true;
        break;
      }
    }

    any
  };
  let no_route = if route_mode == LAYA {
    &CONTROL_WORDS.fallback.no_laya
  } else {
    &CONTROL_WORDS.fallback.no_provider
  };
  let model = |provider: &str| choices.get(provider).map(|one| one.model.clone());
  let codex = choices.get("codex");
  let view = json!({
    "key": key,
    "label": profile.label,
    "kind": profile.kind,
    "line": profile.line,
    "configurationErrors": errors,
    "claudeModel": model("claude-code").unwrap_or_default(),
    "services": services,
    "codexModel": model("codex").unwrap_or_default(),
    "codexEffort": codex.and_then(|one| one.effort.clone()).unwrap_or_default(),
    // The retained fields describe the shared choice, never a competing default.
    "defaultCodexModel": model("codex").unwrap_or_default(),
    "defaultCodexEffort": codex.and_then(|one| one.effort.clone()).unwrap_or_default(),
    "codexOverridden": false,
    "openrouterModel": model("openrouter"),
    "routeMode": route_mode,
    "account": row.as_ref().and_then(|row| row.account.clone()),
    "setAt": row.as_ref().map(|row| row.set_at),
    "offered": offered.iter().map(OfferedRoad::view).collect::<Vec<_>>(),
    "noRoute": (!available).then_some(no_route),
  });

  Ok(view)
}

/// The order as the screen shows it, each provider with its own word on whether it can answer here.
async fn order_view(db: &DatabaseConnection, machine: &Machine) -> Result<Vec<Value>, Refused> {
  let mut views = Vec::new();

  for one in provider_order(db).await? {
    let provider = named(&one.name)?;

    views.push(json!({
      "name": one.name,
      "enabled": one.enabled,
      "label": provider.declared().label,
      "reason": missing(db, machine, provider).await?,
    }));
  }

  Ok(views)
}

/// Everything the fallback settings show.
pub struct FallbackView;

impl Route for FallbackView {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/fallback";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let db = call.store();
    let judging = Judging::of(call.ground());
    let machine = call.machine();
    let mut services = Vec::new();

    // Each provider whose models answer a request with no tools, and whether it can answer here.
    for provider in providers() {
      if provider.unregistered().is_some() {
        services.push(json!({
          "name": provider.name(),
          "label": provider.declared().label,
          "shortLabel": provider.declared().label,
          "present": missing(db, machine, provider).await?.is_none(),
        }));
      }
    }

    let mut profiles = Vec::new();

    for profile in &WORK_KEYS.profile {
      profiles.push(profile_view(db, machine, profile).await?);
    }

    let active = active_agent_usage(judging).await?;
    Ok(answer(
      StatusCode::OK,
      json!({
        "routes": order_view(db, machine).await?,
        "answering": !active.is_null(),
        "codexPresent": missing(db, machine, named("codex")?).await?.is_none(),
        "keyPresent": missing(db, machine, named("openrouter")?).await?.is_none(),
        "services": services,
        "since": fallback_since(db).await?,
        "active": active,
        "order": order_view(db, machine).await?,
        "orderIsDefault": is_default_order(&provider_order(db).await?),
        "accounts": claude_accounts::views(judging).await?,
        "profiles": profiles,
      }),
    ))
  }
}

crate::moved!(FallbackView);

fn bad(error: impl Into<String>) -> Refused {
  Refused::failed(error.into())
}

async fn order_answer(db: &DatabaseConnection) -> Result<Response, Refused> {
  Ok(answer(StatusCode::OK, json!({ "ok": true, "order": provider_order(db).await? })))
}

async fn accounts_answer(judging: Judging<'_>) -> Result<Response, Refused> {
  Ok(answer(
    StatusCode::OK,
    json!({ "ok": true, "accounts": claude_accounts::views(judging).await? }),
  ))
}

/// What of an account's credential a change sends, whether the account is added or changed, or the
/// refusal: a kind is one the credential kinds name, and a path, an endpoint and a key file are text. The
/// model map is an object from a family word to the endpoint's model id, read as the row keeps it.
pub fn credential_given(body: &Value) -> Result<CredentialGiven, Refused> {
  let kind = match body.get("kind") {
    None => None,
    Some(kind) => Some(kind.as_str().and_then(CredentialKind::named).ok_or_else(|| {
      bad(fill_marks(
        &CONTROL_WORDS.accounts.credential_kind,
        &[("kinds", &CredentialKind::words())],
      ))
    })?),
  };
  let text = |field: &str, shape: &str| -> Result<Option<String>, Refused> {
    match body.get(field) {
      None | Some(Value::Null) => Ok(None),
      Some(Value::String(text)) => Ok(Some(text.clone())),
      Some(_) => Err(bad(shape)),
    }
  };
  let path = text("path", &CONTROL_WORDS.accounts.credential_path)?;
  let endpoint = text("endpoint", &CONTROL_WORDS.accounts.endpoint_url)?;
  let key_file = text("keyFile", &CONTROL_WORDS.accounts.key_file_needed)?;
  let models = match body.get("models") {
    None | Some(Value::Null) => None,
    Some(models) => Some(
      serde_json::from_value::<EndpointModels>(models.clone())
        .map_err(|_| bad(CONTROL_WORDS.accounts.models_shape.as_str()))?,
    ),
  };

  Ok(CredentialGiven {
    kind,
    path,
    endpoint,
    key_file,
    models,
  })
}

/// The base prompt a change sends, whether the account is added or changed: a prompt's name names it,
/// null or `none` lets the account's base go, and a body naming none keeps the base the account holds.
pub fn base_prompt_given(body: &Value) -> Result<BasePromptGiven, Refused> {
  match body.get("basePrompt") {
    None => Ok(BasePromptGiven::Keep),
    Some(Value::Null) => Ok(BasePromptGiven::None),
    Some(Value::String(name)) if name.trim() == "none" => Ok(BasePromptGiven::None),
    Some(Value::String(name)) => Ok(BasePromptGiven::Named(name.clone())),
    Some(_) => Err(bad(CONTROL_WORDS.accounts.base_prompt_shape.as_str())),
  }
}

/// A change that stands on its own — the provider order or the Claude accounts — answered, or nothing
/// where the action is none of them.
async fn own_change(judging: Judging<'_>, action: &str, body: &Value) -> Result<Option<Response>, Refused> {
  let db = judging.db;
  let text = |field: &str| body.get(field).and_then(Value::as_str);
  let switch = body.get("enabled").and_then(Value::as_bool);

  match action {
    "set-provider-order" => {
      let held = provider_order(db).await?;
      let order = order_to_save(body.get("order").unwrap_or(&Value::Null), &held)
        .ok_or_else(|| bad(order_shape_reason()))?;

      set_provider_order(db, &order).await?;
    }
    "reset-provider-order" => {
      let order: Vec<OrderEntry> = ARCHIVE_PROVIDERS
        .iter()
        .map(|name| OrderEntry {
          name: (*name).into(),
          enabled: true,
        })
        .collect();

      set_provider_order(db, &order).await?;
    }
    "set-provider-enabled" => {
      let (Some(name), Some(enabled)) =
        (text("provider").filter(|name| ARCHIVE_PROVIDERS.contains(name)), switch)
      else {
        return Err(bad(CONTROL_WORDS.fallback.provider_switch.as_str()));
      };
      let order: Vec<OrderEntry> = provider_order(db)
        .await?
        .into_iter()
        .map(|one| OrderEntry {
          enabled: if one.name == name { enabled } else { one.enabled },
          name: one.name,
        })
        .collect();

      set_provider_order(db, &order).await?;
    }
    "set-account-order" => {
      let held = accounts(db).await?;
      let order = changes::order_to_save(body.get("order").unwrap_or(&Value::Null), &held)
        .ok_or_else(|| bad(CONTROL_WORDS.accounts.order.as_str()))?;

      changes::set_order(db, &order).await?;

      return accounts_answer(judging).await.map(Some);
    }
    "set-account-enabled" => {
      let (Some(id), Some(enabled)) = (text("account"), switch) else {
        return Err(bad(CONTROL_WORDS.accounts.switch.as_str()));
      };

      changes::update(db, id, None, None, Some(enabled), BasePromptGiven::Keep).await?;

      return accounts_answer(judging).await.map(Some);
    }
    "choose-account" => {
      let id = text("account").ok_or_else(|| bad(CONTROL_WORDS.accounts.run_on.as_str()))?;

      changes::choose(db, Some(id)).await?;

      return accounts_answer(judging).await.map(Some);
    }
    "clear-account-choice" => {
      changes::choose(db, None).await?;

      return accounts_answer(judging).await.map(Some);
    }
    "add-account" => {
      let name = text("name").ok_or_else(|| bad(CONTROL_WORDS.accounts.name_needed.as_str()))?;

      let given = credential_given(body)?;

      if given.path.as_deref().is_none_or(|path| path.trim().is_empty()) {
        return Err(bad(CONTROL_WORDS.accounts.credential_path.as_str()));
      }

      changes::add(db, name, &given, base_prompt_given(body)?).await?;

      return accounts_answer(judging).await.map(Some);
    }
    "update-account" => {
      let given = credential_given(body)?;
      let id = text("account").ok_or_else(|| bad(CONTROL_WORDS.accounts.which_to_change.as_str()))?;

      // A change naming part of a credential keeps the rest of the one the account already holds, and
      // one naming no base prompt keeps the base it holds.
      changes::update(db, id, text("name"), Some(&given), None, base_prompt_given(body)?).await?;

      return accounts_answer(judging).await.map(Some);
    }
    "remove-account" => {
      let id = text("account").ok_or_else(|| bad(CONTROL_WORDS.accounts.which_to_remove.as_str()))?;

      changes::remove(db, id).await?;

      return accounts_answer(judging).await.map(Some);
    }
    _ => return Ok(None),
  }

  order_answer(db).await.map(Some)
}

/// A change to one work key's own row, answered, or nothing where the action is none of them.
async fn key_change(
  db: &DatabaseConnection,
  machine: &Machine,
  action: &str,
  key: &str,
  body: &Value,
) -> Result<Option<Response>, Refused> {
  let at = now_ms().div_euclid(1000);

  match action {
    "set-route-mode" => {
      let mode = body
        .get("mode")
        .and_then(Value::as_str)
        .filter(|mode| is_route_mode(mode))
        .ok_or_else(|| bad(CONTROL_WORDS.fallback.run_mode.as_str()))?;

      if let Some(refusal) = mode_refusal(db, machine, key, mode).await? {
        return Err(bad(refusal));
      }

      set_mode(db, key, mode, at).await?;

      Ok(Some(answer(StatusCode::OK, json!({ "ok": true, "key": key, "mode": mode }))))
    }
    // Which Claude login this key runs on: one account, or none to walk the order. An account the
    // owner turned off is refused, so the screen and the run never disagree.
    "set-route-account" => {
      let asked = body
        .get("account")
        .and_then(Value::as_str)
        .ok_or_else(|| bad(CONTROL_WORDS.accounts.choice.as_str()))?;

      if asked == "auto" || asked.is_empty() {
        set_account(db, key, None, at).await?;

        return Ok(Some(answer(StatusCode::OK, json!({ "ok": true, "key": key, "account": null }))));
      }

      let held = account_named(db, asked)
        .await?
        .ok_or_else(|| bad(fill_marks(&CONTROL_WORDS.accounts.unknown, &[("id", asked)])))?;

      if !held.enabled {
        return Err(bad(fill_marks(&CONTROL_WORDS.accounts.off, &[("name", &held.name)])));
      }

      set_account(db, key, Some(&held.id), at).await?;

      Ok(Some(answer(StatusCode::OK, json!({ "ok": true, "key": key, "account": held.id }))))
    }
    _ => Ok(None),
  }
}

// The per-provider model writes the tier settings took over, answered as gone.
const RETIRED: [&str; 6] = [
  "set-claude",
  "set-local-model",
  "set-codex",
  "reset-codex",
  "set-openrouter",
  "clear-openrouter",
];

/// A change the fallback screen makes, each action reading the fields it names.
pub struct FallbackChange;

impl Route for FallbackChange {
  const METHOD: Method = Method::PATCH;
  const PATH: &'static str = "/fallback";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let body = call.body().await?;
    let db = call.store();
    let action = body.get("action").and_then(Value::as_str).unwrap_or_default();

    if let Some(answered) = own_change(Judging::of(call.ground()), action, &body).await? {
      return Ok(answered);
    }

    let key = body.get("key").and_then(Value::as_str).map(str::trim).unwrap_or_default();

    if profile(key).is_none() {
      return Err(bad(fill_marks(&CONTROL_WORDS.fallback.unknown_key, &[("key", key)])));
    }

    if let Some(answered) = key_change(db, call.machine(), action, key, &body).await? {
      return Ok(answered);
    }

    if RETIRED.contains(&action) {
      return Err(Refused::new(StatusCode::GONE, CONTROL_WORDS.fallback.models_retired.as_str()));
    }

    Err(bad(CONTROL_WORDS.fallback.unknown_action.as_str()))
  }
}

crate::moved!(FallbackChange);
