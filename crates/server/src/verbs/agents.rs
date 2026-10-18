//! The agents the bot runs and their settings, as a shell reads and changes them (docs/agent-control.md):
//! the provider order and, inside its Claude Code entry, the order of the Claude accounts; the accounts
//! themselves; the registered models; a work key's road and login; the job workers the owner owns —
//! added, changed, set to rest, placed on a node, given a checkout on one, and removed; an agent's own
//! thinking, effort and gates; and the listing of every agent. Each change is the one the console makes,
//! refused in the same sentence.
//!
//! Called with none of the order and account flags, both orders and the registered models print above
//! whatever else the verb does. A refusal is said on the error stream and ends the verb with 1; a call
//! the verb cannot read ends it with 2.

use std::path::{Path, PathBuf};

use futures_util::future::BoxFuture;
use harness_providers::{ARCHIVE_PROVIDERS, Machine};
use policy::control_words::CONTROL_WORDS;
use policy::fill_marks;
use policy::verb_words::VERB_WORDS;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use serde_json::{Value, json};

use super::VerbError;
use super::work_node::{work_refusal, workplace};
use crate::control::Refused;
use crate::control::agent_switches::{agent_gates, set_reasoning, switch_gate};
use crate::control::agents::{Agent, configured};
use crate::control::claude_account_changes::{self as changes, Credential, account_named};
use crate::control::claude_accounts::{self, Judging, accounts};
use crate::control::harness::{OrderEntry, provider_order, set_provider_order};
use crate::control::registry::{self, registrations};
use crate::control::roads::{self, LAYA};
use crate::control::tiers;
use crate::now_ms;
use crate::places::Ground;
use crate::queue::worker_rules::{self, Adding};
use crate::queue::workers::{self, Changes, NewWorker, NodeCheckout, Worker, node_checkout_label};
use crate::queue::{paths, pins};
use crate::text::grouped;
use store::{all, one};

/// What the verb is asked: every flag it reads.
#[derive(clap::Parser, Debug, Default, Clone)]
pub struct AgentsArgs {
  #[arg(long)]
  pub provider_order: Option<String>,
  #[arg(long)]
  pub provider_on: Option<String>,
  #[arg(long)]
  pub provider_off: Option<String>,
  #[arg(long)]
  pub registered: bool,
  #[arg(long)]
  pub register: Option<String>,
  /// The registration's name, its node and its address.
  #[arg(long, num_args = 3, value_names = ["NAME", "NODE", "ADDRESS"])]
  pub register_address: Option<Vec<String>>,
  #[arg(long)]
  pub key_stdin: bool,
  #[arg(long)]
  pub no_key: bool,
  /// The registration's name, and the node of the one address to take away.
  #[arg(long, num_args = 1..=2, value_names = ["NAME", "NODE"])]
  pub unregister: Option<Vec<String>>,
  #[arg(long)]
  pub context: Option<String>,
  #[arg(long)]
  pub answer: Option<String>,
  #[arg(long)]
  pub wait: Option<String>,
  #[arg(long)]
  pub walked: Option<String>,
  #[arg(long)]
  pub research: Option<String>,
  #[arg(long)]
  pub account_order: Option<String>,
  #[arg(long)]
  pub account_on: Option<String>,
  #[arg(long)]
  pub account_off: Option<String>,
  #[arg(long)]
  pub account_reset: bool,
  #[arg(long)]
  pub account_use: Option<String>,
  #[arg(long)]
  pub account_add: Option<String>,
  #[arg(long)]
  pub account_set: Option<String>,
  #[arg(long)]
  pub account_remove: Option<String>,
  #[arg(long)]
  pub kind: Option<String>,
  #[arg(long)]
  pub name: Option<String>,
  #[arg(long)]
  pub route: Option<String>,
  #[arg(long)]
  pub account: Option<String>,
  #[arg(long)]
  pub provider: Option<String>,
  #[arg(long)]
  pub model: Option<String>,
  #[arg(long)]
  pub add: Option<String>,
  #[arg(long)]
  pub set: Option<String>,
  #[arg(long)]
  pub remove: Option<String>,
  #[arg(long, allow_hyphen_values = true)]
  pub label: Option<String>,
  #[arg(long, allow_hyphen_values = true)]
  pub path: Option<String>,
  #[arg(long, allow_hyphen_values = true)]
  pub preamble: Option<String>,
  #[arg(long)]
  pub preamble_file: Option<PathBuf>,
  #[arg(long, allow_hyphen_values = true)]
  pub dev_command: Option<String>,
  #[arg(long, allow_hyphen_values = true)]
  pub herdr_label: Option<String>,
  #[arg(long)]
  pub downgrade_heal: Option<String>,
  #[arg(long)]
  pub keepalive: Option<String>,
  #[arg(long)]
  pub support: Option<String>,
  #[arg(long)]
  pub support_keepalive: Option<String>,
  #[arg(long)]
  pub node: Option<String>,
  #[arg(long)]
  pub rest: Option<String>,
  #[arg(long)]
  pub node_checkout: Option<String>,
  #[arg(long)]
  pub drop_node_checkout: Option<String>,
  #[arg(long)]
  pub reasoning: Option<String>,
  #[arg(long)]
  pub thinking: Option<String>,
  #[arg(long)]
  pub effort: Option<String>,
  #[arg(long)]
  pub gates: Option<String>,
  /// The gate, and on or off.
  #[arg(long, num_args = 1..=2, value_names = ["GATE", "SWITCH"])]
  pub gate: Option<Vec<String>>,
  #[arg(long)]
  pub pause_keepalive: Option<String>,
}

/// What the verb printed, and where it ended otherwise than well: the sentence on the error stream and
/// the code.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Said {
  pub out: Vec<String>,
  pub ended: Option<(u8, String)>,
}

impl Said {
  fn line(&mut self, line: impl Into<String>) {
    self.out.push(line.into());
  }

  fn refused(mut self, why: impl Into<String>) -> Self {
    self.ended = Some((1, why.into()));
    self
  }

  fn misread(mut self, usage: impl Into<String>) -> Self {
    self.ended = Some((2, usage.into()));
    self
  }
}

/// What the verb does beyond the archive, held apart so a test stands in for it: a worker's workbench and
/// dev server brought to its row, the bot told the workers moved, which workers a job holds now, and a
/// key read from standing input.
pub trait AgentsHands: Send + Sync {
  fn settle<'a>(&'a self, worker: &'a str) -> BoxFuture<'a, ()>;
  fn tell_workers(&self) -> BoxFuture<'_, ()>;
  /// The workers with something at work, or none where nobody could say.
  fn running(&self) -> BoxFuture<'_, Option<Vec<String>>>;
  fn standing_input(&self) -> String;
}

/// The hands of the verb run from a shell, on the ground its process stands on.
pub struct ShellHands {
  ground: crate::places::Ground,
}

impl AgentsHands for ShellHands {
  fn settle<'a>(&'a self, worker: &'a str) -> BoxFuture<'a, ()> {
    Box::pin(crate::places::dev::settle(&self.ground, worker))
  }

