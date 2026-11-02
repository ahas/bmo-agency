//! The one road resolution (docs/provider-order.md, 「Choosing the provider」): the one place that decides
//! where a supervisor's or a role's run goes out now. The row's own order is walked from the top; every
//! provider of it answers for itself whether it can answer now — the owner's switch, its presence on this
//! machine, its balance and its logins — and whether it has quota above the row's share, each out of its own
//! member, and the first that can carries the run on the model, effort and thinking its entry names, on the
//! first login with quota where the provider holds logins. A run whose caller fixes its road — a repository's
//! pin, a delegation as its supervisor's double, a native co-worker on its caller's provider, a session's
//! named road — walks that road alone and waits while it cannot answer. The resolution knows no provider by
//! name. A move off the top of the order, a return to it and an order that cannot answer at all are each
//! published once, as the facts of `road.changed` (docs/api.md, 「Events」).

use cache_siren::Slot;
use harness_providers::{Machine, Provider};
use policy::fill_marks;
use policy::roads::ROADS;
use serde::{Deserialize, Serialize};

use crate::control::claude_accounts::{self, Judging, moment_words};
use crate::control::harness;
use crate::events::Event;
use crate::now_ms;
use crate::order::{self, Entry, Order};
use crate::places::Ground;
use crate::queue::settings;

/// The row a run is of: a supervisor or a role, by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row<'a> {
  Supervisor(&'a str),
  Role(&'a str),
}

impl Row<'_> {
  /// The row's name.
  pub fn name(&self) -> &str {
    match self {
      Self::Supervisor(name) | Self::Role(name) => name,
    }
  }

  // The key what is kept of its road stands under.
  fn kept_key(&self) -> String {
    match self {
      Self::Supervisor(name) => format!("road_kept:supervisor:{name}"),
      Self::Role(name) => format!("road_kept:role:{name}"),
    }
  }
}

/// What the resolution is asked: the row, the line its runs go out on, its order, the user a run is made for,
/// the providers a job already left in this job with why, which its walk counts as unable to answer, and the
/// road the caller fixed where it fixes one.
#[derive(Clone, Copy, Debug)]
pub struct Ask<'a> {
  pub row: Row<'a>,
  pub line: &'a str,
  pub order: &'a Order,
  pub user: Option<i64>,
  pub left: &'a [Closed],
  pub fixed: Option<&'a Entry>,
}

impl<'a> Ask<'a> {
  /// A row's own run, walking its order.
  pub fn of(row: Row<'a>, line: &'a str, order: &'a Order) -> Self {
    Self {
      row,
      line,
      order,
      user: None,
      left: &[],
      fixed: None,
    }
  }
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

/// What one provider of the walk answers of itself for the row now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
  /// It is no road of this row: its program does not carry the row's line.
  NoRoad,
  /// It cannot answer now, and why.
  Cannot(Closed),
  /// It can, on this road.
  Can(Road),
}

/// One provider of the walk, with the entry the row would go out on there and what it answered.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
  pub provider: String,
  pub entry: Entry,
  pub answer: Answer,
}

/// Where a row's run goes now, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolution {
  pub row: String,
  pub supervisor: bool,
  /// Every provider of the walk in its order, with what it answered.
  pub steps: Vec<Step>,
  /// The row's line, and the providers of its order whose program cannot carry it.
  pub line: String,
  pub uncarried: Vec<String>,
  /// Whether the caller fixed the road, so the walk was that road alone.
  pub fixed: bool,
}

impl Resolution {
  /// The road work goes out on now: the first of the walk that can answer.
  pub fn road(&self) -> Option<&Road> {
    self.open().next()
  }

