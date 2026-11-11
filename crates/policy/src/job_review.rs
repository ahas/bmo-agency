//! How a job's reports are examined by its orderer before the job goes on or ends (docs/job-review.md).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobReview {
  /// How many times the work is sent back before it may be put to the owner.
  pub escalate_after: i64,
  /// How long the owner's question waits before the job gives its checkout up.
  pub owner_wait_seconds: i64,
  /// The name the record's own entries are written under.
  pub actor: String,
  /// The checks every report is examined by, in order.
  pub checks: Vec<Check>,
  /// What the orderer is told of the check due now.
  pub review_order: String,
  /// The moves an order names before the work may be put to the owner.
  pub moves_early: String,
  /// The moves an order names once it may be.
  pub moves_open: String,
  /// The heading of a report under review.
  pub report_heading: String,
  /// The heading of an answer under review.
  pub answer_heading: String,
  /// The heading of the owner's word under review.
  pub ruling_heading: String,
  /// The stage of a job of one stage.
  pub stage_one: String,
  /// A stage of a job of several: `{stage}`, `{stages}` and `{step}`.
  pub stage_of: String,
  /// The final report of a job of several stages.
  pub stage_final: String,
  /// A job that gave its checkout up while the owner's question waits.
  pub parked_order: String,
  /// The owner's question.
  pub question_text: String,
  /// The owner's choice to send the work back.
  pub send_back: String,
  /// The owner's choice to take it.
  pub take_it: String,
  /// The review opened.
  pub opened: String,
  /// What the first turn of a job of several stages is told.
  pub first_stage: String,
  /// What every turn of a job of one stage is told.
  pub one_stage: String,
  /// A stage handed over.
  pub next_stage: String,
  /// The mark of the last stage.
  pub last_stage: String,
  /// The turn asking for the final report.
  pub final_turn: String,
  /// A question of the review.
  pub question_turn: String,
  /// The work sent back.
  pub rejection_turn: String,
  /// The work sent back to a stage already passed.
  pub rejection_back_turn: String,
  /// A turn's prompt where the job's session could not be resumed.
  pub carried: String,
  /// What a supervisor filing an item is answered with before the item is filed.
  pub steps_check: String,
  /// What a list of the steps check names where it holds nothing.
  pub steps_check_none: String,
}

/// One check a report is examined by.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
  /// Its name, as the record and the orderer read it.
  pub name: String,
  /// What it requires.
  pub requirement: String,
}

impl JobReview {
  /// The check named, or none for one the policy does not hold.
  pub fn check(&self, name: &str) -> Option<&Check> {
    self.checks.iter().find(|check| check.name == name)
  }

  /// The check after the one named, or none where it is the last.
  pub fn after(&self, name: &str) -> Option<&Check> {
    let at = self.checks.iter().position(|check| check.name == name)?;

    self.checks.get(at + 1)
  }
}

crate::part!(pub JOB_REVIEW: JobReview = "job-review.toml");
