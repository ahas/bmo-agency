//! The logins a Claude turn is carried by (docs/cache-fallback.md, 「The Claude Code road is a list of
//! accounts」): the logins its work key is offered, in order, for the user it is made for, each carrying the
//! turn on its own session; a login that cannot answer is never offered, and a run none of whose logins
//! answers goes out on none. Where a turn fails on the login rather than on the work, the next login
//! carries it: a full window is put to rest as it is passed over, and a login that is gone is marked and
//! published once, as is the rest that closed the last login (docs/api.md, 「Events」). A login near its
//! credential's end is renewed by one run at a time, under the account's own lock.

use std::future::Future;
use std::time::Duration;

use super::failure::{AccountFailure, Failure};
use crate::control::claude_accounts::{
  self, Account, Judging, account_open, carried_for_user, credential_expiry, hold_in_use, in_use,
  pinned_account, reopens_at, walk_for_user,
};
use crate::keepalive::logins;
use crate::now_ms;
use crate::places::{Ground, locks};
use crate::rest::{self, DeploymentResting, RestKind};

// How close to its end a login has to be for a run to take the account's lock on the way out.
const REFRESH_WINDOW_MS: f64 = 30.0 * 60_000.0;

// How long a run waits for another run renewing the same login.
const REFRESH_WAIT: Duration = Duration::from_secs(30);

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

/// What a turn does on one login of the walk.
pub trait Attempt<T> {
  /// The turn on this login, or on the run as it stands where none is named.
  fn attempt(&mut self, account: Option<Account>) -> impl Future<Output = Result<T, Failure>>;

  /// Whether this turn may go out on a login at all: one it may not is passed over without a run and
  /// without anything written under it. Every login, unless the turn says otherwise.
  fn takes(&self, _account: &Account) -> bool {
    true
  }
}

/// One turn carried by the logins its key is offered. The attempt is handed the login it goes out on,
/// or none where no login is registered for a run made for nobody, which is made once as it stands. A
/// run whose every login is closed goes out on none of them: it ends as the limit the first of them
/// reopens from, or as the login a person has to sign in again. A login the turn does not take is no
/// login of this walk; where the turn takes none of the logins carrying it, it goes out on none.
pub async fn on_accounts<T, A: Attempt<T>>(
  walking: &Walking<'_>,
  key: &str,
  user: Option<i64>,
  attempt: &mut A,
) -> Result<T, Failure> {
  let db = &walking.ground.db;
  let mut walk = walk_for_user(walking.judging(), key, user).await.map_err(refused)?;
  let pinned = pinned_account(db, key).await.map_err(refused)?.is_some();

  walk.retain(|account| attempt.takes(account));

  if walk.is_empty() {
    let carried = carried_for_user(db, key, user).await.map_err(refused)?;
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

  let mut last = Failure::error(format!("claude {key}: no login answered"));

  for (place, account) in walk.iter().enumerate() {
    let tried = renewed(&walking.ground.roots.data, account, attempt.attempt(Some(account.clone()))).await;

    match tried {
      Ok(result) => {
        if !pinned {
          hold_where_moved(walking.ground, &account.id).await?;
        }

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

        last = failed;

        println!(
          "claude {key}: {} {}{}",
          account.id,
          match failure {
            AccountFailure::Limit => "is at its ceiling",
            AccountFailure::NodeToken => "needs a token for the work nodes",
            _ => "needs a login",
          },
          walk.get(place + 1).map_or_else(
            || ", and no login is left".to_owned(),
            |next| format!(", carried by {}", next.id)
          )
        );
      }
    }
  }

  Err(last)
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

/// The login is renewed by one run and not by several: where its credential is close to its end, this run
/// takes the account's own lock on the way out and the others wait for what it wrote.
pub async fn renewed<T>(
  data_root: &std::path::Path,
  account: &Account,
  go: impl Future<Output = Result<T, Failure>>,
) -> Result<T, Failure> {
  let due =
    || credential_expiry(account).is_some_and(|expiry| expiry - now_ms() as f64 <= REFRESH_WINDOW_MS);

  if !due() {
    return go.await;
  }

  let path = data_root.join("locks").join(format!("claude-account-{}.lock", account.id));
  let Some(held) = locks::hold_within(path, REFRESH_WAIT).await else {
    // Nobody released it in time; the run goes out rather than being lost.
    eprintln!("claude account {}: waited out the renewal lock, going out anyway", account.id);

    return go.await;
  };

  if !due() {
    println!("claude account {}: another run renewed its login, using what it wrote", account.id);
  }

  let result = go.await;

  drop(held);
  result
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

// What passing a login over leaves behind: a full window is a rest for that login alone, and a login that
// is gone, or its token for the work nodes, waits for a person, published once. It answers whether the rest
// it wrote silenced the deployment with nobody told, which a walk publishing on its bus never leaves.
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

    return Ok(false);
  }

  if failure != AccountFailure::Limit {
    return Ok(false);
  }

  let until = resets_at.unwrap_or_else(|| (now + REST_FALLBACK_MS).div_euclid(1000));

  write_rest(walking.ground, &account.id, until).await
}

/// A full window's rest written under the login that reached its ceiling, by the one writer of rests
/// (`crate::rest`), which leaves it as it stands where that login already rests. Where it is the rest that
/// closed the last open login, it is published once as `deployment-resting`, and the consumer holding the
/// owner's chat says it; the answer is whether that silence is still to be told, which once published it
/// never is.
pub async fn write_rest(ground: &Ground, account: &str, until: i64) -> Result<bool, Failure> {
  let silenced =
    rest::write_rest(Judging::of(ground), until as f64, RestKind::Limit, account, now_ms() / 1000)
      .await
      .map_err(|why| Failure::error(why.detail))?;

  if silenced {
    ground.events.publish(DeploymentResting::of(RestKind::Limit, account, until));
  }

  Ok(false)
}