  fn tell_workers(&self) -> BoxFuture<'_, ()> {
    Box::pin(super::tell_server("/workers/stand", json!({})))
  }

  fn running(&self) -> BoxFuture<'_, Option<Vec<String>>> {
    Box::pin(crate::control::sessions::running_workers(&self.ground))
  }

  fn standing_input(&self) -> String {
    std::io::read_to_string(std::io::stdin()).unwrap_or_default()
  }
}

/// Where the verb stands: agency's ground — its store, its node service, its events, the data root a
/// worker with no directory of its own is given one under and the source root the built-in worker works in
/// — the machine its providers are judged on, and its hands.
pub struct Stand<'a> {
  pub ground: &'a Ground,
  pub machine: &'a Machine,
  pub hands: &'a dyn AgentsHands,
  /// The home a path on a node is written from, `~` standing for it.
  pub home: &'a Path,
}

/// The verb run from a shell: what it prints on standard output first, then the refusal it ends with.
pub async fn run(data_root: &Path, args: AgentsArgs) -> Result<Said, VerbError> {
  let hands = ShellHands {
    ground: super::ground(data_root).await?,
  };
  let machine = Machine::of_process(data_root);
  let home = super::home();
  let stand = Stand {
    ground: &hands.ground,
    machine: &machine,
    hands: &hands,
    home: &home,
  };

  agents(&stand, &args).await
}

// Whether a directory is a git checkout.
fn is_checkout(path: &Path) -> bool {
  path.join(".git").exists()
}

fn failed(refused: Refused) -> VerbError {
  VerbError::Refused(refused.error)
}

fn archive_failed(refusal: crate::Refusal) -> VerbError {
  VerbError::Store(refusal)
}

fn now_seconds() -> i64 {
  now_ms().div_euclid(1000)
}

/// The verb itself: the first group of flags given, in the order below, decides what it does.
pub async fn agents(stand: &Stand<'_>, args: &AgentsArgs) -> Result<Said, VerbError> {
  if args.provider_order.is_some() || args.provider_on.is_some() || args.provider_off.is_some() {
    return provider_switches(&stand.ground.db, args).await;
  }

  if args.registered
    || args.register.is_some()
    || args.register_address.is_some()
    || args.unregister.is_some()
  {
    return registry_change(stand, args).await;
  }

  if args.account_order.is_some()
    || args.account_on.is_some()
    || args.account_off.is_some()
    || args.account_reset
    || args.account_use.is_some()
  {
    return account_order(Judging::of(stand.ground), args).await;
  }

  if args.account_add.is_some() || args.account_set.is_some() || args.account_remove.is_some() {
    return account_change(&stand.ground.db, args).await;
  }

  // Both orders stand above the listing, and neither is printed by a run that changed one of them.
  let mut said = Said {
    out: header(Judging::of(stand.ground)).await.map_err(failed)?,
    ended: None,
  };

  if let Some(key) = &args.route {
    return match &args.account {
      Some(account) => route_account(&stand.ground.db, said, key, account).await,
      None => route_mode(stand, said, key, args).await,
    };
  }

  if let Some(name) = &args.add {
    return add_worker(stand, said, name, args).await;
  }

  if let Some(base) = args.node_checkout.as_ref().or(args.drop_node_checkout.as_ref()) {
    return node_checkout(stand, said, base, args).await;
  }

  if let Some(name) = &args.set {
    return match &args.rest {
      Some(rest) => rest_worker(stand, said, name, rest).await,
      None => set_worker(stand, said, name, args).await,
    };
  }

  if let Some(name) = &args.reasoning {
    return reasoning(stand, said, name, args).await;
  }

  if let Some(name) = &args.gates {
    return gates(&stand.ground.db, said, name, args.gate.as_deref()).await;
  }

  if let Some(name) = &args.pause_keepalive {
    if !workers::pause_keepalive(&stand.ground.db, name).await.map_err(archive_failed)? {
      return Ok(said.refused(fill_marks(&VERB_WORDS.agents.no_worker, &[("worker", name)])));
    }

    said.line(format!("worker {name}: keepalive off until its next job"));

    return Ok(said);
  }

  if let Some(name) = &args.remove {
    return remove_worker(stand, said, name).await;
  }

  said.out.extend(listing(&stand.ground.db).await?);

  Ok(said)
}

// The provider order, set whole or one provider switched, printed as it then stands.
async fn provider_switches(db: &DatabaseConnection, args: &AgentsArgs) -> Result<Said, VerbError> {
  let said = Said::default();
  let names = build_providers().join(", ");

  if let Some(order) = &args.provider_order {
    let named: Vec<&str> = order.split(',').collect();
    let once = named.len() == ARCHIVE_PROVIDERS.len()
      && ARCHIVE_PROVIDERS
        .iter()
        .all(|provider| named.iter().filter(|one| *one == provider).count() == 1);

    if !once {
      return Ok(said.refused(fill_marks(&VERB_WORDS.agents.provider_order, &[("names", &names)])));
    }

    let held = provider_order(db).await.map_err(failed)?;
    let entries: Vec<OrderEntry> = named
      .iter()
      .map(|name| OrderEntry {
        name: (*name).to_owned(),
        enabled: held.iter().any(|one| one.name == *name && one.enabled),
      })
      .collect();

    if let Err(refused) = set_provider_order(db, &entries).await {
      return Ok(said.refused(refused.error));
    }
  }

  for (named, enabled) in [(&args.provider_on, true), (&args.provider_off, false)] {
    let Some(named) = named else { continue };

    if !ARCHIVE_PROVIDERS.contains(&named.as_str()) {
      return Ok(said.refused(fill_marks(&VERB_WORDS.agents.provider_unknown, &[("names", &names)])));
    }

    let entries: Vec<OrderEntry> = provider_order(db)
      .await
      .map_err(failed)?
      .into_iter()
      .map(|one| OrderEntry {
        enabled: if one.name == *named { enabled } else { one.enabled },
        name: one.name,
      })
      .collect();

    if let Err(refused) = set_provider_order(db, &entries).await {
      return Ok(said.refused(refused.error));
    }
  }

  let order = provider_order(db).await.map_err(failed)?;
  let mut said = said;

  said.line(serde_json::to_string(&order).unwrap_or_default());

  Ok(said)
}

// The providers this build carries, in the order the build declares them.
fn build_providers() -> Vec<&'static str> {
  harness_providers::providers().iter().map(|provider| provider.name()).collect()
}

// A switch given on the command line, read as on or off; any other word refused with the flag named.
fn on_off(value: Option<&str>, flag: &str) -> Result<Option<bool>, String> {
  match value {
    None => Ok(None),
    Some("on") => Ok(Some(true)),
    Some("off") => Ok(Some(false)),
    Some(_) => Err(fill_marks(&VERB_WORDS.agents.on_off, &[("flag", flag)])),
  }
}

