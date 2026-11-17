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
  /// An item whose launch was refused for its `agent` label: `{item}`, `{title}`, `{label}` and `{why}`.
  pub agent_order: String,
  /// An open item whose model no agent of its role runs: `{item}`, `{title}`, `{model}`, `{role}` and
  /// `{agents}`.
  pub no_candidate_order: String,
  /// An item moved while its job runs: `{item}`, `{title}`, `{state}`, `{merged}`, `{job}` and `{worker}`.
  pub moved_order: String,
  /// An item closed by somebody else: `{item}`, `{title}` and `{by}`.
  pub closed_order: String,
  /// The labels a held item names the items it waits on with.
  pub wait_labels: Vec<String>,
  /// A held item whose every item it waits on has ended: `{item}`, `{title}`, `{label}` and `{waited}`.
  pub wait_ended_order: String,
  /// One item a held item waited on, as `{waited}` lists it: `{item}`, `{title}` and `{state}`.
  pub waited_item: String,
  /// A job's commit handed over to go live, its job waiting on the outcome: `{item}`, `{title}`, `{repository}`
  /// and `{commit}`.
  pub deploy_order: String,
  /// A deployment that ended without going live: `{repository}`, `{commit}`, `{unit}`, `{outcome}`, `{stage}`,
  /// `{reason}`, `{items}` and `{journal}`.
  pub deploy_failed_order: String,
  /// One waiting item a failed deployment carries, as `{items}` lists it: `{item}` and `{title}`.
  pub deploy_failed_item: String,
  /// What `{items}` reads where the deployment carries no waiting item of the room.
  pub deploy_failed_no_items: String,
  /// The outcome a run cut off with none written is told as.
  pub deploy_cut_off: String,
  /// What `{stage}` reads where the run entered no stage.
  pub deploy_no_stage: String,
  /// What `{reason}` reads where the run gave none.
  pub deploy_no_reason: String,
  /// How often the deploy roots' progress records are read, in seconds.
  pub deploy_read_seconds: u64,
  /// How many of the last lines of a failed run's unit journal the order carries.
  pub deploy_journal_lines: usize,
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
  /// How often agency's sender sends the notices due, in seconds.
  pub send_seconds: u64,
  /// The wait after a notice's first failed try, in seconds, doubled after each try that fails after it.
  pub retry_first_seconds: u64,
  /// The longest wait between two tries of a notice, in seconds.
  pub retry_most_seconds: u64,
  /// How long a standing notice waits after it was last told before it is told again, in seconds.
  pub retell_seconds: u64,
  /// How long one try waits for the receiver's answer, in seconds; a notice is held to one sender for twice it.
  pub answer_seconds: u64,
  /// How long after a job's end, a check of its review coming due or its wait beginning, the sender waits before
  /// it owes a notice of it not yet owed itself, in seconds.
  pub owe_grace_seconds: u64,
}

crate::part!(pub JOB_NOTICES: JobNotices = "job-notices.toml");
