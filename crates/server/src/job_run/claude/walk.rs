//! The logins a Claude turn is carried by (docs/cache-fallback.md, 「The Claude Code road is a list of
//! accounts」): the logins the run is offered, in order, for the user it is made for, each carrying the
//! turn on its own session; a login that cannot answer is never offered, and a run none of whose logins
//! answers goes out on none. Where a turn fails on the login rather than on the work, the next login
//! carries it, and the turn hears of every login passed over with the words the run came back with: a full
//! window is put to rest as it is passed over; a login gone or refusing the run — a request for `/login`, a
//! 403 — is marked and published once as waiting for a person, and, where its credential would lift that
//! mark at once, rests alone for the provider's span so the credential's own moment cannot put it back in
//! the walk before then; the rest that closed the last login to a limit is published once too (docs/api.md, 「Events」). A run every login of which refused it ends on the login
//! failure naming each with its words, which a person settles by signing in again. A login near its
//! credential's end is renewed before the turn goes out on it, in a step of its own (`super::renewal`): one
//! short run of the CLI by one run at a time under the account's own lock, which no run holds across its
//! turn; a renewal that fails on the login, or that the run waited on and saw fail so, is that login's
//! failure like a turn's, and the walk carries the run on to the next login.

use std::future::Future;

use super::failure::{AccountFailure, Failure};
use super::renewal::{Renewing, renewed};
use crate::control::claude_accounts::{
  self, Account, Judging, account_open, carried_for_user, hold_in_use, in_use, reopens_at, walk_for_user,
};
use crate::keepalive::logins;
use crate::now_ms;
use crate::places::Ground;
use crate::rest::{self, DeploymentResting, RestKind};
use policy::providers::claude_code::CLAUDE_CODE;

// How long a full window rests where the run named no reset moment.
pub const REST_FALLBACK_MS: i64 = 3_600_000;

/// What the walk stands on: the ground, whose bus a login gone and the rest that silenced the deployment
/// are published on.
pub struct Walking<'a> {
  pub ground: &'a Ground,
}

impl Walking<'_> {
  fn judging(&self) -> Judging<'_> {
    Judging::of(self.ground)
  }
}

fn refused(why: crate::control::Refused) -> Failure {
  Failure::error(why.error)
}

/// One login the walk passed over for its own failure, as the turn hears of it: the login, what its failure
/// is, the words the run came back with, and the login the turn goes out on next, none where none is left.
pub struct PassedOver<'w> {
  pub account: &'w Account,
  pub failure: AccountFailure,
  pub said: &'w str,
  pub next: Option<&'w Account>,
}

/// What a turn does on one login of the walk, the login renewed first where it is near its end.
pub trait Attempt<T>: Renewing {
  /// The turn on this login, or on the run as it stands where none is named.
  fn attempt(&mut self, account: Option<Account>) -> impl Future<Output = Result<T, Failure>>;

  /// Whether this turn may go out on a login at all: one it may not is passed over without a run and
  /// without anything written under it. Every login, unless the turn says otherwise.
  fn takes(&self, _account: &Account) -> bool {
    true
  }

  /// What the turn makes of a login passed over for its own failure, before the next login carries it:
  /// nothing, unless the turn says otherwise.
  fn passed_over(&mut self, _passed: &PassedOver<'_>) -> impl Future<Output = ()> {
    async {}
  }
}

