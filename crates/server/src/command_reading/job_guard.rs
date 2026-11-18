//! The job guard: what agency's shell hook refuses of a shell command on a line whose programs are jobs'
//! (docs/live-claude-guard.md). It refuses, in this order, what no opening lets through — Claude Code in
//! print mode, a removal the CLI would stop on, a shell habit that stalls a job — then a heavy command started
//! outside the testbed, then a command that starts Claude Code, writes into its session records, forks a
//! session or puts words into a pane, unless the job has opened the live-Claude guard for itself, in which case
//! the command is recorded against the opening and let through.
//!
//! What it needs beyond the command's text it asks of its ground: the testbed's reading of the line, the
//! configuration directories of the accounts, and the job's opening. The reading never fails a command: a
//! ground that cannot answer lets it through.

use super::ShellCall;
use super::habits::habit_refusal;
use super::records::{CliRecords, command_refusal, print_mode_refusal};
use super::removal::removal_refusal;
use super::testbed::{Classified, testbed_refusal};

/// The word every account's record directory ends in: a command that writes into one names it in its text.
const RECORDS_WORD: &str = "projects";

/// What the job guard asks beside the command's text.
pub trait JobGround {
  /// The testbed's reading of the line, or nothing where it cannot be asked.
  fn classify(&self, line: &str) -> impl Future<Output = Option<Classified>>;
  /// Every configuration directory a login keeps its session records under: the accounts' and the
  /// machine's own.
  fn config_dirs(&self) -> impl Future<Output = Vec<String>>;
  /// The latest live-Claude opening the job running this program made, or nothing.
  fn opening(&self) -> impl Future<Output = Option<i64>>;
  /// A command let through because of the opening, recorded against it.
  fn record(&self, opening: i64, what: &str) -> impl Future<Output = ()>;
}

/// What a refusal of a command the job has not opened the live-Claude guard for says.
pub fn live_claude_refusal(refused: &str) -> String {
  format!(
    "refused before this job has said what for: {refused}, which writes a whole prefix to the cache again. To open it for this job: bmo live-claude --purpose '<what you measure and why>'. Nobody is asked; every run it lets through is recorded and the count goes into the item's report (docs/live-claude-guard.md, 「A job opens it by saying what for」)."
  )
}

/// Why a job's shell may not run this command as written, or nothing; `home` is the home the command's
/// paths are spelled under.
pub async fn job_refusal(
  command: &str,
  call: ShellCall,
  home: &str,
  ground: &impl JobGround,
) -> Option<String> {
  if let Some(refused) = print_mode_refusal(command)
    .or_else(|| removal_refusal(command))
    .or_else(|| habit_refusal(command, call))
  {
    return Some(refused);
  }

  if let Some(refused) = ground.classify(command).await.as_ref().and_then(testbed_refusal) {
    return Some(refused);
  }

  // Which accounts there are is the store's, asked only where the text could name their records at all.
  let dirs = match command.contains(RECORDS_WORD) {
    true => ground.config_dirs().await,
    false => vec![],
  };
  let refused = command_refusal(command, &CliRecords::of(&dirs, home))?;
  let Some(opening) = ground.opening().await else {
    return Some(live_claude_refusal(&refused));
  };

  ground
    .record(opening, &format!("shell: {}", command.chars().take(200).collect::<String>()))
    .await;

  None
}
