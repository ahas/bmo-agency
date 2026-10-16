//! Native `Agent` co-workers as a kind of place (docs/coworker-pool.md, 「Native co-worker places」):
//! their reservations hold seats and warm nothing. A native child's prefix is assembled by the parent CLI
//! inside its own session, and sending those bytes from outside it would take opening a Claude Code
//! session, which the live-claude guard refuses.

use policy::pool_warm::POOL_WARM;

use super::{Boxed, Place, PlaceKind};
use crate::control::pool::Reservation;
use crate::order_run::dispatch::NATIVE;
use crate::places::Ground;

pub struct NativePlaces;

impl PlaceKind for NativePlaces {
  fn answers(&self, source: &str) -> bool {
    source == NATIVE
  }

  fn refusal(&self) -> Option<&str> {
    Some(&POOL_WARM.native.refusal)
  }

  fn places<'a>(
    &'a self,
    _ground: &'a Ground,
    _reservation: &'a Reservation,
  ) -> Boxed<'a, Vec<Box<dyn Place>>> {
    Box::pin(async { Ok(vec![]) })
  }
}