/// One turn carried by the logins its key is offered. The attempt is handed the login it goes out on,
/// or none where no login is registered for a run made for nobody, which is made once as it stands. A
/// run whose every login is closed goes out on none of them: it ends as the limit the first of them
/// reopens from, or as the login a person has to sign in again. A login the turn does not take is no
/// login of this walk; where the turn takes none of the logins carrying it, it goes out on none. A run
/// every login carried and failed ends on the last failure that was no refusal — a full window, a token
/// for the work nodes gone — or, where every one of them refused it, on the login failure naming each.
pub async fn on_accounts<T, A: Attempt<T>>(
  walking: &Walking<'_>,
  key: &str,
  user: Option<i64>,
  attempt: &mut A,
) -> Result<T, Failure> {
  let db = &walking.ground.db;
  let mut walk = walk_for_user(walking.judging(), user).await.map_err(refused)?;

  walk.retain(|account| attempt.takes(account));

  if walk.is_empty() {
    let carried = carried_for_user(db, user).await.map_err(refused)?;
    let (taken, passed): (Vec<Account>, Vec<Account>) =
      carried.into_iter().partition(|account| attempt.takes(account));

    if !taken.is_empty() {
      return Err(closed(walking, key, &taken).await?);
    }

    if !passed.is_empty() {
      let names = passed.iter().map(|account| account.id.as_str()).collect::<Vec<_>>().join(", ");

      return Err(Failure::Account {
        failure: AccountFailure::Login,
        detail: format!("claude {key}: no login that carries this run can carry this turn ({names})"),
      });
    }

    if let Some(user) = user {
      return Err(Failure::error(format!(
        "claude {key}: no Claude account of user {user} is there to carry this run"
      )));
    }

    return attempt.attempt(None).await;
  }

  // The last failure that was no refusal, and every login that refused the run with its words.
  let mut other: Option<Failure> = None;
  let mut refusals: Vec<(&Account, String)> = vec![];

  for (place, account) in walk.iter().enumerate() {
    let tried = match renewed(walking.ground, account, attempt).await {
      Ok(()) => attempt.attempt(Some(account.clone())).await,
      Err(failed) => Err(failed),
    };

    match tried {
      Ok(result) => {
        hold_where_moved(walking.ground, &account.id).await?;

        return Ok(result);
      }
      Err(mut failed) => {
        let failure = match failed.account_failure() {
          None | Some(AccountFailure::Outage) => return Err(failed),
          Some(failure) => failure,
        };
        // A full window is the login's the run went out on, named on the failure so whatever reads it
        // after the walk writes nothing under another.
        let resets_at = match &mut failed {
          Failure::Limit {
            resets_at,
            account: on,
            ..
          } => {
            *on = Some(account.id.clone());
            *resets_at
          }
          _ => None,
        };
        let untold = pass_over(walking, account, failure, resets_at).await?;

        // A silence the walk had no road to tell is left on the failure for whatever reads it.
        if let Failure::Limit { silenced, .. } = &mut failed {
          *silenced = untold;
        }

        let said = said_of(&failed, key);
        let next = walk.get(place + 1);

        println!(
          "claude {key}: {} {}{}",
          account.id,
          match failure {
            AccountFailure::Limit => "is at its ceiling".to_owned(),
            AccountFailure::NodeToken => "needs a token for the work nodes".to_owned(),
            _ => format!("refused the run ({said})"),
          },
          next.map_or_else(
            || ", and no login is left".to_owned(),
            |next| format!(", carried by {}", next.id)
          )
        );
        attempt
          .passed_over(&PassedOver {
            account,
            failure,
            said: &said,
            next,
          })
          .await;

        if failure == AccountFailure::Login {
          refusals.push((account, said));
        } else {
          other = Some(failed);
        }
      }
    }
  }

  Err(other.unwrap_or_else(|| every_refused(key, &refusals)))
}

// The words a login's failure came back with, without the work key the failure reading put in front of them.
fn said_of(failed: &Failure, key: &str) -> String {
  let message = failed.message();

  message.strip_prefix(&format!("{key}: ")).map_or_else(|| message.clone(), str::to_owned)
}

// What a run every login of which refused it comes to: the login failure naming each login with the words it
// refused in, which only a person signing in to each again settles.
fn every_refused(key: &str, refusals: &[(&Account, String)]) -> Failure {
  let each = refusals
    .iter()
    .map(|(account, said)| format!("{} answered \"{said}\"", account.words()))
    .collect::<Vec<_>>()
    .join("; ");

  Failure::Account {
    failure: AccountFailure::Login,
    detail: format!(
      "claude {key}: every Claude account of the road refused this run, and a person must sign in to each \
       again: {each}"
    ),
  }
}

// What a run every login of which is closed comes to, said once in the machine's log: the limit the first
// of them reopens from, or, where none rests, the login a person has to sign in again.
async fn closed(walking: &Walking<'_>, key: &str, carried: &[Account]) -> Result<Failure, Failure> {
  let reopens = reopens_at(walking.judging(), carried).await.map_err(refused)?;
  let names = carried.iter().map(|account| account.id.as_str()).collect::<Vec<_>>().join(", ");

  println!("claude {key}: no login answers now ({names}), none is launched");

  Ok(match reopens {
    Some(at) => Failure::Limit {
      resets_at: Some(at),
      account: None,
      silenced: false,
    },
    None => Failure::Account {
      failure: AccountFailure::Login,
      detail: format!("claude {key}: every login that carries this run waits for a person ({names})"),
    },
  })
}

