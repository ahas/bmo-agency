//! When a worker's session is full: past the fold ratio of its model's window, and never on a turn that
//! reports no share.

use agency_server::worker_session::fold::worker_session_full;
use policy::resident_turn::RESIDENT_TURN;

#[test]
fn a_worker_session_is_full_past_the_fold_ratio_of_its_models_window_alone() {
  assert!(worker_session_full(Some(RESIDENT_TURN.fold_ratio)));
  assert!(!worker_session_full(Some(RESIDENT_TURN.fold_ratio - 0.01)));
  assert!(!worker_session_full(None));
}
