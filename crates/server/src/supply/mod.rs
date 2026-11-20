//! A person's supply inside a road (docs/supply.md): the supply hook, the source a run takes as it starts, its
//! reservations and their settlement, the meter that extends them while the run goes and finds a source run out,
//! and the facts told. A run of no account or of the administrator's account takes no source of a person's and is
//! only recorded; every rule here holds for the runs of the other accounts.

pub mod hook;
pub mod meter;
pub mod reservations;
pub mod told;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use policy::fill_marks;
use policy::supply::SUPPLY;
use store::{one, run};

use crate::Refusal;
use crate::control::claude_accounts::{Carrying, Judging, moment_words, walk_for_user};
use crate::places::Ground;
pub use hook::Kind;

/// The source of a person's own credential.
pub const PERSONAL: &str = "personal";
/// The source the deployment's own runs go out on, which reserve nothing: the platform's.
pub const PLATFORM: &str = "platform";
/// The Claude Code provider, whose credentials are its accounts.
const CLAUDE_CODE: &str = "claude-code";
/// The provider whose models are the deployment's own registrations, which no person brings.
const REGISTERED: &str = "registered";

/// The source a run goes out on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
  /// A run of no account or of the administrator's: the platform's credentials, reserved nowhere.
  Deployment,
  /// The person's own credential: a Claude login or key of theirs, or their own key of a gateway road.
  Personal,
  /// A gateway road's member or a registration the person owns, which no key of theirs carries
  /// (docs/supply.md, 「Owned supply」).
  Owned,
  /// A platform source, by the name the hook gave it, with the store's row of the reservation it goes out
  /// on.
  Platform { name: String, reservation: i64 },
}

impl Source {
  /// The word the ledger keeps of it.
  pub fn word(&self) -> &str {
    match self {
      Self::Deployment => PLATFORM,
      Self::Personal | Self::Owned => PERSONAL,
      Self::Platform { name, .. } => name,
    }
  }
}

/// A run's supply as it was chosen: the key its asks are made under, the person, the road and the source, and the
/// order the hook gave, which a source that runs out hands on along.
#[derive(Clone, Debug, PartialEq)]
pub struct Chosen {
  pub run: String,
  pub account: Option<i64>,
  pub provider: String,
  pub model: String,
  pub source: Source,
  pub order: Vec<String>,
  /// The next step a money ask of the run takes.
  pub step: i64,
}

impl Chosen {
  /// The deployment's own run: on the platform's credentials, reserved nowhere.
  pub fn deployment(run: String, provider: &str, model: &str) -> Self {
    Self {
      run,
      account: None,
      provider: provider.to_owned(),
      model: model.to_owned(),
      source: Source::Deployment,
      order: Vec::new(),
      step: 0,
    }
  }
}

/// What the choice comes to: a source to go out on, or none left with the sentence that says so.
#[derive(Clone, Debug, PartialEq)]
pub enum Choice {
  Go(Chosen),
  Closed(Closed),
}

/// No source left for a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Closed {
  pub source: String,
  pub reason: String,
  pub reopens_at: Option<i64>,
  pub sentence: String,
}

impl Closed {
  /// No source left, the last one tried named with why, and the one sentence a person is told.
  pub fn of(source: &str, reason: String, reopens_at: Option<i64>) -> Self {
    let reopens = reopens_at.map_or_else(String::new, |at| {
      fill_marks(&SUPPLY.words.reopens, &[("at", &moment_words(at / 1000))])
    });
    let sentence = fill_marks(
      &SUPPLY.words.closed,
      &[("source", source), ("reopens", &reopens), ("settings", &SUPPLY.words.settings_section)],
    );

    Self {
      source: source.to_owned(),
      reason,
      reopens_at,
      sentence,
    }
  }
}

/// What a run asks as it starts.
#[derive(Clone, Debug)]
pub struct Asked<'a> {
  pub kind: Kind,
  /// The run's own number: the job's id, or the ledger run a supervisor's turn opened.
  pub id: i64,
  pub agent: &'a str,
  pub account: Option<i64>,
  pub provider: &'a str,
  pub model: &'a str,
  pub job: Option<i64>,
  pub supervisor: Option<&'a str>,
  pub room: Option<i64>,
}

