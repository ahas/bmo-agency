//! A turn put into a program's pane ending on its program's error (docs/agent-control.md, 「Telling that a turn's
//! program is gone」): a failed request the program reported through its failure hook ends the wait the moment
//! it is read, whatever herdr reads the pane as, and the error the program wrote in the answer's place once the
//! pane is no longer working.

use agency_server::door::turn::over_on_error;

#[test]
fn a_reported_failure_ends_the_wait_however_herdr_reads_the_pane() {
  // herdr can go on reading `working` for a look or two after the program gave the turn up.
  assert!(over_on_error(true, false, true), "reported while herdr reads working");
  assert!(over_on_error(false, false, true), "reported at rest");
  assert!(over_on_error(true, true, true));
}

#[test]
fn an_error_written_in_the_answer_s_place_ends_the_wait_once_the_pane_is_not_working() {
  assert!(!over_on_error(true, true, false), "still working on past it");
  assert!(over_on_error(false, true, false));
}

#[test]
fn a_turn_that_goes_on_past_a_retry_inside_its_program_is_untouched() {
  // No failure reported and no error in the record: a request the program retried and got past.
  assert!(!over_on_error(true, false, false));
  assert!(!over_on_error(false, false, false));
}