  /// The step the road work goes out on now stands on, with its entry.
  pub fn taken(&self) -> Option<&Step> {
    self.steps.iter().find(|step| matches!(step.answer, Answer::Can(_)))
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

  /// The head of the walk: the first provider of the order that is a road of the row, on the model it would
  /// go out on there.
  pub fn head(&self) -> Option<Road> {
    let step = self.steps.iter().find(|step| step.answer != Answer::NoRoad)?;

    match &step.answer {
      Answer::Can(road) => Some(road.clone()),
      _ => Some(Road {
        provider: step.provider.clone(),
        model: step.entry.model.clone(),
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

  /// The sentence a closed road is held with: every provider's reason, the moment the first road that waits
  /// for one reopens, and what a person does to reopen the first a person can.
  pub fn sentence(&self) -> String {
    let words = &ROADS.closed;
    let mut lines = vec![if self.fixed { words.named.clone() } else { words.head.clone() }];

    for closed in self.closed() {
      lines.push(fill_marks(
        &words.provider,
        &[("provider", &closed.provider), ("reason", &closed.reason)],
      ));
    }

    if let Some(at) = self.reopens_at() {
      lines.push(fill_marks(&words.reopens, &[("at", &moment_words(at))]));
    }

    if !self.uncarried.is_empty() {
      let needs = policy::run_lines::RUN_LINES.line(&self.line).map(|declared| declared.needs.join(", "));

      lines.push(fill_marks(
        &words.uncarried,
        &[
          ("providers", &self.uncarried.join(", ")),
          ("line", &self.line),
          ("needs", &needs.unwrap_or_default()),
        ],
      ));
    }

    if let Some((provider, command)) = self.command() {
      lines.push(fill_marks(&words.person, &[("provider", provider), ("command", command)]));
    }

    lines.join("\n")
  }
}

/// Where a job of a role at a checkout goes out now: its repository's pinned model alone where the repository
/// pins one, which its work waits for and never leaves, else the role's own order (docs/repositories.md, 「A
/// pinned model」); the job role where none is named. The providers the job already left count as unable to
/// answer.
pub async fn for_job(
  ground: &Ground,
  role: Option<&str>,
  checkout: Option<&str>,
  user: Option<i64>,
  left: &[Closed],
) -> Result<Resolution, String> {
  let role = role.unwrap_or_else(|| crate::agent_roles::job_role());
  let held = crate::agent_roles::named(&ground.db, role)
    .await
    .map_err(|why| why.detail)?
    .ok_or_else(|| crate::agent_roles::unknown(role))?;
  let pin = match checkout {
    Some(checkout) => crate::queue::workers::repository_of(&ground.db, checkout)
      .await
      .map_err(|why| why.detail)?
      .and_then(|repository| repository.pin),
    None => None,
  };
  // A pin goes out at the effort the role's order names for its provider, else the provider's own default.
  let fixed = pin.map(|pin| {
    let own = held.order.entry(&pin.provider);

    Entry {
      provider: pin.provider.clone(),
      model: pin.model,
      effort: own.and_then(|entry| entry.effort.clone()),
      thinking: own.and_then(|entry| entry.thinking.clone()),
      answer_tokens: own.and_then(|entry| entry.answer_tokens),
    }
  });
  let ask = Ask {
    user,
    left,
    fixed: fixed.as_ref(),
    ..Ask::of(Row::Role(&held.name), &held.line, &held.order)
  };

  resolve(ground, &ask).await
}

/// Where a row's run goes out now (docs/provider-order.md, 「Choosing the provider」).
pub async fn resolve(ground: &Ground, ask: &Ask<'_>) -> Result<Resolution, String> {
  let machine = ground.machine();
  let walked: Vec<&Entry> = match ask.fixed {
    Some(entry) => vec![entry],
    None => ask.order.entries.iter().collect(),
  };
  let mut steps = vec![];
  let mut uncarried = vec![];

  for entry in walked {
    let provider = harness::named(&entry.provider).map_err(|refused| refused.error)?;
    let mut asked = step(ground, &machine, ask, provider, entry).await?;

    if asked.answer == Answer::NoRoad {
      uncarried.push(entry.provider.clone());
    }

    // A provider this job already left is one that cannot answer it, for the reason it was left.
    if let Some(left) = ask.left.iter().find(|left| left.provider == entry.provider)
      && asked.answer != Answer::NoRoad
    {
      asked.answer = Answer::Cannot(left.clone());
    }

    steps.push(asked);
  }

  Ok(Resolution {
    row: ask.row.name().to_owned(),
    supervisor: matches!(ask.row, Row::Supervisor(_)),
    steps,
    line: ask.line.to_owned(),
    uncarried,
    fixed: ask.fixed.is_some(),
  })
}

// One provider of the walk asked whether it can answer for the row now, every part of the answer its own
// member's: whether its program carries the row's line, the owner's switch, its presence on this machine,
// the balance it offers, the quota it has left above the row's share and the logins it holds.
async fn step(
  ground: &Ground,
  machine: &Machine,
  ask: &Ask<'_>,
  provider: &dyn Provider,
  entry: &Entry,
) -> Result<Step, String> {
  let db = &ground.db;
  let name = provider.name();
  let step = |answer: Answer| Step {
    provider: name.to_owned(),
    entry: entry.clone(),
    answer,
  };
  let refused = |refused: crate::control::Refused| refused.error;
  let cannot = |closed: Closed| Ok(step(Answer::Cannot(closed)));

  if !provider.declared().carries(ask.line) {
    if ask.fixed.is_some() {
      let reason = fill_marks(&ROADS.reasons.uncarried, &[("line", ask.line)]);

      return cannot(Closed::plain(name, reason));
    }

    return Ok(step(Answer::NoRoad));
  }

  if !harness::provider_enabled(db, name).await.map_err(refused)? {
    return cannot(Closed::plain(name, &ROADS.reasons.off));
  }

  if let Some(missing) = harness::missing(db, machine, provider).await.map_err(refused)? {
    return cannot(Closed::plain(name, missing.text));
  }

  if let Some(closed) = balance_spent(ground, provider).await {
    return cannot(closed);
  }

  let quota = order::quota(ground, provider, ask.order.quota_share, ask.user).await;

  if !quota.has && !provider.holds_logins() {
    let reason = fill_marks(
      &ROADS.reasons.quota,
      &[
        ("left", &quota.left.map_or_else(String::new, percent)),
        ("share", &percent(ask.order.quota_share)),
      ],
    );

    return cannot(Closed {
      reopens_at: quota.resets_at,
      ..Closed::plain(name, reason)
    });
  }

  let account = if provider.holds_logins() {
    match logins(ground, ask, name, &quota).await? {
      Ok(account) => account,
      Err(closed) => return cannot(closed),
    }
  } else {
    None
  };

  Ok(step(Answer::Can(Road {
    provider: name.to_owned(),
    model: entry.model.clone(),
    account,
  })))
}

// A share as a person reads it.
fn percent(share: f64) -> String {
  format!("{:.0}%", share * 100.0)
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

// The login a provider that holds logins offers the run first: the open login with quota above the share
// that the quota read counted first, else the account judgement's first. Where every login is closed, the
// provider is, with every login's own reason, what a person does for the first a person can reopen, and the
// moment the first that rests reopens; where logins are open but none has quota above the share, the
// provider is closed for its quota.
async fn logins(
  ground: &Ground,
  ask: &Ask<'_>,
  name: &str,
  quota: &order::Quota,
) -> Result<Result<Option<String>, Closed>, String> {
  let judging = Judging::of(ground);
  let refused = |refused: crate::control::Refused| refused.error;
  let open = claude_accounts::walk_for_user(judging, ask.user).await.map_err(refused)?;

  if !open.is_empty() {
    if !quota.has {
      let reason = fill_marks(
        &ROADS.reasons.quota,
        &[
          ("left", &quota.left.map_or_else(String::new, percent)),
          ("share", &percent(ask.order.quota_share)),
        ],
      );

      return Ok(Err(Closed {
        reopens_at: quota.resets_at,
        ..Closed::plain(name, reason)
      }));
    }

    return Ok(Ok(quota.account.clone().or_else(|| open.first().map(|first| first.id.clone()))));
  }

  let carried = claude_accounts::carried_for_user(&ground.db, ask.user).await.map_err(refused)?;

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

/// Why a provider cannot answer now whatever the row, or none where it can: the owner has it off, it is
/// missing on this machine, or its balance is spent. What the warming asks before it warms a provider's
/// sessions, which leaves the logins to the account judgement it already walks.
pub async fn provider_closed(ground: &Ground, name: &str) -> Result<Option<String>, String> {
  let refused = |refused: crate::control::Refused| refused.error;
  let provider = harness::named(name).map_err(refused)?;

  if !harness::provider_enabled(&ground.db, name).await.map_err(refused)? {
    return Ok(Some(ROADS.reasons.off.clone()));
  }

  if let Some(missing) = harness::missing(&ground.db, &ground.machine(), provider).await.map_err(refused)? {
    return Ok(Some(missing.text));
  }

  Ok(balance_spent(ground, provider).await.map(|closed| closed.reason))
}

// ---- the facts of a row's road ----

/// What is kept of a row's road between its runs: the road it moved to, or that nothing could answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Kept {
  Moved { road: Road },
  Closed,
}

/// One fact of a row's road, published once (docs/api.md, 「Events」).
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

/// What a row's road comes to as work goes out on `took` — none where nothing could — against what was kept
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

/// A row's road moved off the head of its walk: the road it moved from and to, and why each provider passed
/// could not answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadMoved {
  pub fact: &'static str,
  pub supervisor: Option<String>,
  pub role: Option<String>,
  pub from: Option<Road>,
  pub to: Road,
  pub reasons: Vec<Closed>,
}

impl Event for RoadMoved {
  const NAME: &'static str = "road.changed";
}

/// A row's road back on the head of its walk after a move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadReturned {
  pub fact: &'static str,
  pub supervisor: Option<String>,
  pub role: Option<String>,
  pub from: Option<Road>,
  pub to: Road,
}

impl Event for RoadReturned {
  const NAME: &'static str = "road.changed";
}

/// Nothing in a row's order can answer: every provider's reason, what a person does to reopen the first
/// road a person can, and the moment the first that waits for one reopens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadClosed {
  pub fact: &'static str,
  pub supervisor: Option<String>,
  pub role: Option<String>,
  pub reasons: Vec<Closed>,
  pub command: Option<String>,
  pub reopens_at: Option<i64>,
}

impl Event for RoadClosed {
  const NAME: &'static str = "road.changed";
}

/// That a row's work goes out on `took` now — none where nothing in its order could answer — told once: the
/// fact it comes to published on the bus, and what is kept of the road written for the next run. `failed`
/// are the roads the work was tried on and that did not answer, beside the ones the resolution passed.
pub async fn went(ground: &Ground, resolution: &Resolution, took: Option<&Road>, failed: &[Closed]) {
  let db = &ground.db;
  let row = if resolution.supervisor {
    Row::Supervisor(&resolution.row)
  } else {
    Row::Role(&resolution.row)
  };
  let key = row.kept_key();
  let kept = match settings::get(db, &key).await {
    Ok(kept) => kept.and_then(|text| serde_json::from_str::<Kept>(&text).ok()),
    Err(why) => {
      return eprintln!("road {}: what was kept of it could not be read: {}", resolution.row, why.detail);
    }
  };
  let (fact, after) = fact(kept.as_ref(), resolution, took, failed);

  if after != kept {
    let written = match &after {
      Some(after) => settings::set(db, &key, &serde_json::to_string(after).unwrap_or_default()).await,
      None => settings::forget(db, &key).await,
    };

    if let Err(why) = written {
      eprintln!("road {}: what is kept of it was not written: {}", resolution.row, why.detail);
    }
  }

  let key = resolution.row.clone();
  let (supervisor, role) = if resolution.supervisor {
    (Some(key.clone()), None)
  } else {
    (None, Some(key.clone()))
  };

  match fact {
    None => {}
    Some(Fact::Moved { from, to, reasons }) => {
      println!("road {key}: moved to {}", to.words());
      ground.events.publish(RoadMoved {
        fact: "road-moved",
        supervisor,
        role,
        from,
        to,
        reasons,
      });
    }
    Some(Fact::Returned { from, to }) => {
      println!("road {key}: back on {}", to.words());
      ground.events.publish(RoadReturned {
        fact: "road-returned",
        supervisor,
        role,
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
        supervisor,
        role,
        reasons,
        command,
        reopens_at,
      });
    }
  }
}