// Each agent's supply as its last run chose it, by the data root it was chosen under and the agent, so two
// stores a process serves at once never read each other's.
static CHOSEN: Mutex<Option<HashMap<(PathBuf, String), Chosen>>> = Mutex::new(None);

fn chosen_map<T>(work: impl FnOnce(&mut HashMap<(PathBuf, String), Chosen>) -> T) -> T {
  let mut held = CHOSEN.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

  work(held.get_or_insert_with(HashMap::new))
}

/// The supply an agent's run goes out on now, where its run chose one.
pub fn chosen_for(data_root: &Path, agent: &str) -> Option<Chosen> {
  chosen_map(|map| map.get(&(data_root.to_path_buf(), agent.to_owned())).cloned())
}

/// The name a job's supply is kept under beside its checkout's.
pub fn job_key(job: i64) -> String {
  format!("job:{job}")
}

/// The supply an agent's run goes out on from now.
pub fn remember(data_root: &Path, agent: &str, chosen: &Chosen) {
  chosen_map(|map| map.insert((data_root.to_path_buf(), agent.to_owned()), chosen.clone()));
}

/// A run's supply kept for its agent, and a person's source that is not the one their agent's last run took
/// told as a move (docs/supply.md, 「The facts told」).
pub async fn settle_choice(
  ground: &Ground,
  agent: &str,
  chosen: &Chosen,
  job: Option<i64>,
  told: &told::Told,
) {
  let before = chosen_for(&ground.roots.data, agent);

  remember(&ground.roots.data, agent, chosen);

  let Some(before) = before.filter(|before| chosen.account.is_some() && before.account == chosen.account)
  else {
    return;
  };

  if before.source.word() != chosen.source.word() {
    let why = fill_marks(&SUPPLY.words.taken_first, &[("source", chosen.source.word())]);

    told::moved_on(ground, chosen, job, told, before.source.word(), chosen.source.word(), &why).await;
  }
}

/// The person a run is for, where it is for a person other than the administrator; none for the deployment's
/// own runs.
pub async fn person(ground: &Ground, account: Option<i64>) -> Result<Option<i64>, Refusal> {
  let Some(account) = account else { return Ok(None) };

  if crate::account_isolation::owners_own(&ground.db, &account.to_string()).await? {
    return Ok(None);
  }

  Ok(Some(account))
}

/// The account a named agent's runs are for: the supervisor's, else the role agent's.
pub async fn account_of(ground: &Ground, agent: &str) -> Result<Option<i64>, Refusal> {
  let acting = crate::person_nodes::acting_for(
    &ground.db,
    &ground.roots.data,
    crate::person_nodes::Caller {
      agent,
      role: None,
      checkout: None,
    },
  )
  .await?;

  Ok(acting.account.and_then(|account| account.parse().ok()))
}

/// The Claude logins a run walks, for the supply it goes out on: a person's own on personal supply, the
/// lendable ones on a platform source, and the platform's and the administrator's for the deployment's own.
pub fn carrying(chosen: Option<&Chosen>) -> Carrying {
  match chosen.map(|chosen| (&chosen.source, chosen.account)) {
    Some((Source::Personal | Source::Owned, Some(account))) => Carrying::Own(account),
    Some((Source::Platform { .. }, Some(_))) => Carrying::Lent,
    _ => Carrying::Platform,
  }
}

/// The name a gateway road's run is sent under on the person's own key: its member's name marked with the
/// person (docs/own-llm.md, 「Personal members」); none where the run goes out on the member's own name, the
/// platform's or one the person owns.
pub fn personal_member(chosen: Option<&Chosen>, name: &str) -> Option<String> {
  match chosen.map(|chosen| (&chosen.source, chosen.account)) {
    Some((Source::Personal, Some(account))) => Some(format!("{name}@u{account}")),
    _ => None,
  }
}