/// A count given on the command line, its grouping commas let go; a word that reads as no number is
/// handed on as it stands, for the registry to refuse.
pub fn count(value: Option<&str>) -> Option<Value> {
  let value = value?;
  let digits = value.replace(',', "");

  Some(match digits.trim().parse::<f64>() {
    Ok(number) if number.fract() == 0.0 && number.abs() < 9e15 => json!(number as i64),
    Ok(number) if number.is_finite() => json!(number),
    _ => json!(value),
  })
}

// The registry's change, and the registry as it then stands.
async fn registry_change(stand: &Stand<'_>, args: &AgentsArgs) -> Result<Said, VerbError> {
  let mut said = Said::default();
  let body = if let Some(name) = &args.register {
    let mut body = json!({ "action": "save", "name": name });

    for (field, value) in [
      ("label", args.label.as_ref().map(|label| json!(label))),
      ("model", args.model.as_ref().map(|model| json!(model))),
      ("contextTokens", count(args.context.as_deref())),
      ("answerTokens", count(args.answer.as_deref())),
      ("waitSeconds", count(args.wait.as_deref())),
    ] {
      if let Some(value) = value {
        body[field] = value;
      }
    }

    for (field, value, flag) in [
      ("thinking", &args.thinking, "--thinking"),
      ("walked", &args.walked, "--walked"),
      ("keepsResearch", &args.research, "--research"),
    ] {
      match on_off(value.as_deref(), flag) {
        Ok(Some(on)) => body[field] = json!(on),
        Ok(None) => {}
        Err(why) => return Ok(said.refused(why)),
      }
    }

    Some(body)
  } else if let Some(address) = &args.register_address {
    let mut body =
      json!({ "action": "set-address", "name": address[0], "node": address[1], "url": address[2] });

    if args.key_stdin {
      body["key"] = json!(stand.hands.standing_input());
    } else if args.no_key {
      body["key"] = Value::Null;
    }

    Some(body)
  } else {
    args
      .unregister
      .as_ref()
      .map(|named| match named.get(1).filter(|node| !node.starts_with("--")) {
        Some(node) => json!({ "action": "remove-address", "name": named[0], "node": node }),
        None => json!({ "action": "remove", "name": named[0] }),
      })
  };

  if let Some(body) = body
    && let Err(why) = registry::change(&stand.ground.db, &stand.ground.roots.data, &body)
      .await
      .map_err(failed)?
  {
    return Ok(said.refused(why));
  }

  let view = registry::view(&stand.ground.db, &stand.ground.roots.data).await.map_err(failed)?;

  for one in view["registrations"].as_array().into_iter().flatten() {
    let number = |field: &str| grouped(one[field].as_u64().unwrap_or_default() as usize);
    let flag = |field: &str| one[field].as_bool().unwrap_or_default();

    said.line(format!(
      "{} ({}): {}, context {}, answer {}, wait {}s, thinking {}, {}, {}",
      text(&one["name"]),
      text(&one["label"]),
      text(&one["model"]),
      number("contextTokens"),
      number("answerTokens"),
      one["waitSeconds"],
      if flag("thinking") { "on" } else { "off" },
      if flag("walked") { "walked" } else { "not walked" },
      if flag("keepsResearch") { "keeps its research" } else { "research elsewhere" },
    ));

    for address in one["addresses"].as_array().into_iter().flatten() {
      let key = match address["key"].as_str() {
        Some("usable") => &VERB_WORDS.agents.key_usable,
        Some("none") => &VERB_WORDS.agents.key_none,
        _ => &VERB_WORDS.agents.key_unusable,
      };

      said.line(format!("  {} {} {key}", text(&address["node"]), text(&address["url"])));
    }

    let naming: Vec<String> = one["referrers"].as_array().into_iter().flatten().map(text).collect();

    if !naming.is_empty() {
      said.line(format!("  named by: {}", naming.join(", ")));
    }
  }

  let gateway = &view["gateway"];

  said.line(if gateway.is_null() {
    "gateway: no status written".to_owned()
  } else {
    let serving = if gateway["serving"].is_null() {
      "nothing".to_owned()
    } else {
      text(&gateway["serving"])
    };
    let starting = if gateway["starting"].is_null() {
      String::new()
    } else {
      format!(", starting {}", text(&gateway["starting"]))
    };
    let draining = gateway["draining"].as_f64().unwrap_or_default();
    let draining = if draining > 0.0 {
      format!(", {} draining", gateway["draining"])
    } else {
      String::new()
    };

    format!("gateway: serving {serving}{starting}{draining}")
  });

  Ok(said)
}

// A value as a template literal writes it: a string as it stands, anything else as its JSON.
fn text(value: &Value) -> String {
  match value {
    Value::String(text) => text.clone(),
    other => other.to_string(),
  }
}

/// One account as the order prints it.
#[derive(Serialize)]
struct OrderedAccount<'a> {
  id: &'a str,
  name: &'a str,
  enabled: bool,
}

// The account order put back, set, switched, or one chosen to run on; printed as it then stands with
// the account runs go out on now.
async fn account_order(judging: Judging<'_>, args: &AgentsArgs) -> Result<Said, VerbError> {
  let db = judging.db;
  let mut said = Said::default();
  let no_account = |named: &str| fill_marks(&VERB_WORDS.agents.no_account, &[("named", named)]);
  let changed: Result<Result<(), String>, Refused> = async {
    if args.account_reset {
      let order: Vec<(String, bool)> = all(db, "SELECT id FROM claude_accounts ORDER BY rowid", vec![])
        .await?
        .iter()
        .map(|row| row.try_get::<String>("", "id").map(|id| (id, true)))
        .collect::<Result<_, _>>()?;

      changes::set_order(db, &order).await?;
      changes::choose(db, None).await?;
    }

    if let Some(order) = &args.account_order {
      let mut ids = Vec::new();

      // Each part is whichever of the two the owner typed; the order itself is always by id.
      for named in order.split(',') {
        match account_named(db, named.trim()).await? {
          Some(account) => ids.push(json!(account.id)),
          None => return Ok(Err(no_account(named.trim()))),
        }
      }

      let held = accounts(db).await?;
      let Some(order) = changes::order_to_save(&json!(ids), &held) else {
        return Ok(Err(CONTROL_WORDS.accounts.order.clone()));
      };

      changes::set_order(db, &order).await?;
    }

    for (named, enabled) in [(&args.account_on, true), (&args.account_off, false)] {
      let Some(named) = named else { continue };
      let Some(account) = account_named(db, named).await? else {
        return Ok(Err(no_account(named)));
      };

      changes::update(db, &account.id, None, None, Some(enabled)).await?;
    }

    if let Some(asked) = &args.account_use {
      let id = if asked == "auto" {
        None
      } else {
        match account_named(db, asked).await? {
          Some(account) => Some(account.id),
          None => return Ok(Err(no_account(asked))),
        }
      };

      changes::choose(db, id.as_deref()).await?;
    }

    Ok(Ok(()))
  }
  .await;

  match changed {
    Ok(Err(why)) => return Ok(said.refused(why)),
    Err(refused) => return Ok(said.refused(refused.error)),
    Ok(Ok(())) => {}
  }

  let held = accounts(db).await.map_err(failed)?;
  let order: Vec<OrderedAccount> = held
    .iter()
    .map(|one| OrderedAccount {
      id: &one.id,
      name: &one.name,
      enabled: one.enabled,
    })
    .collect();

  said.line(serde_json::to_string(&order).unwrap_or_default());
  said.line(account_use_line(judging).await.map_err(failed)?);

  Ok(said)
}

