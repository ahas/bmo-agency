//! The one road resolution (docs/cache-fallback.md, 「One road resolution」): the one place that decides where
//! a work key's work goes out now. The key's setting — a worker's own pin, else the key's row, else
//! automatic — is the head of a walk through the owner's provider order; every provider of that walk answers
//! for itself whether it can answer now (「Availability: whether a provider can answer now」), each out of
//! its own member, and the first that can carries the work, on the pinned model where the pin names one and
//! the key's tier mapping everywhere else, on the first login the account judgement offers where the
//! provider holds logins. The resolution knows no provider by name. A move off the head of the walk, a
//! return to it and an order that cannot answer at all are each published once, as the facts of
//! `road.changed` (docs/api.md, 「Events」).

use cache_siren::Slot;
use harness_providers::{Machine, Provider};
use policy::fill_marks;
use policy::roads::ROADS;
use policy::work_keys::WORK_KEYS;
use serde::{Deserialize, Serialize};

use crate::control::claude_accounts::{self, Judging, moment_words};
use crate::control::harness::{self, provider_order};
use crate::control::roads::{self, AUTO, LAYA};
use crate::control::tiers;
use crate::events::Event;
use crate::now_ms;
use crate::places::Ground;
use crate::queue::routes::worker_pin_held;
use crate::queue::settings;

/// What the resolution is asked: the work key, the worker a job goes out on where it is a job's, the user a
/// run is made for, and the providers a job already left in this job with why, which its walk counts as
/// unable to answer.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ask<'a> {
  pub key: &'a str,
  pub worker: Option<&'a str>,
  pub user: Option<i64>,
  pub left: &'a [Closed],
}

impl<'a> Ask<'a> {
  /// A work key's own work.
  pub fn key(key: &'a str) -> Self {
    Self {
      key,
      ..Self::default()
    }
  }
}

/// The head a key's setting gives its walk: a provider, and one model of it where a worker's pin names one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
  pub provider: String,
  pub model: Option<String>,
}

/// One road: the provider work goes out on, the model, and the login where the provider holds logins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Road {
  pub provider: String,
  pub model: String,
  pub account: Option<String>,
}

impl Road {
  /// The road as a sentence names it: its provider and model, and its login where it has one.
  pub fn words(&self) -> String {
    match &self.account {
      Some(account) => format!("{} · {} ({account})", self.provider, self.model),
      None => format!("{} · {}", self.provider, self.model),
    }
  }
}

/// Why one provider cannot answer now: its reason, what a person does to reopen it where a person can, and
/// the epoch second it reopens on its own where it waits for one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Closed {
  pub provider: String,
  pub reason: String,
  #[serde(skip)]
  pub command: Option<String>,
  #[serde(skip)]
  pub reopens_at: Option<i64>,
}

impl Closed {
  /// A reason with nothing a person does and no moment it reopens.
  pub fn plain(provider: &str, reason: impl Into<String>) -> Self {
    Self {
      provider: provider.to_owned(),
      reason: reason.into(),
      command: None,
      reopens_at: None,
    }
  }
}

/// What one provider of the walk answers of itself for the key now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
  /// It is no road of this key: its program does not carry the key's line, the key maps no model on it,
  /// its registration is not walked, or the owner has it off in an automatic walk.
  NoRoad,
  /// It cannot answer now, and why.
  Cannot(Closed),
  /// It can, on this road.
  Can(Road),
}

/// One provider of the walk, with the model the key would go out on there and what it answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
  pub provider: String,
  pub model: Option<String>,
  pub answer: Answer,
}

/// Where a key's work goes now, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
  pub key: String,
  /// The provider the key's setting pins, none where it walks automatically.
  pub pinned: Option<String>,
  /// Every provider of the walk in its order, with what it answered.
  pub steps: Vec<Step>,
}

/// The walk over the owner's order: automatic from the top; a pin from its provider, then the providers
/// after it in the order, then those before it.
pub fn walk(order: &[String], pin: Option<&str>) -> Vec<String> {
  let Some(at) = pin.and_then(|pin| order.iter().position(|one| one == pin)) else {
    return order.to_vec();
  };

  order[at..].iter().chain(&order[..at]).cloned().collect()
}