/// The account that owns the model a run on the road goes out on, none for the platform's: a gateway road's
/// member by the members file, a registration by its own row; a Claude account's owner is read by the walk
/// of its logins (docs/supply.md, 「Owned supply」).
pub async fn owner_of(ground: &Ground, provider: &str, model: &str) -> Result<Option<i64>, Refusal> {
  match provider {
    CLAUDE_CODE => Ok(None),
    REGISTERED => Ok(
      one(
        &ground.db,
        r#"SELECT "account" FROM "registered_models" WHERE "name" = $1"#,
        vec![model.into()],
      )
      .await?
      .map(|row| row.try_get::<Option<i64>>("", "account"))
      .transpose()?
      .flatten(),
    ),
    road => Ok(
      harness_providers::member::member_named(&ground.machine(), road, model)
        .map_err(Refusal::failed)?
        .and_then(|member| member.account),
    ),
  }
}

/// The person's own supply open for the road's provider, where one is: a Claude login or key that can answer
/// now; on a gateway road the road's member they own, else an open key of the road; or the registration they
/// own.
pub async fn personal_source(
  ground: &Ground,
  account: i64,
  provider: &str,
  model: &str,
) -> Result<Option<Source>, Refusal> {
  if provider == CLAUDE_CODE {
    let open = !walk_for_user(Judging::of(ground), Carrying::Own(account), Some(model))
      .await
      .map_err(|refused| Refusal::failed(refused.error))?
      .is_empty();

    return Ok(open.then_some(Source::Personal));
  }

  if owner_of(ground, provider, model).await? == Some(account) {
    return Ok(Some(Source::Owned));
  }

  if provider == REGISTERED {
    return Ok(None);
  }

  let keyed = one(
    &ground.db,
    r#"SELECT 1 AS "open" FROM "personal_supply"
       WHERE "user_id" = $1 AND "road" = $2 AND "enabled" AND "refused_at" IS NULL LIMIT 1"#,
    vec![account.into(), provider.into()],
  )
  .await?
  .is_some();

  Ok(keyed.then_some(Source::Personal))
}

/// Whether the platform's credentials can carry a person's run on the road: for Claude Code, an open
/// lendable account; on every other road, a model no person owns.
pub async fn platform_open(ground: &Ground, provider: &str, model: &str) -> Result<bool, Refusal> {
  if provider != CLAUDE_CODE {
    return Ok(owner_of(ground, provider, model).await?.is_none());
  }

  Ok(
    !walk_for_user(Judging::of(ground), Carrying::Lent, Some(model))
      .await
      .map_err(|refused| Refusal::failed(refused.error))?
      .is_empty(),
  )
}

/// The order a person's run walks: the hook's, or personal supply alone where nobody is registered.
async fn order_of(ground: &Ground, asked: &Asked<'_>, run: &str, account: i64) -> Vec<String> {
  let ask = hook::OrderAsk {
    run: run.to_owned(),
    account,
    provider: asked.provider.to_owned(),
    model: asked.model.to_owned(),
    kind: asked.kind,
    job: asked.job,
    supervisor: asked.supervisor.map(str::to_owned),
    room: asked.room,
  };

  hook::order(&ground.hooks, ask).await.unwrap_or_else(|| vec![PERSONAL.to_owned()])
}

/// The key a run's money asks are made under: its kind, its number and the moment it chose, so a run after a
/// restart never takes an earlier run's key.
fn run_key(asked: &Asked<'_>) -> String {
  let kind = match asked.kind {
    Kind::Job => "job",
    Kind::Turn => "turn",
  };

  format!("{kind}-{}-{}", asked.id, crate::now_ms())
}

/// The source a run takes as it starts (docs/supply.md, 「Choosing a source as a run starts」): the deployment's
/// own runs the platform's, unreserved; a person's the first of their order that can take it inside the road,
/// a platform source only once the hook reserved it, and none where nothing is left.
pub async fn choose(ground: &Ground, asked: &Asked<'_>) -> Result<Choice, Refusal> {
  let run = run_key(asked);
  let Some(account) = person(ground, asked.account).await? else {
    return Ok(Choice::Go(Chosen::deployment(run, asked.provider, asked.model)));
  };
  let order = order_of(ground, asked, &run, account).await;
  let mut chosen = Chosen {
    run,
    account: Some(account),
    provider: asked.provider.to_owned(),
    model: asked.model.to_owned(),
    source: Source::Personal,
    order: order.clone(),
    step: 1,
  };

  match next_source(ground, &mut chosen, &order).await? {
    Ok(()) => Ok(Choice::Go(chosen)),
    Err(closed) => Ok(Choice::Closed(closed)),
  }
}