// What a run that answered leaves behind: the login it went out on becomes the one in use where no login
// is in use or the one in use can no longer answer.
async fn hold_where_moved(ground: &Ground, account: &str) -> Result<(), Failure> {
  let db = &ground.db;
  let judging = Judging::of(ground);
  let now = now_ms();
  let held = in_use(db).await.map_err(refused)?;

  if held.as_deref() == Some(account) {
    return Ok(());
  }

  if let Some(held) = &held
    && account_open(judging, held, now).await.map_err(refused)?
  {
    return Ok(());
  }

  if !account_open(judging, account, now).await.map_err(refused)? {
    return Ok(());
  }

  hold_in_use(db, account).await.map_err(refused)
}

// What passing a login over leaves behind: a full window is a rest for that login alone; a login gone or
// refusing the run waits for a person, published once, and rests alone for the provider's span where its
// credential would lift that mark at once; and a token
// for the work nodes gone waits for a person on the nodes alone. It answers whether the rest it wrote
// silenced the deployment with nobody told, which a walk publishing on its bus never leaves.
async fn pass_over(
  walking: &Walking<'_>,
  account: &Account,
  failure: AccountFailure,
  resets_at: Option<i64>,
) -> Result<bool, Failure> {
  let db = &walking.ground.db;
  let now = now_ms();

  // A token for the nodes gone is the account's on a node alone: its login here still answers.
  if failure == AccountFailure::NodeToken {
    let roots = &walking.ground.roots;

    logins::mark_node_token_gone(db, account, &roots.data)
      .await
      .map_err(|why| Failure::error(why.detail))?;
    logins::tell_node_token(db, &walking.ground.events, account, roots, now)
      .await
      .map_err(|why| Failure::error(why.detail))?;

    return Ok(false);
  }

  if failure == AccountFailure::Login {
    logins::mark_gone(db, &account.id, now)
      .await
      .map_err(|why| Failure::error(why.detail))?;

    let held = claude_accounts::accounts(db)
      .await
      .map_err(refused)?
      .into_iter()
      .find(|one| one.id == account.id);

    if let Some(held) = held {
      logins::tell(db, &walking.ground.events, &held, now)
        .await
        .map_err(|why| Failure::error(why.detail))?;
    }

    // A subscription's credential states a moment ahead at almost any time, which lifts the mark at once:
    // the rest is what keeps such a login out of the walk until a person can have signed in. A credential past
    // its moment, and an endpoint's key, hold the mark themselves.
    if !account.login_back(now) {
      return Ok(false);
    }

    let until = (now + CLAUDE_CODE.login.refused_rest_minutes * 60_000).div_euclid(1000);

    return write_rest(walking.ground, &account.id, RestKind::Refused, until).await;
  }

  if failure != AccountFailure::Limit {
    return Ok(false);
  }

  let until = resets_at.unwrap_or_else(|| (now + REST_FALLBACK_MS).div_euclid(1000));

  write_rest(walking.ground, &account.id, RestKind::Limit, until).await
}

/// A login's own rest — a full window's, or a refusal's — written under that login by the one writer of rests
/// (`crate::rest`), which leaves it as it stands where that login already rests. Where a full window's is
/// the rest that closed the last open login, it is published once as `deployment-resting`, and the consumer
/// holding the owner's chat says it; a refusal's is told as the login notice tells it. The answer is whether
/// that silence is still to be told, which once published it never is.
pub async fn write_rest(
  ground: &Ground,
  account: &str,
  kind: RestKind,
  until: i64,
) -> Result<bool, Failure> {
  let silenced = rest::write_rest(Judging::of(ground), until as f64, kind, account, now_ms() / 1000)
    .await
    .map_err(|why| Failure::error(why.detail))?;

  if silenced && kind == RestKind::Limit {
    ground.events.publish(DeploymentResting::of(kind, account, until));
  }

  Ok(false)
}