const ACCOUNT_ADD_USAGE: &str =
  "usage: bmo-agency agent --account-add <name> --path <dir|file> [--kind directory|token]";
const ACCOUNT_SET_USAGE: &str = "usage: bmo-agency agent --account-set <account> [--name <name>] [--path <dir|file>] [--kind directory|token]";

// The line an account is printed in after a change: where its credential is reached, never the
// credential.
fn account_line(account: &claude_accounts::Account) -> String {
  format!(
    "account {} ({}) {} at {}",
    account.id, account.name, account.credential_kind, account.credential_path
  )
}

// Registering an account, renaming one, pointing it at another credential and removing one — the same
// things the console's own panel does, for a hand that has a shell and no browser.
async fn account_change(db: &DatabaseConnection, args: &AgentsArgs) -> Result<Said, VerbError> {
  let mut said = Said::default();
  let kind = args.kind.as_deref();

  if kind.is_some_and(|kind| kind != "directory" && kind != "token") {
    return Ok(said.misread(VERB_WORDS.agents.credential_kind.as_str()));
  }

  let no_account = |named: &str| fill_marks(&VERB_WORDS.agents.no_account, &[("named", named)]);
  let by_id = async |id: &str| -> Result<claude_accounts::Account, VerbError> {
    accounts(db)
      .await
      .map_err(failed)?
      .into_iter()
      .find(|one| one.id == id)
      .ok_or_else(|| VerbError::Refused(no_account(id)))
  };

  if let Some(name) = &args.account_add {
    let Some(path) = &args.path else {
      return Ok(said.misread(ACCOUNT_ADD_USAGE));
    };
    let credential = Credential {
      kind: kind.unwrap_or("directory").to_owned(),
      path: path.clone(),
    };

    if let Err(refused) = changes::add(db, name, &credential).await {
      return Ok(said.refused(refused.error));
    }

    let held = accounts(db).await.map_err(failed)?;
    let added = held.iter().rev().find(|one| one.name == name.trim());

    if let Some(added) = added {
      said.line(account_line(added));
    }
  }

  if let Some(named) = &args.account_set {
    let Some(held) = account_named(db, named).await.map_err(failed)? else {
      return Ok(said.refused(no_account(named)));
    };

    if args.name.is_none() && args.path.is_none() {
      return Ok(said.misread(ACCOUNT_SET_USAGE));
    }

    // A path with no --kind keeps the way the account already holds; the id never moves.
    let credential = args.path.as_ref().map(|path| Credential {
      kind: kind.map_or_else(|| held.credential_kind.clone(), str::to_owned),
      path: path.clone(),
    });

    if let Err(refused) =
      changes::update(db, &held.id, args.name.as_deref(), credential.as_ref(), None).await
    {
      return Ok(said.refused(refused.error));
    }

    said.line(account_line(&by_id(&held.id).await?));
  }

  if let Some(named) = &args.account_remove {
    let Some(held) = account_named(db, named).await.map_err(failed)? else {
      return Ok(said.refused(no_account(named)));
    };

    if let Err(refused) = changes::remove(db, &held.id).await {
      return Ok(said.refused(refused.error));
    }

    said.line(format!("account {} ({}) removed", held.id, held.name));
  }

  Ok(said)
}

// The four lines above everything but a change of an order: both orders, the registered models, and
// the account runs go out on now.
async fn header(judging: Judging<'_>) -> Result<Vec<String>, Refused> {
  let db = judging.db;
  let providers: Vec<String> = provider_order(db)
    .await?
    .iter()
    .map(|one| format!("{} ({})", one.name, if one.enabled { "on" } else { "off" }))
    .collect();
  let registered: Vec<String> = registrations(db)
    .await?
    .iter()
    .map(|one| {
      let addresses = match one.addresses.len() {
        1 => "1 address".to_owned(),
        many => format!("{many} addresses"),
      };

      format!("{} ({}, {addresses})", one.name, grouped(one.context_tokens.max(0) as usize))
    })
    .collect();
  let held = accounts(db).await?;
  let as_added: Vec<String> = all(db, "SELECT id FROM claude_accounts ORDER BY rowid", vec![])
    .await?
    .iter()
    .map(|row| row.try_get::<String>("", "id"))
    .collect::<Result<_, _>>()?;
  let default = held.iter().zip(&as_added).all(|(one, id)| one.enabled && &one.id == id);
  let order: Vec<String> = held
    .iter()
    .map(|one| format!("{} ({}, {})", one.id, one.name, if one.enabled { "on" } else { "off" }))
    .collect();

  Ok(vec![
    format!("provider order: {}", providers.join(" → ")),
    format!(
      "registered models: {}",
      if registered.is_empty() { "none".to_owned() } else { registered.join(", ") }
    ),
    format!("account order: {}{}", order.join(" → "), if default { " [default]" } else { "" }),
    account_use_line(judging).await?,
  ])
}

// Which account runs go out on now, and the one chosen to run on where there is one. Where the two
// differ the chosen one is resting or its login is gone, or the login in use is still being spent to its
// end, and the line says which.
async fn account_use_line(judging: Judging<'_>) -> Result<String, Refused> {
  let views = claude_accounts::views(judging).await?;
  let flag = |view: &&Value, field: &str| view[field].as_bool().unwrap_or_default();
  let serving = views.iter().find(|view| flag(view, "serving"));
  let chosen = views.iter().find(|view| flag(view, "chosen"));
  let using = match serving {
    None => "account in use: none".to_owned(),
    Some(serving) => format!("account in use: {} ({})", text(&serving["id"]), text(&serving["name"])),
  };
  let Some(chosen) = chosen else {
    return Ok(format!("{using}; chosen: none, walks the account order"));
  };

  if flag(&chosen, "serving") {
    return Ok(format!("{using}; chosen: {}", text(&chosen["id"])));
  }

  let why = if let Some(until) = chosen["restUntil"].as_i64() {
    let at = chrono::DateTime::from_timestamp(until, 0)
      .map(|at| at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string())
      .unwrap_or_default();

    format!("resting at its limit until {at}")
  } else if chosen["login"] == "gone" {
    "its login is gone".to_owned()
  } else {
    match serving {
      None => "not answering".to_owned(),
      Some(serving) => format!("next once {} is spent", text(&serving["id"])),
    }
  };

  Ok(format!("{using}; chosen: {} ({}), {why}", text(&chosen["id"]), text(&chosen["name"])))
}