impl Resolution {
  /// The road work goes out on now: the first of the walk that can answer.
  pub fn road(&self) -> Option<&Road> {
    self.open().next()
  }

  /// Every road of the walk that can answer now, in the walk's order.
  pub fn open(&self) -> impl Iterator<Item = &Road> {
    self.steps.iter().filter_map(|step| match &step.answer {
      Answer::Can(road) => Some(road),
      _ => None,
    })
  }

  /// Every provider of the walk that cannot answer now, with why.
  pub fn closed(&self) -> Vec<&Closed> {
    self
      .steps
      .iter()
      .filter_map(|step| match &step.answer {
        Answer::Cannot(closed) => Some(closed),
        _ => None,
      })
      .collect()
  }

  /// The providers passed ahead of the road work goes out on, each with why it could not answer.
  pub fn passed(&self) -> Vec<&Closed> {
    let mut passed = vec![];

    for step in &self.steps {
      match &step.answer {
        Answer::Can(_) => break,
        Answer::Cannot(closed) => passed.push(closed),
        Answer::NoRoad => {}
      }
    }

    passed
  }

  /// The head of the walk: the pinned provider where the key is pinned, else the first provider of the walk
  /// that is a road of the key, on the model the key would go out on there.
  pub fn head(&self) -> Option<Road> {
    let step = match &self.pinned {
      Some(pinned) => self.steps.iter().find(|step| &step.provider == pinned)?,
      None => self.steps.iter().find(|step| step.answer != Answer::NoRoad)?,
    };

    match &step.answer {
      Answer::Can(road) => Some(road.clone()),
      _ => Some(Road {
        provider: step.provider.clone(),
        model: step.model.clone().unwrap_or_default(),
        account: None,
      }),
    }
  }

  /// What a person does to reopen the first road a person can reopen, and which provider it is.
  pub fn command(&self) -> Option<(&str, &str)> {
    self
      .closed()
      .into_iter()
      .find_map(|closed| closed.command.as_deref().map(|command| (closed.provider.as_str(), command)))
  }

  /// The epoch second the first road that waits for a moment reopens on its own.
  pub fn reopens_at(&self) -> Option<i64> {
    self.closed().into_iter().filter_map(|closed| closed.reopens_at).min()
  }

  /// The sentence a closed road is refused or held with: every provider's reason, the moment the first
  /// road that waits for one reopens, and what a person does to reopen the first a person can.
  pub fn sentence(&self) -> String {
    let words = &ROADS.closed;
    let mut lines = vec![words.head.clone()];

    for closed in self.closed() {
      lines.push(fill_marks(
        &words.provider,
        &[("provider", &closed.provider), ("reason", &closed.reason)],
      ));
    }

    if let Some(at) = self.reopens_at() {
      lines.push(fill_marks(&words.reopens, &[("at", &moment_words(at))]));
    }

    if let Some((provider, command)) = self.command() {
      lines.push(fill_marks(&words.person, &[("provider", provider), ("command", command)]));
    }

    lines.join("\n")
  }
}

// The head a key's setting gives its walk: a worker's own pin where the ask names a worker that has one,
// else the key's row where it names one provider; a gate on Laya and an automatic key walk from the top.
async fn pin_of(ground: &Ground, ask: &Ask<'_>) -> Result<Option<Pin>, String> {
  let db = &ground.db;

  if let Some(worker) = ask.worker
    && let Some((provider, model)) = worker_pin_held(db, worker).await.map_err(|unrouted| unrouted.0)?
  {
    return Ok(Some(Pin { provider, model }));
  }

  let mode = roads::selected_mode(db, ask.key).await.map_err(|refused| refused.error)?;

  Ok((mode != AUTO && mode != LAYA && harness::named(&mode).is_ok()).then_some(Pin {
    provider: mode,
    model: None,
  }))
}

