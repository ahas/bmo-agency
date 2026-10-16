//! What the resident door writes as a turn goes (docs/agent-control.md, 「The order for putting in one
//! turn and waiting for it to end」): a turn's notes, the headings its kept screens are filed under, what
//! is written of a confirmation's answer, the status line at the foot of a resident program's screen, and
//! why a turn on a provider switched off is refused.

use serde::Deserialize;

/// What a turn writes into its run's log as it goes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notes {
  pub title: String,
  pub trust_written: String,
  pub trust_unwritten: String,
  pub trusted_folder: String,
  pub person_turn: String,
  pub stand_again: String,
  pub untrusted: String,
  pub not_ready: String,
  pub not_taken: String,
  pub herdr_silent_ready: String,
  pub enter_pressed: String,
  pub unsubmitted: String,
  pub overrun_unasked: String,
  pub herdr_silent: String,
  pub herdr_gave_up: String,
  pub herdr_back: String,
  pub quiet: String,
  pub error_menu: String,
}

/// The heading a kept screen is filed under as a turn ends without its answer.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screens {
  pub vanished: String,
  pub not_ready: String,
  pub not_taken: String,
  pub unsubmitted: String,
  pub stopped: String,
  pub overran: String,
  pub silent: String,
  pub unknown_window: String,
}

/// What is written of a confirmation put up in the middle of a turn and how it was answered.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Confirm {
  pub mark: String,
  pub left_to_person: String,
  pub no_allow: String,
  pub allowed: String,
  pub refused_with_reason: String,
  pub refused: String,
  pub closed: String,
  pub tries: String,
  pub again: String,
  pub answered: String,
  pub changed: String,
  pub repeated: String,
  pub closed_early: String,
  pub stopped_at: String,
  pub unasked: String,
}

/// The one line at the foot of a resident program's screen.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusLine {
  pub resting: String,
  pub person: String,
  pub running: String,
  pub rested_after_person: String,
  pub rested_after_run: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoorWords {
  pub provider_off: String,
  pub notes: Notes,
  pub screens: Screens,
  pub confirm: Confirm,
  pub status_line: StatusLine,
}

crate::part!(pub DOOR_WORDS: DoorWords = "door-words.toml");