// Which Claude login a key's runs go out on: one account, or `auto` to walk the account order again.
async fn route_account(
  db: &DatabaseConnection,
  mut said: Said,
  key: &str,
  asked: &str,
) -> Result<Said, VerbError> {
  if roads::profile(key).is_none() {
    return Ok(said.misread("usage: bmo-agency agent --route <key> --account <auto|account>"));
  }

  if asked == "auto" {
    roads::set_account(db, key, None, now_seconds()).await.map_err(failed)?;
    said.line(format!("route {key}: walks the account order"));

    return Ok(said);
  }

  let Some(named) = account_named(db, asked).await.map_err(failed)? else {
    return Ok(said.refused(fill_marks(&VERB_WORDS.agents.no_account, &[("named", asked)])));
  };

  roads::set_account(db, key, Some(&named.id), now_seconds()).await.map_err(failed)?;
  said.line(format!("route {key}: runs on {} ({})", named.id, named.name));

  Ok(said)
}

// A key's road set, judged by what the providers offer it as the console's choice is.
async fn route_mode(
  stand: &Stand<'_>,
  mut said: Said,
  key: &str,
  args: &AgentsArgs,
) -> Result<Said, VerbError> {
  if args.model.is_some() {
    return Ok(said.misread("Per-key model writes are retired; use shared model-tier settings."));
  }

  let mode = args.provider.as_deref().unwrap_or_default();

  if roads::profile(key).is_none() || !roads::is_route_mode(mode) {
    let modes: Vec<&str> = std::iter::once(roads::AUTO).chain(build_providers()).chain([LAYA]).collect();

    return Ok(
      said.misread(format!("usage: bmo-agency agent --route <key> --provider <{}>", modes.join("|"))),
    );
  }

  if let Some(refusal) =
    roads::mode_refusal(&stand.ground.db, stand.machine, key, mode).await.map_err(failed)?
  {
    return Ok(said.refused(refusal));
  }

  roads::set_mode(&stand.ground.db, key, mode, now_seconds()).await.map_err(failed)?;

  let laya = roads::selected_mode(&stand.ground.db, key).await.map_err(failed)? == LAYA;
  let on = if laya {
    "Laya, its own checkpoint".to_owned()
  } else {
    let provider = roads::road_provider(&stand.ground.db, key).await.map_err(failed)?;
    let model = tiers::resolve(&stand.ground.db, key, &provider)
      .await
      .map(|mapping| mapping.model)
      .unwrap_or_default();

    format!("{provider}/{model}")
  };

  said.line(format!("route {key} changed to {mode} ({on})"));

  Ok(said)
}