/// The first source of `sources` that can take the run, written on it; where none can, why the last one tried
/// could not.
pub async fn next_source(
  ground: &Ground,
  chosen: &mut Chosen,
  sources: &[String],
) -> Result<Result<(), Closed>, Refusal> {
  let Some(account) = chosen.account else {
    chosen.source = Source::Deployment;
    return Ok(Ok(()));
  };
  let mut last = Closed::of(PLATFORM, SUPPLY.words.no_order.clone(), None);

  for source in sources {
    if source == PERSONAL {
      if let Some(personal) = personal_source(ground, account, &chosen.provider, &chosen.model).await? {
        chosen.source = personal;
        return Ok(Ok(()));
      }

      let reason = fill_marks(&SUPPLY.words.no_personal, &[("provider", &chosen.provider)]);

      last = Closed::of(PERSONAL, reason, None);
      continue;
    }

    if !platform_open(ground, &chosen.provider, &chosen.model).await? {
      let reason = if chosen.provider == CLAUDE_CODE {
        fill_marks(&SUPPLY.words.no_lendable, &[("provider", &chosen.provider)])
      } else {
        fill_marks(&SUPPLY.words.owned, &[("model", &chosen.model)])
      };

      last = Closed::of(source, reason, None);
      continue;
    }

    match reservations::reserve(ground, chosen, source).await? {
      Ok(row) => {
        chosen.source = Source::Platform {
          name: source.clone(),
          reservation: row,
        };
        return Ok(Ok(()));
      }
      Err(reopens_at) => {
        last = Closed::of(source, fill_marks(&SUPPLY.words.refused, &[("source", source)]), reopens_at);
      }
    }
  }

  Ok(Err(last))
}

/// The sources of a run's order after the one it goes out on now, the platform's alone: where an extension
/// refused hands the run on along (docs/supply.md, 「Running out mid-turn」).
pub fn platform_after(chosen: &Chosen) -> Vec<String> {
  let now = chosen.source.word().to_owned();
  let at = chosen.order.iter().position(|source| *source == now).map_or(0, |at| at + 1);

  chosen.order[at.min(chosen.order.len())..]
    .iter()
    .filter(|source| *source != PERSONAL)
    .cloned()
    .collect()
}

/// The sources of a run's order after personal supply, for a run whose own credential was refused.
pub fn after_personal(chosen: &Chosen) -> Vec<String> {
  chosen.order.iter().filter(|source| *source != PERSONAL).cloned().collect()
}

/// A run on a model the person owns that refused it, passed over to the rest of its order: the next source
/// after personal supply that can take it, or why none can (docs/supply.md, 「Owned supply」).
pub async fn pass_owned(ground: &Ground, chosen: &Chosen) -> Result<Choice, Refusal> {
  let mut next = chosen.clone();

  Ok(match next_source(ground, &mut next, &after_personal(chosen)).await? {
    Ok(()) => Choice::Go(next),
    Err(closed) => Choice::Closed(closed),
  })
}

/// A person's own key for a gateway road found refused by its provider, where the run went out on one: it waits
/// for its person, who is told once, and its next key or the next source is taken (docs/supply.md, 「Running out
/// mid-turn」). A run on a model the person owns, or on a Claude login, marks no key. Answers the key marked.
pub async fn refuse_personal_key(ground: &Ground, chosen: &Chosen) -> Result<Option<i64>, Refusal> {
  let (Source::Personal, Some(account)) = (&chosen.source, chosen.account) else {
    return Ok(None);
  };
  if chosen.provider == CLAUDE_CODE {
    return Ok(None);
  }
  let road = chosen.provider.as_str();
  let Some(row) = one(
    &ground.db,
    r#"SELECT "id" FROM "personal_supply"
       WHERE "user_id" = $1 AND "road" = $2 AND "enabled" AND "refused_at" IS NULL ORDER BY "place" LIMIT 1"#,
    vec![account.into(), road.into()],
  )
  .await?
  else {
    return Ok(None);
  };
  let id: i64 = row.try_get("", "id")?;

  run(
    &ground.db,
    r#"UPDATE "personal_supply" SET "refused_at" = $2 WHERE "id" = $1"#,
    vec![id.into(), crate::now_ms().into()],
  )
  .await?;

  Ok(Some(id))
}