/// Where a key's work goes out now (docs/cache-fallback.md, 「One road resolution」).
pub async fn resolve(ground: &Ground, ask: &Ask<'_>) -> Result<Resolution, String> {
  let db = &ground.db;
  let machine = ground.machine();
  let order = provider_order(db).await.map_err(|refused| refused.error)?;
  let pin = pin_of(ground, ask).await?;
  let names: Vec<String> = order.iter().map(|entry| entry.name.clone()).collect();
  let mut steps = vec![];

  for name in walk(&names, pin.as_ref().map(|pin| pin.provider.as_str())) {
    let provider = harness::named(&name).map_err(|refused| refused.error)?;
    let enabled = order.iter().any(|entry| entry.name == name && entry.enabled);
    let pinned = pin.as_ref().filter(|pin| pin.provider == name);
    let mut asked = step(ground, &machine, ask, provider, enabled, pinned).await?;

    // A provider this job already left is one that cannot answer it, for the reason it was left.
    if let Some(left) = ask.left.iter().find(|left| left.provider == name)
      && asked.answer != Answer::NoRoad
    {
      asked.answer = Answer::Cannot(left.clone());
    }

    steps.push(asked);
  }

  Ok(Resolution {
    key: ask.key.to_owned(),
    pinned: pin.map(|pin| pin.provider),
    steps,
  })
}

// One provider of the walk asked whether it can answer for the key now, every part of the answer its own
// member's: whether its program carries the key's line, its presence on this machine, the balance it offers
// and the logins it holds.
async fn step(
  ground: &Ground,
  machine: &Machine,
  ask: &Ask<'_>,
  provider: &dyn Provider,
  enabled: bool,
  pin: Option<&Pin>,
) -> Result<Step, String> {
  let db = &ground.db;
  let name = provider.name();
  // A key no profile declares names no line, and every provider carries it.
  let line = WORK_KEYS.profile(ask.key).map(|profile| profile.line.as_str());
  let step = |model: Option<String>, answer: Answer| Step {
    provider: name.to_owned(),
    model,
    answer,
  };
  let refused = |refused: crate::control::Refused| refused.error;

  if line.is_some_and(|line| !provider.declared().carries(line)) || (pin.is_none() && !enabled) {
    return Ok(step(None, Answer::NoRoad));
  }

  if pin.is_none()
    && let Some(line) = line
    && !roads::walked(db, provider, ask.key, line).await.map_err(refused)?
  {
    return Ok(step(None, Answer::NoRoad));
  }

  let mapped = match pin.and_then(|pin| pin.model.clone()) {
    Some(model) => Ok(model),
    None if enabled => tiers::resolve_execution(db, ask.key, name, None).await.map(|mapping| mapping.model),
    None => tiers::resolve(db, ask.key, name).await.map(|mapping| mapping.model),
  };
  let model = match mapped {
    Ok(model) => model,
    Err(_) if pin.is_none() => return Ok(step(None, Answer::NoRoad)),
    Err(why) => return Ok(step(None, Answer::Cannot(Closed::plain(name, why)))),
  };
  let cannot = |closed: Closed| Ok(step(Some(model.clone()), Answer::Cannot(closed)));

  if !enabled {
    return cannot(Closed::plain(name, &ROADS.reasons.off));
  }

  if let Some(missing) = harness::missing(db, machine, provider).await.map_err(refused)? {
    return cannot(Closed::plain(name, missing));
  }

  if let Some(closed) = balance_spent(ground, provider).await {
    return cannot(closed);
  }

  let account = if provider.holds_logins() {
    match logins(ground, ask, name).await? {
      Ok(account) => account,
      Err(closed) => return cannot(closed),
    }
  } else {
    None
  };

  Ok(step(
    Some(model.clone()),
    Answer::Can(Road {
      provider: name.to_owned(),
      model,
      account,
    }),
  ))
}

// A provider that offers a balance and whose own road's last reading is at or below zero: closed, with
// where its member says the balance is topped up. A balance never read, or a keeping that cannot be read,
// closes nothing.
async fn balance_spent(ground: &Ground, provider: &dyn Provider) -> Option<Closed> {
  let offers = provider.balance()?;
  let kept = match ground
    .windows
    .balance(Slot {
      mode: provider.name(),
      account: "",
    })
    .await
  {
    Ok(kept) => kept?,
    Err(fault) => {
      eprintln!("road: the balance of {}: not read: {fault}", provider.name());
      return None;
    }
  };

  if kept.balance.amount > 0.0 {
    return None;
  }

  let amount = format!("{}", kept.balance.amount);
  let reason =
    fill_marks(&ROADS.reasons.balance, &[("amount", &amount), ("currency", &kept.balance.currency)]);

  Some(Closed {
    command: Some(offers.declared().top_up.clone()),
    ..Closed::plain(provider.name(), reason)
  })
}

