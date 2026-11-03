//! The fallback's view (docs/api.md, 「providers, models and model files」): the Claude accounts, the account in
//! use and the chosen one, and the provider the supervisor stands on now with what it has used, its windows read
//! off agency's own keeping. What a caller may know of a
//! credential is the path it is reached by, and of the OpenRouter key whether there is one. A gate's own
//! options are its consumer's, kept beside its own judgement.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use cache_siren::{Keeping, KeptBalance, Slot};
use harness_providers::{ARCHIVE_PROVIDERS, Window, providers};
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::claude_account_changes::{self as changes, BasePromptGiven, CredentialGiven};
use super::claude_accounts::{
  self, Account, CredentialKind, EndpointModels, Judging, SEED_ACCOUNT, accounts, leading_integer, walk_for,
};
use super::harness::{missing, named};
use super::{Call, Refused, Route, answer, setting};

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

/// One slot's two headline windows, the session's and the week's, as the provider picks them out of every
/// window agency keeps under the slot; null where the slot holds no such window. A slot with no login is
/// read under the empty account. A login's slot answers only the windows its answering login was read
/// with: none seen at or before the account's unread mark, nor at or before `marked`, the moment the login
/// was marked as waiting for a person (docs/usage-windows.md, 「What reads the windows」). A keeping that
/// cannot be read is a fault, answered with its cause rather than read as a slot with no windows.
pub(super) async fn headline_windows(
  windows: &Keeping,
  name: &str,
  account: Option<&str>,
  marked: Option<i64>,
) -> Result<(Value, Value), Refused> {
  let provider = named(name)?;
  let reading = windows
    .read(Slot {
      mode: name,
      account: account.unwrap_or_default(),
    })
    .await
    .map_err(|fault| {
      eprintln!("{name} windows of {}: not read: {fault}", account.unwrap_or("no login"));
      Refused::from(fault)
    })?;
  let after = account.and(reading.unread_at).max(marked);
  let read: Vec<_> = reading
    .windows
    .into_iter()
    .filter(|kept| after.is_none_or(|after| kept.seen_at > after))
    .collect();
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

  headline_windows(windows, cache_siren::claude::MODE, Some(id), account.needs_login_at).await
}

// A balance as the routes answer it: what is left, its currency and when it was seen, or null.
fn balance_view(kept: Option<KeptBalance>) -> Value {
  kept.map_or(Value::Null, |kept| {
    json!({ "amount": kept.balance.amount, "currency": kept.balance.currency, "seenAt": kept.seen_at })
  })
}

// A keeping that cannot be read is a fault, said with its cause.
fn unread(what: &str) -> impl FnOnce(store::Fault) -> Refused + '_ {
  move |fault| {
    eprintln!("the balance of {what}: not read: {fault}");
    Refused::from(fault)
  }
}

/// An endpoint account's balance as its provider's reader last kept it (docs/usage-windows.md,
/// 「Balances」); null for a directory or token account, which is paid by its subscription, and where none
/// was read.
pub(super) async fn account_balance(windows: &Keeping, account: &Account) -> Result<Value, Refused> {
  if account.credential.window_credential().is_some() {
    return Ok(Value::Null);
  }

  Ok(balance_view(
    windows.balance_of_account(&account.id).await.map_err(unread(&account.id))?,
  ))
}

/// The supervisor whose turn went out last and what it has used: the provider its road answers on, the login
/// it walks to where the provider holds logins, that slot's two headline windows and its balance.
pub(super) async fn active_agent_usage(judging: Judging<'_>) -> Result<Value, Refused> {
  let db = judging.db;
  let latest = store::one(
    db,
    r#"SELECT "last_mode" FROM "supervisors" WHERE "last_mode" IS NOT NULL
       ORDER BY "last_run_at" DESC NULLS LAST, "name" LIMIT 1"#,
    vec![],
  )
  .await?
  .map(|row| row.try_get::<String>("", "last_mode"))
  .transpose()?;
  let name = match latest {
    Some(provider) => provider,
    None => fallback_provider(db).await?.unwrap_or_else(|| "claude-code".to_owned()),
  };
  let provider = named(&name)?;
  let account = if provider.holds_logins() { walk_for(judging).await?.into_iter().next() } else { None };
  let id = account.as_ref().map(|one| one.id.clone());
  let marked = account.as_ref().and_then(|one| one.needs_login_at);
  let (session, week) = headline_windows(judging.windows, &name, id.as_deref(), marked).await?;
  let balance = match &account {
    Some(account) => account_balance(judging.windows, account).await?,
    None => balance_view(
      judging
        .windows
        .balance(Slot {
          mode: &name,
          account: "",
        })
        .await
        .map_err(unread(&name))?,
    ),
  };
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
    "balance": balance,
  }))
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
          "code": provider.name(),
          "label": provider.declared().label,
          "shortLabel": provider.declared().label,
          "present": missing(db, machine, provider).await?.is_none(),
        }));
      }
    }

    let active = active_agent_usage(judging).await?;
    Ok(answer(
      StatusCode::OK,
      json!({
        "answering": !active.is_null(),
        "codexPresent": missing(db, machine, named("codex")?).await?.is_none(),
        "keyPresent": missing(db, machine, named("openrouter")?).await?.is_none(),
        "services": services,
        "since": fallback_since(db).await?,
        "active": active,
        "accounts": claude_accounts::views(judging).await?,
      }),
    ))
  }
}

crate::moved!(FallbackView);

fn bad(error: impl Into<String>) -> Refused {
  Refused::failed(error.into())
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

/// A change of the Claude accounts answered, or nothing where the action is none of them.
async fn own_change(judging: Judging<'_>, action: &str, body: &Value) -> Result<Option<Response>, Refused> {
  let db = judging.db;
  let text = |field: &str| body.get(field).and_then(Value::as_str);
  let switch = body.get("enabled").and_then(Value::as_bool);

  match action {
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
    let _db = call.store();
    let action = body.get("action").and_then(Value::as_str).unwrap_or_default();

    if let Some(answered) = own_change(Judging::of(call.ground()), action, &body).await? {
      return Ok(answered);
    }

    if RETIRED.contains(&action) {
      return Err(Refused::new(StatusCode::GONE, CONTROL_WORDS.fallback.models_retired.as_str()));
    }

    Err(bad(CONTROL_WORDS.fallback.unknown_action.as_str()))
  }
}

crate::moved!(FallbackChange);
