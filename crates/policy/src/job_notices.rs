//! What an item's orderer and the owner are told of a change in the queue (docs/agent-control.md, 「An item's
//! change is told to its orderer」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobNotices {
  /// How many items up the chain an item was raised from are read to find its orderer.
  pub orderer_depth: usize,
  /// The word a launch notice names where the item goes to any free checkout.
  pub any_worker: String,
  /// An open item in a queue that gives no lease: `{item}`, `{title}` and `{queue}`.
  pub paused_order: String,
  /// An open item whose assignee no checkout is of: `{item}`, `{title}` and `{assignee}`.
  pub unlaunchable_order: String,
  /// An item moved while its job runs: `{item}`, `{title}`, `{state}`, `{merged}`, `{job}` and `{worker}`.
  pub moved_order: String,
  /// An item closed by somebody else: `{item}`, `{title}` and `{by}`.
  pub closed_order: String,
  /// A run's question: `{question}`, `{item}`, `{text}` and `{choices}`.
  pub question_order: String,
  /// A job waiting for a road or a slot: `{item}`, `{worker}` and `{why}`.
  pub waiting_order: String,
  /// An ended job's item still leased: `{item}` and `{lease}`.
  pub close_order: String,
  /// The code of the owner's notice of a question past its due time.
  pub question_due_code: String,
  /// The code of the owner's notice of a job that failed in another room.
  pub failed_code: String,
}

crate::part!(pub JOB_NOTICES: JobNotices = "job-notices.toml");