// The login a provider that holds logins offers the key first, by the account judgement; none where no
// login carries the key's run, which goes out as it stands. Where every login that carries it is closed,
// the provider is, with every login's own reason, what a person does for the first a person can reopen,
// and the moment the first that rests reopens.
async fn logins(
  ground: &Ground,
  ask: &Ask<'_>,
  name: &str,
) -> Result<Result<Option<String>, Closed>, String> {
  let judging = Judging::of(ground);
  let refused = |refused: crate::control::Refused| refused.error;
  let open = claude_accounts::walk_for_user(judging, ask.key, ask.user).await.map_err(refused)?;

  if let Some(first) = open.first() {
    return Ok(Ok(Some(first.id.clone())));
  }

  let carried = claude_accounts::carried_for_user(&ground.db, ask.key, ask.user)
    .await
    .map_err(refused)?;

  if carried.is_empty() {
    return Ok(Ok(None));
  }

  let now = now_ms();
  let machine = ground.machine();
  let mut reasons = vec![];
  let mut command = None;
  let mut reopens_at: Option<i64> = None;

  for account in &carried {
    let Some(why) = claude_accounts::closed_why(judging, account, now).await.map_err(refused)? else {
      continue;
    };

    reasons.push(why.reason(account));
    command = command.or_else(|| why.command(account, &machine));

    if let claude_accounts::LoginClosed::Resting(until) = why {
      reopens_at = Some(reopens_at.map_or(until, |first| first.min(until)));
    }
  }

  Ok(Err(Closed {
    provider: name.to_owned(),
    reason: format!("{}: {}", ROADS.reasons.no_login, reasons.join("; ")),
    command,
    reopens_at,
  }))
}

/// Why a provider cannot answer now whatever the key, or none where it can: the owner has it off, it is
/// missing on this machine, or its balance is spent. What the warming asks before it warms a provider's
/// sessions, which leaves the logins to the account judgement it already walks.
pub async fn provider_closed(ground: &Ground, name: &str) -> Result<Option<String>, String> {
  let refused = |refused: crate::control::Refused| refused.error;
  let provider = harness::named(name).map_err(refused)?;

  if !harness::provider_enabled(&ground.db, name).await.map_err(refused)? {
    return Ok(Some(ROADS.reasons.off.clone()));
  }

  if let Some(missing) = harness::missing(&ground.db, &ground.machine(), provider).await.map_err(refused)? {
    return Ok(Some(missing));
  }

  Ok(balance_spent(ground, provider).await.map(|closed| closed.reason))
}

// ---- the facts of a key's road ----

/// What is kept of a key's road between its runs: the road it moved to, or that nothing could answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Kept {
  Moved { road: Road },
  Closed,
}

/// One fact of a key's road, published once (docs/api.md, 「Events」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fact {
  Moved {
    from: Option<Road>,
    to: Road,
    reasons: Vec<Closed>,
  },
  Returned {
    from: Option<Road>,
    to: Road,
  },
  Closed {
    reasons: Vec<Closed>,
    command: Option<String>,
    reopens_at: Option<i64>,
  },
}

