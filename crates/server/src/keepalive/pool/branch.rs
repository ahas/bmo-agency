//! The standing branch checkouts as a kind of place the pool warms for a co-work reservation
//! (docs/coworker-pool.md, 「Standing branch checkouts and their list」). Which pool the reservation's places
//! stand in, and which of them its owner's tasks sit in now, the consumer answers through the co-work
//! places hook; agency reads none of its orders. The candidates are the list's rows for the reserved
//! provider and model whose place still stands; a Claude Code row whose prefix has moved names a cache
//! nobody can read and is no candidate. The ping is the worker's own line in that place, keeping no
//! session, and an answer writes the row again with the prefix it read.

use std::sync::Arc;

use policy::fill_marks;
use policy::pool_warm::POOL_WARM;

use super::{Boxed, Place, PlaceKind, PlaceRun, WarmPlace};
use crate::Refusal;
use crate::control::claude_accounts;
use crate::control::pool::Reservation;
use crate::hooks::HookPoint;
use crate::lines::standing::worker_prefix_digest;
use crate::lines::worker::{configured_for, worker_run};
use crate::lines::{Trees, fault};
use crate::order_run::dispatch::COWORK;
use crate::order_run::places::{Agent, Held, agents, busy_slots, record, set_agent};
use crate::places::Ground;
use crate::places::branch::{self, place_agent, place_label, pool_of, pool_slots, slot_dir};
use crate::places::locks::lock_path;
use crate::queue::routes::JOB_WORK_KEY;

// The provider whose rows are judged by the prefix they name: the one prefix agency assembles itself and
// can therefore measure.
const DIGESTED: &str = "claude-code";

/// What the consumer whose co-work reserves answers for a reservation's owner: the checkout its order works
/// in, which sets the pool, and the places its tasks sit in now.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct OwnerPlaces {
  pub cwd: String,
  pub held: Vec<Held>,
}

/// The co-work places, for a reservation (docs/api.md, 「Hooks」): asked with the reservation's owner as
/// it stands. The default answers none, and that reservation's places are not warmed.
pub struct CoworkPlaces;

impl HookPoint for CoworkPlaces {
  type Ask = String;
  type Answer = Option<OwnerPlaces>;
  const NAME: &'static str = "cowork-places";

  fn default_answer(_owner: String) -> Option<OwnerPlaces> {
    None
  }
}

crate::wired!(CoworkPlaces);

pub struct BranchPlaces;

struct BranchPlace {
  warm: WarmPlace,
  pool: String,
  slot: i64,
  role: String,
  held: Arc<Vec<Held>>,
}

// The worker's line in the place, laid on the tier its reservation's work key resolves to on the row's provider;
// it keeps no session.
async fn branch_place(
  ground: &Ground,
  trees: &Trees,
  pool: &str,
  row: &Agent,
  work_key: &str,
  held: &Arc<Vec<Held>>,
) -> Result<BranchPlace, Refusal> {
  let cwd = slot_dir(&ground.roots.data, pool, row.slot).to_string_lossy().into_owned();
  // A Claude Code place is warmed on the login the walk offers now, so its prefix is that login's base.
  let account = if row.mode == DIGESTED {
    claude_accounts::walk_for(claude_accounts::Judging::of(ground), JOB_WORK_KEY)
      .await
      .map_err(|refused| fault(refused.error))?
      .into_iter()
      .next()
  } else {
    None
  };
  let line = worker_run(ground, trees, None, account.as_ref(), &row.model, &cwd).await?;
  let line = configured_for(ground, &ground.machine(), line, work_key, &row.mode).await?;
  let upstream = (!row.upstream.is_empty()).then(|| row.upstream.clone());
  let key = format!(
    "cowork:{pool}:{}:{}:{}{}",
    row.slot,
    row.mode,
    row.model,
    upstream.as_ref().map(|upstream| format!(":{upstream}")).unwrap_or_default()
  );
  let label = place_label(pool, row.slot);

  Ok(BranchPlace {
    warm: WarmPlace {
      key,
      provider: row.mode.clone(),
      model: row.model.clone(),
      effort: Some(line.effort.clone()),
      upstream,
      last_write_at: row.last_reply_at * 1000,
      work_key: JOB_WORK_KEY.to_owned(),
      name: format!("bmo-branch-warm-{}", row.slot),
      title: fill_marks(&POOL_WARM.branch.title, &[("place", &label)]),
      usage_name: format!("branch-agent-{pool}-{}", row.slot),
      lock: lock_path(&ground.roots.data, &cwd),
      // The worker line pings in the place's own pane, where its branches run.
      run: PlaceRun {
        agent: place_agent(pool, row.slot),
        home: branch::home(ground, &cwd, None).map(Box::new),
        line: Box::new(line),
      },
    },
    pool: pool.to_owned(),
    slot: row.slot,
    role: row.role.clone(),
    held: held.clone(),
  })
}

impl Place for BranchPlace {
  fn warm(&self) -> &WarmPlace {
    &self.warm
  }

  fn busy<'a>(&'a self, _ground: &'a Ground) -> Boxed<'a, bool> {
    Box::pin(async move { Ok(busy_slots(Some(&self.pool), &self.held).await.contains(&self.slot)) })
  }

  // The row written again: a provider with no published lifetime keeps none, and a Claude Code row is kept
  // with the prefix it read or not at all.
  fn warmed<'a>(&'a self, ground: &'a Ground, at: i64) -> Boxed<'a, ()> {
    Box::pin(async move {
      let warm = &self.warm;
      let line = &warm.run.line;
      let digest = if warm.provider == DIGESTED {
        worker_prefix_digest(ground, &Trees::of(ground), &line.cwd, &warm.model, Some(&line.effort)).await?
      } else {
        String::new()
      };
      let written = record(
        self.slot,
        &warm.provider,
        &warm.model,
        &self.role,
        warm.upstream.as_deref(),
        Some(&digest),
        at,
      );

      match written {
        Some(row) => set_agent(&ground.db, &self.pool, &row).await.map_err(fault),
        None => Ok(()),
      }
    })
  }
}

impl PlaceKind for BranchPlaces {
  fn answers(&self, source: &str) -> bool {
    source == COWORK
  }

  fn places<'a>(
    &'a self,
    ground: &'a Ground,
    reservation: &'a Reservation,
  ) -> Boxed<'a, Vec<Box<dyn Place>>> {
    Box::pin(async move {
      let Some(owner) = ground.hooks.ask::<CoworkPlaces>(reservation.owner.clone()).await else {
        return Ok(vec![]);
      };
      let pool = pool_of(&owner.cwd).await;
      let slots = pool_slots(&ground.roots.data, &pool);
      let trees = Trees::of(ground);
      let held = Arc::new(owner.held);
      let mut places: Vec<Box<dyn Place>> = Vec::new();

      for row in agents(&ground.db, &pool).await.map_err(fault)? {
        if row.mode != reservation.provider || row.model != reservation.model || !slots.contains(&row.slot)
        {
          continue;
        }

        let place = branch_place(ground, &trees, &pool, &row, &reservation.work_key, &held).await?;
        let line = &place.warm.run.line;

        // A mapping that now resolves elsewhere opens on another prefix than the one the row names.
        if line.model != row.model {
          continue;
        }

        if row.mode == DIGESTED
          && row.prefix_digest
            != worker_prefix_digest(ground, &trees, &line.cwd, &row.model, Some(&line.effort)).await?
        {
          continue;
        }

        places.push(Box::new(place));
      }

      Ok(places)
    })
  }
}