// A text the command line gives a worker's field: trimmed, and emptied to none.
fn worker_text(value: &str) -> Option<String> {
  let trimmed = value.trim();

  (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The preamble and the dev command as the command line gives them: none where neither flag was given,
/// and a text emptied to none where one was given empty.
struct Bench {
  preamble: Option<Option<String>>,
  dev_command: Option<Option<String>>,
}

fn bench(args: &AgentsArgs) -> Result<Bench, VerbError> {
  let preamble = match &args.preamble_file {
    Some(file) => Some(
      std::fs::read_to_string(file)
        .map_err(|why| VerbError::Refused(format!("{}: {why}", file.display())))?,
    ),
    None => args.preamble.clone(),
  };

  Ok(Bench {
    preamble: preamble.as_deref().map(worker_text),
    dev_command: args.dev_command.as_deref().map(worker_text),
  })
}

async fn add_worker(
  stand: &Stand<'_>,
  mut said: Said,
  name: &str,
  args: &AgentsArgs,
) -> Result<Said, VerbError> {
  let label = args.label.as_deref().unwrap_or(name);
  // No --path is a directory of the worker's own under the data root.
  let (cwd, made) = match args.path.as_deref().map(str::trim).filter(|path| !path.is_empty()) {
    Some(path) => (paths::resolve(path), false),
    None => (stand.ground.roots.data.join("worker-homes").join(name), true),
  };
  let cwd = cwd.to_string_lossy().into_owned();
  let bench = bench(args)?;
  let preamble = bench.preamble.flatten();
  let dev_command = bench.dev_command.flatten();
  let adding = Adding {
    name,
    label,
    cwd: &cwd,
    made,
    preamble: preamble.as_deref(),
    dev_command: dev_command.as_deref(),
  };
  let taken = workers::taken_names(&stand.ground.db).await.map_err(archive_failed)?;

  if let Some(fault) = worker_rules::new_worker_fault(&adding, &taken) {
    return Ok(said.refused(fault));
  }

  if made {
    std::fs::create_dir_all(&cwd).map_err(|why| VerbError::Refused(format!("{cwd}: {why}")))?;
  }

  workers::add_worker(
    &stand.ground.db,
    &NewWorker {
      name,
      label,
      cwd: &cwd,
      preamble: preamble.as_deref(),
      dev_command: dev_command.as_deref(),
    },
  )
  .await
  .map_err(archive_failed)?;
  stand.hands.settle(name).await;

  let checkout = if is_checkout(Path::new(&cwd)) {
    ""
  } else {
    " — not a git checkout: ordinary jobs only, no co-work"
  };

  said.line(format!("worker {name} ({label}) added at {cwd}{checkout}"));

  Ok(said)
}

// Whether a worker is busy by what is running: every worker where nobody could say.
fn busy(running: &Option<Vec<String>>, worker: &str) -> bool {
  running.as_ref().is_none_or(|running| running.iter().any(|one| one == worker))
}

/// A path on a node as its programs take it: one written from the home read against the node's home
/// where its setup has read it, and kept as written until then.
pub(crate) async fn on_node_home(
  db: &DatabaseConnection,
  node: &str,
  path: &str,
) -> Result<String, VerbError> {
  let home = workplace(db, node).await?.and_then(|place| place.home);

  Ok(crate::watches::links::node_path(path, home.as_deref()))
}

/// Where a base's checkout on a node stands unless a path is given: the base's own folder name followed
/// by the node's, under the same parent, written from the home where the base's path is under it.
pub fn node_checkout_path(base_cwd: &str, node: &str, home: &Path) -> String {
  let base = paths::resolve(base_cwd);
  let folder = base.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
  let path = base.parent().unwrap_or(Path::new("/")).join(format!("{folder}-{node}"));
  let path = path.to_string_lossy().into_owned();
  let home = home.to_string_lossy();

  match path.strip_prefix(&format!("{home}/")) {
    Some(rest) => format!("~/{rest}"),
    None => path,
  }
}

fn node_written(path: &str) -> bool {
  path.starts_with('/') || path.starts_with("~/")
}

/// A node the node service keeps: its name, the label it is shown under and whether it is reached over
/// SSH.
struct Listed {
  name: String,
  label: String,
  logs_in: bool,
}

// The node the node service keeps under a name, none where it keeps no such node.
async fn listed_node(stand: &Stand<'_>, node: &str) -> Result<Option<Listed>, VerbError> {
  let rows = stand
    .ground
    .peers
    .node
    .node_rows()
    .await
    .map_err(|fault| VerbError::Refused(fault.to_string()))?;
  let Some(row) = rows.into_iter().find(|row| row.node == node) else {
    return Ok(None);
  };

  Ok(Some(Listed {
    label: row.label.unwrap_or_else(|| row.node.clone()),
    logs_in: row.kind.over_ssh(),
    name: row.node,
  }))
}

// Why a node takes no job, in the owner's words, or none where it takes them.
fn node_refusal(target: &Listed) -> Option<String> {
  let refused = work_refusal(&target.name, target.logs_in, None)?;

  Some(fill_marks(
    &VERB_WORDS.agents.node_placing.no_jobs,
    &[("node", &target.name), ("why", &refused)],
  ))
}

/// A base's checkout on a node added as a support worker placed there: the worker it stands as and its
/// path, or why not.
pub(crate) async fn add_node_checkout(
  stand: &Stand<'_>,
  base: &str,
  node: &str,
  path: Option<&str>,
) -> Result<Result<(String, String), String>, VerbError> {
  let words = &VERB_WORDS.agents.node_checkout;
  let marks = [("base", base), ("node", node)];
  let row = workers::worker_row(&stand.ground.db, base, &stand.ground.source)
    .await
    .map_err(archive_failed)?;
  let Some(row) = row.filter(|row| !row.is_support()) else {
    return Ok(Err(fill_marks(&words.not_base, &marks)));
  };

  if let Some(on) = &row.node {
    return Ok(Err(fill_marks(&words.base_on_node, &[("base", base), ("node", on)])));
  }

  let Some(target) = listed_node(stand, node).await? else {
    return Ok(Err(fill_marks(&VERB_WORDS.agents.node_placing.no_node, &marks)));
  };

  if let Some(refused) = node_refusal(&target) {
    return Ok(Err(refused));
  }

  let held = workers::workers(&stand.ground.db, &stand.ground.source).await.map_err(archive_failed)?;

  if held
    .iter()
    .any(|worker| worker.support_of.as_deref() == Some(base) && worker.node.as_deref() == Some(node))
  {
    return Ok(Err(fill_marks(&words.already, &marks)));
  }

  let name = format!("{base}-{node}");

  if worker_rules::name_fault(&name, &[]).is_some() {
    return Ok(Err(fill_marks(&words.name_long, &[("base", base), ("name", &name)])));
  }

  if worker_rules::name_fault(&name, &workers::taken_names(&stand.ground.db).await.map_err(archive_failed)?)
    .is_some()
  {
    return Ok(Err(fill_marks(&words.name_taken, &[("name", &name)])));
  }

  let written = match path.map(str::trim).filter(|path| !path.is_empty()) {
    Some(path) => path.to_owned(),
    None => node_checkout_path(&row.cwd, node, stand.home),
  };

  if !node_written(&written) {
    return Ok(Err(words.path.clone()));
  }

  let cwd = on_node_home(&stand.ground.db, node, &written).await?;

  workers::add_node_checkout(
    &stand.ground.db,
    &NodeCheckout {
      name: &name,
      label: &node_checkout_label(&row.label, &target.label),
      cwd: &cwd,
      base,
      node,
    },
  )
  .await
  .map_err(archive_failed)?;

  Ok(Ok((name, cwd)))
}

/// A base's checkout on a node taken away, refused while a job holds it; the checkout on the node stays.
pub(crate) async fn remove_node_checkout(
  stand: &Stand<'_>,
  base: &str,
  node: &str,
  running: &Option<Vec<String>>,
) -> Result<Result<(String, String), String>, VerbError> {
  let words = &VERB_WORDS.agents.node_checkout;
  let held = workers::workers(&stand.ground.db, &stand.ground.source).await.map_err(archive_failed)?;
  let Some(worker) = held.into_iter().find(|worker| {
    worker.support_of.as_deref() == Some(base)
      && worker.node.as_deref() == Some(node)
      && worker.support_slot.is_none()
  }) else {
    return Ok(Err(fill_marks(&words.missing, &[("base", base), ("node", node)])));
  };

  if busy(running, &worker.name) {
    return Ok(Err(fill_marks(&words.busy, &[("worker", &worker.name)])));
  }

  if !workers::remove_support_worker(&stand.ground.db, &stand.ground.events, &worker.name)
    .await
    .map_err(archive_failed)?
  {
    return Ok(Err(fill_marks(&words.not_removed, &[("worker", &worker.name)])));
  }

  Ok(Ok((worker.name, worker.cwd)))
}

const NODE_CHECKOUT_USAGE: &str = "usage: bmo-agency agent --node-checkout <base> --node <node> [--path <path>] | --drop-node-checkout <base> --node <node>";

// A base's checkout on a node, added or taken away by itself (docs/work-nodes.md, 「A checkout on a
// node」).
async fn node_checkout(
  stand: &Stand<'_>,
  mut said: Said,
  base: &str,
  args: &AgentsArgs,
) -> Result<Said, VerbError> {
  let Some(node) = &args.node else {
    return Ok(said.refused(NODE_CHECKOUT_USAGE));
  };
  let running = stand.hands.running().await;
  let adding = args.node_checkout.is_some();
  let outcome = if adding {
    add_node_checkout(stand, base, node, args.path.as_deref()).await?
  } else {
    remove_node_checkout(stand, base, node, &running).await?
  };
  let (worker, cwd) = match outcome {
    Ok(done) => done,
    Err(fault) => return Ok(said.refused(fault)),
  };

  stand.hands.tell_workers().await;
  said.line(if adding {
    format!("node checkout {worker} added on {node} at {cwd}")
  } else {
    format!("node checkout {worker} removed; its checkout on {node} stays")
  });

  Ok(said)
}

// Resting is set apart from every other change, since a resting worker is no worker of the list the
// others are read from.
async fn rest_worker(stand: &Stand<'_>, mut said: Said, name: &str, rest: &str) -> Result<Said, VerbError> {
  let Ok(Some(resting)) = on_off(Some(rest), "--rest") else {
    return Ok(said.refused(fill_marks(&VERB_WORDS.agents.on_off, &[("flag", "--rest")])));
  };

  if !workers::set_resting(&stand.ground.db, name, resting).await.map_err(archive_failed)? {
    return Ok(said.refused(fill_marks(&VERB_WORDS.agents.not_restable, &[("worker", name)])));
  }

  stand.hands.settle(name).await;
  said.line(format!("worker {name} {}", if resting { "rests" } else { "woken" }));

  Ok(said)
}

/// A count the command line gives, read as a number reads one: none and spaces alone are nought, and a
/// word that reads as no number is no whole count.
pub fn number(value: &str) -> f64 {
  let trimmed = value.trim();

  if trimmed.is_empty() { 0.0 } else { trimmed.parse().unwrap_or(f64::NAN) }
}

/// Why a worker cannot stand on a node, or none where it may: a base with neither support workers nor
/// node checkouts, which is not the built-in one, on a node that takes work, at a path written on that
/// node. None for the node clears it, which needs no path.
pub(crate) async fn worker_node_fault(
  stand: &Stand<'_>,
  name: &str,
  node: Option<&str>,
  path: Option<&str>,
) -> Result<Option<String>, VerbError> {
  let words = &VERB_WORDS.agents.node_placing;
  let Some(row) = workers::worker_row(&stand.ground.db, name, &stand.ground.source)
    .await
    .map_err(archive_failed)?
  else {
    return Ok(Some(fill_marks(&VERB_WORDS.agents.no_worker, &[("worker", name)])));
  };
  let Some(node) = node else { return Ok(None) };

  if row.builtin {
    return Ok(Some(words.builtin.clone()));
  }

  if let Some(base) = &row.support_of {
    return Ok(Some(fill_marks(&words.support, &[("worker", name), ("base", base)])));
  }

  let held = workers::workers(&stand.ground.db, &stand.ground.source).await.map_err(archive_failed)?;

  if row.support_count > 0 || held.iter().any(|worker| worker.support_of.as_deref() == Some(name)) {
    return Ok(Some(fill_marks(&words.has_support, &[("worker", name)])));
  }

  let Some(target) = listed_node(stand, node).await? else {
    return Ok(Some(fill_marks(&words.no_node, &[("node", node)])));
  };

  if let Some(refused) = node_refusal(&target) {
    return Ok(Some(refused));
  }

  Ok((!path.is_some_and(node_written)).then(|| words.path.clone()))
}

async fn set_worker(
  stand: &Stand<'_>,
  mut said: Said,
  name: &str,
  args: &AgentsArgs,
) -> Result<Said, VerbError> {
  let held = workers::workers(&stand.ground.db, &stand.ground.source)
    .await
    .map_err(archive_failed)?
    .into_iter()
    .find(|worker| worker.name == name);
  let Some(held) = held else {
    return Ok(said.refused(fill_marks(&VERB_WORDS.agents.no_worker, &[("worker", name)])));
  };
  let bench = bench(args)?;
  let mut changes = Changes {
    preamble: bench.preamble,
    dev_command: bench.dev_command,
    herdr_label: args.herdr_label.as_deref().map(worker_text),
    ..Changes::default()
  };

  if let Some(pin) = &args.provider {
    match pins::pin_choice(&stand.ground.db, pin).await.map_err(failed)? {
      Ok(pin) => changes.pinned = Some((pin.provider, pin.model)),
      Err(fault) => return Ok(said.refused(fault)),
    }
  }

  match on_off(args.downgrade_heal.as_deref(), "--downgrade-heal") {
    Ok(heal) => changes.downgrade_heal = heal,
    Err(_) => return Ok(said.refused(VERB_WORDS.agents.heal_on_off.as_str())),
  }

  match on_off(args.keepalive.as_deref(), "--keepalive") {
    Ok(warming) => changes.keepalive_stopped = warming.map(|on| !on),
    Err(why) => return Ok(said.refused(why)),
  }

  changes.label = args.label.clone();

  // The node the worker stands on, with its path there; none brings it back to this machine, at the
  // path given here. A path on a node is the node's to read, so it is written as given and never judged
  // as a directory of this machine.
  let placed_on = args.node.as_deref().filter(|node| *node != "none");

  if let (Some(path), None) = (&args.path, placed_on) {
    changes.cwd = Some(paths::resolve(path).to_string_lossy().into_owned());
  }

  // The two support settings are judged as the pair they make once written: either may come alone.
  if args.support.is_some() || args.support_keepalive.is_some() {
    let count = args.support.as_deref().map_or(held.support_count as f64, number);
    let keepalive = args.support_keepalive.as_deref().map_or(held.support_keepalive as f64, number);

    if let Some(fault) = worker_rules::support_fault(&held, count, keepalive) {
      return Ok(said.refused(fault));
    }

    changes.support_count = Some(count as i64);
    changes.support_keepalive = Some(keepalive as i64);
  }

  if let Some(fault) = worker_rules::change_fault(&held, &changes) {
    return Ok(said.refused(fault));
  }

  if args.node.is_some() {
    let fault = match (placed_on, &args.path) {
      (None, None) => Some(VERB_WORDS.agents.back_here_path.clone()),
      (node, path) => worker_node_fault(stand, name, node, path.as_deref()).await?,
    };

    if let Some(fault) = fault {
      return Ok(said.refused(fault));
    }

    changes.node = Some(placed_on.map(str::to_owned));

    if let (Some(node), Some(path)) = (placed_on, &args.path) {
      changes.cwd = Some(on_node_home(&stand.ground.db, node, path.trim()).await?);
    }
  }

  if !workers::update_worker(&stand.ground.db, name, &changes).await.map_err(archive_failed)? {
    return Ok(said.refused(VERB_WORDS.agents.nothing_to_change.as_str()));
  }

  if changes.support_count.is_some() || changes.label.is_some() || changes.cwd.is_some() {
    let running = stand.hands.running().await;
    let settled = workers::settle_support(
      &stand.ground.db,
      stand.ground.peers.node.as_ref(),
      &stand.ground.events,
      name,
      &stand.ground.source,
      |worker| busy(&running, worker),
    )
    .await
    .map_err(archive_failed)?;

    settled
      .added
      .iter()
      .for_each(|added| said.line(format!("support worker {added} added")));
    settled
      .removed
      .iter()
      .for_each(|removed| said.line(format!("support worker {removed} removed")));
  }

  if changes.dev_command.is_some() {
    stand.hands.settle(name).await;
  }

  if changes.pinned.is_some() {
    stand.hands.tell_workers().await;
  }

  said.line(format!("worker {name} changed"));

  Ok(said)
}

// An agent's own thinking and effort (docs/model-tiers.md, 「An agent's own thinking and effort」): each
// flag left out keeps what the row holds, and default gives it back to the tier and the line.
async fn reasoning(
  stand: &Stand<'_>,
  mut said: Said,
  name: &str,
  args: &AgentsArgs,
) -> Result<Said, VerbError> {
  let held = one(
    &stand.ground.db,
    r#"SELECT "thinking", "effort" FROM "agents" WHERE "name" = $1"#,
    vec![name.into()],
  )
  .await?;
  let held_thinking = held
    .as_ref()
    .and_then(|row| row.try_get::<Option<i64>>("", "thinking").ok().flatten())
    .map(|thinking| json!(thinking != 0));
  let held_effort = held
    .as_ref()
    .and_then(|row| row.try_get::<Option<String>>("", "effort").ok().flatten())
    .map(|effort| json!(effort));
  let thinking = match args.thinking.as_deref() {
    None => held_thinking.unwrap_or(Value::Null),
    Some("default") => Value::Null,
    Some("on") => json!(true),
    Some("off") => json!(false),
    Some(other) => json!(other),
  };
  let effort = match args.effort.as_deref() {
    None => held_effort.unwrap_or(Value::Null),
    Some("default") => Value::Null,
    Some(level) => json!(level),
  };
  let kind = match set_reasoning(&stand.ground.db, name, &thinking, &effort).await.map_err(failed)? {
    Ok(kind) => kind,
    Err(fault) => return Ok(said.refused(fault)),
  };

  if kind == "worker" {
    stand.hands.tell_workers().await;
  }

  let thinking = match thinking.as_bool() {
    None => "default",
    Some(true) => "on",
    Some(false) => "off",
  };

  said.line(format!("{name}: thinking {thinking}, effort {}", effort.as_str().unwrap_or("default")));

  Ok(said)
}

// An agent's own gate switches (docs/store.md, 「The settings table」): the one --gate names switched, then
// every gate its turn passes with the switch it stands at.
async fn gates(
  db: &DatabaseConnection,
  mut said: Said,
  name: &str,
  gate: Option<&[String]>,
) -> Result<Said, VerbError> {
  if let Some(gate) = gate {
    let key = json!(gate[0]);
    let on = match gate.get(1).map(String::as_str) {
      Some("on") => json!(true),
      Some("off") => json!(false),
      Some(other) => json!(other),
      None => Value::Null,
    };

    if let Err(fault) = switch_gate(db, name, &key, &on).await.map_err(failed)? {
      return Ok(said.refused(fault));
    }
  }

  let switches = match agent_gates(db, name).await.map_err(failed)? {
    Ok(switches) => switches,
    Err(fault) => return Ok(said.refused(fault)),
  };

  for one in switches {
    let part = if one.part == "pre" {
      &VERB_WORDS.agents.pre_gate
    } else {
      &VERB_WORDS.agents.post_gate
    };

    said.line(format!("{part}  {}  {}", one.key, if one.on { "on" } else { "off" }));
  }

  Ok(said)
}

async fn remove_worker(stand: &Stand<'_>, mut said: Said, name: &str) -> Result<Said, VerbError> {
  let held: Option<Worker> = workers::workers(&stand.ground.db, &stand.ground.source)
    .await
    .map_err(archive_failed)?
    .into_iter()
    .find(|worker| worker.name == name);

  if let Some(fault) = worker_rules::remove_fault(held.as_ref(), name) {
    return Ok(said.refused(fault));
  }

  workers::remove_worker(&stand.ground.db, &stand.ground.events, name)
    .await
    .map_err(archive_failed)?;
  stand.hands.settle(name).await;
  said.line(format!("worker {name} removed"));

  Ok(said)
}

/// How long ago a moment was, in minutes under the hour and hours above it.
pub fn ago(at: Option<i64>, now: i64) -> String {
  let Some(at) = at else { return "never".to_owned() };
  // Rounded a half up.
  let round = |value: f64| (value + 0.5).floor() as i64;
  let minutes = round((now - at) as f64 / 60.0);

  if minutes < 60 {
    format!("{minutes}m ago")
  } else {
    format!("{}h ago", round(minutes as f64 / 60.0))
  }
}

/// Every agent the bot runs, one line each, and the resting workers last, apart, since none of them is a
/// worker any list takes.
async fn listing(db: &DatabaseConnection) -> Result<Vec<String>, VerbError> {
  let now = now_seconds();
  let rows = all(
    db,
    r#"SELECT * FROM "agents" WHERE "resting_since" IS NULL ORDER BY "kind", "name""#,
    vec![],
  )
  .await?;

  let mut lines = Vec::with_capacity(rows.len());

  for row in &rows {
    let agent = Agent::of(row).map_err(failed)?;

    lines.push(agent_line(db, &agent, now).await?);
  }

  for worker in workers::resting_workers(db).await.map_err(archive_failed)? {
    let label = worker.label.map(|label| format!(" {label}")).unwrap_or_default();

    lines.push(format!(
      "{:<20} worker  resting since {}{label}",
      worker.name,
      ago(Some(worker.since), now)
    ));
  }

  Ok(lines)
}

async fn agent_line(db: &DatabaseConnection, agent: &Agent, now: i64) -> Result<String, VerbError> {
  let worker = agent.kind == "worker";
  let place = agent.chat_id.map(|chat| format!(" chat {chat}")).unwrap_or_default();
  let session = agent
    .session_id
    .as_ref()
    .map(|session| format!(" session {}", session.chars().take(8).collect::<String>()))
    .unwrap_or_default();
  // A worker says where it works, since that is the whole of what tells two of them apart. One on a node
  // says which, and its directory is the node's to read (docs/work-nodes.md).
  let cwd = match (&agent.cwd, &agent.node) {
    (None, _) => String::new(),
    (Some(cwd), Some(node)) => format!(" node {node} cwd {cwd}"),
    (Some(cwd), None) => {
      let checkout = if worker && !is_checkout(Path::new(cwd)) { " (not a checkout)" } else { "" };

      format!(" cwd {cwd}{checkout}")
    }
  };
  let paused = if agent.keepalive_paused_at.is_some() { " keepalive off until next job" } else { "" };
  let warming = if agent.keepalive_stopped { " keepalive off" } else { "" };
  let heal = if worker && !agent.downgrade_heal { " downgrade heal off" } else { "" };
  // A base's support settings, and a support worker's base and number (docs/worker-slots.md).
  let support = match (&agent.support_of, &agent.node, agent.support_slot) {
    (Some(base), Some(node), None) => format!(" checkout of {base} on {node}"),
    (Some(base), _, slot) => {
      format!(" support {} of {base}", slot.map_or("?".to_owned(), |slot| slot.to_string()))
    }
    (None, ..) if agent.support_count > 0 => {
      format!(" support workers {} (kept warm {})", agent.support_count, agent.support_keepalive)
    }
    (None, ..) => String::new(),
  };
  let label = agent.label.as_ref().map(|label| format!(" {label}")).unwrap_or_default();
  // What the agent is configured to begin on is the shared tier settings' answer for its work key.
  let route = configured(db, agent).await;
  let pinned = if roads::profile(&agent.name).is_some() {
    roads::route_row(db, &agent.name).await.map_err(failed)?.and_then(|row| row.account)
  } else {
    None
  };
  let configured = match &route {
    None => String::new(),
    Some(route) if route.laya => " configured laya".to_owned(),
    Some(route) => format!(
      " configured {}/{}{}",
      route.provider,
      route.model.as_deref().unwrap_or("-"),
      pinned.map(|account| format!(" on {account}")).unwrap_or_default()
    ),
  };
  let actual = match &agent.last_mode {
    None => String::new(),
    Some(mode) => format!(
      " answered {mode}/{}{}",
      agent.last_model.as_deref().unwrap_or("-"),
      agent.last_effort.as_ref().map(|effort| format!("/{effort}")).unwrap_or_default()
    ),
  };
  let model = match &route {
    Some(route) if route.laya => "laya",
    Some(route) => route.model.as_deref().unwrap_or("-"),
    None => "-",
  };

  Ok(format!(
    "{:<20} {:<7} {model:<28} last run {:<9} keepalive {:<9} runs {:>4}{label}{configured}{actual}{place}{session}{cwd}{support}{warming}{paused}{heal}",
    agent.name,
    agent.kind,
    ago(agent.last_run_at, now),
    ago(agent.last_keepalive_at, now),
    agent.runs,
  ))
}