/// What a key's road comes to as work goes out on `took` — none where nothing could — against what was kept
/// of it: the fact to publish, where one is new, and what is kept after it. Work on the head of its walk
/// with nothing kept is no fact; a move to where the last move already went is none either, and nor is a
/// closed road that was already closed.
pub fn fact(
  kept: Option<&Kept>,
  resolution: &Resolution,
  took: Option<&Road>,
  failed: &[Closed],
) -> (Option<Fact>, Option<Kept>) {
  let Some(took) = took else {
    let fact = (kept != Some(&Kept::Closed)).then(|| Fact::Closed {
      reasons: resolution.closed().into_iter().cloned().chain(failed.iter().cloned()).collect(),
      command: resolution.command().map(|(_, command)| command.to_owned()),
      reopens_at: resolution.reopens_at(),
    });

    return (fact, Some(Kept::Closed));
  };
  let head = resolution.head();

  if head.as_ref().is_none_or(|head| head.provider == took.provider) {
    let fact = match kept {
      Some(Kept::Moved { road }) => Some(Fact::Returned {
        from: Some(road.clone()),
        to: took.clone(),
      }),
      Some(Kept::Closed) => Some(Fact::Returned {
        from: None,
        to: took.clone(),
      }),
      None => None,
    };

    return (fact, None);
  }

  let again = matches!(kept, Some(Kept::Moved { road }) if road.provider == took.provider && road.model == took.model);
  let fact = (!again).then(|| Fact::Moved {
    from: match kept {
      Some(Kept::Moved { road }) => Some(road.clone()),
      _ => head.clone(),
    },
    to: took.clone(),
    reasons: resolution.passed().into_iter().cloned().chain(failed.iter().cloned()).collect(),
  });

  (fact, Some(Kept::Moved { road: took.clone() }))
}

/// A key's road moved off the head of its walk: the road it moved from and to, and why each provider passed
/// could not answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadMoved {
  pub fact: &'static str,
  pub key: String,
  pub from: Option<Road>,
  pub to: Road,
  pub reasons: Vec<Closed>,
}

impl Event for RoadMoved {
  const NAME: &'static str = "road.changed";
}

/// A key's road back on the head of its walk after a move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadReturned {
  pub fact: &'static str,
  pub key: String,
  pub from: Option<Road>,
  pub to: Road,
}

impl Event for RoadReturned {
  const NAME: &'static str = "road.changed";
}

/// Nothing in a key's order can answer: every provider's reason, what a person does to reopen the first
/// road a person can, and the moment the first that waits for one reopens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadClosed {
  pub fact: &'static str,
  pub key: String,
  pub reasons: Vec<Closed>,
  pub command: Option<String>,
  pub reopens_at: Option<i64>,
}

impl Event for RoadClosed {
  const NAME: &'static str = "road.changed";
}

fn kept_key(key: &str) -> String {
  format!("road_kept:{key}")
}

/// That a key's work goes out on `took` now — none where nothing in its order could answer — told once: the
/// fact it comes to published on the bus, and what is kept of the road written for the next run. `failed`
/// are the roads the work was tried on and that did not answer, beside the ones the resolution passed.
pub async fn went(ground: &Ground, resolution: &Resolution, took: Option<&Road>, failed: &[Closed]) {
  let db = &ground.db;
  let key = kept_key(&resolution.key);
  let kept = match settings::get(db, &key).await {
    Ok(kept) => kept.and_then(|text| serde_json::from_str::<Kept>(&text).ok()),
    Err(why) => {
      return eprintln!("road {}: what was kept of it could not be read: {}", resolution.key, why.detail);
    }
  };
  let (fact, after) = fact(kept.as_ref(), resolution, took, failed);

  if after != kept {
    let written = match &after {
      Some(after) => settings::set(db, &key, &serde_json::to_string(after).unwrap_or_default()).await,
      None => settings::forget(db, &key).await,
    };

    if let Err(why) = written {
      eprintln!("road {}: what is kept of it was not written: {}", resolution.key, why.detail);
    }
  }

  let key = resolution.key.clone();

  match fact {
    None => {}
    Some(Fact::Moved { from, to, reasons }) => {
      println!("road {key}: moved to {}", to.words());
      ground.events.publish(RoadMoved {
        fact: "road-moved",
        key,
        from,
        to,
        reasons,
      });
    }
    Some(Fact::Returned { from, to }) => {
      println!("road {key}: back on {}", to.words());
      ground.events.publish(RoadReturned {
        fact: "road-returned",
        key,
        from,
        to,
      });
    }
    Some(Fact::Closed {
      reasons,
      command,
      reopens_at,
    }) => {
      println!("road {key}: no provider of the order can answer");
      ground.events.publish(RoadClosed {
        fact: "road-closed",
        key,
        reasons,
        command,
        reopens_at,
      });
    }
  }
}
