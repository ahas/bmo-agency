//! What the job guard stands on in the shell hook's process (docs/live-claude-guard.md): the testbed's
//! `classify` verb for a heavy command, agency's store for the accounts' record directories and the job's
//! live-Claude opening, and the machine's own configuration directory. The store is opened once, and only
//! where a reading needs it; one that cannot be reached answers that nothing is known.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use harness_providers::Machine;
use sea_orm::DatabaseConnection;
use tokio::sync::OnceCell;

use super::live_claude;
use crate::command_reading::job_guard::JobGround;
use crate::command_reading::testbed::Classified;
use crate::control::claude_accounts::{accounts, machine_config_dir};
use store::{one, run};

/// The testbed's binary, found on the path as every caller finds it.
const TESTBED: &str = "bmo-testbed";

/// The job guard's ground for one call of the hook.
pub struct StoreGround {
  data_root: PathBuf,
  machine: Machine,
  run: Option<String>,
  store: OnceCell<Option<DatabaseConnection>>,
}

impl StoreGround {
  /// The ground under a data root, for the job this process runs for where it is a job's.
  pub fn new(data_root: &Path, machine: Machine) -> Self {
    Self {
      data_root: data_root.to_path_buf(),
      machine,
      run: live_claude::job_run(),
      store: OnceCell::new(),
    }
  }

  async fn db(&self) -> Option<&DatabaseConnection> {
    self
      .store
      .get_or_init(|| async {
        super::store(&self.data_root)
          .await
          .inspect_err(|why| eprintln!("job-guard: {why}"))
          .ok()
      })
      .await
      .as_ref()
  }
}

impl JobGround for StoreGround {
  async fn classify(&self, line: &str) -> Option<Classified> {
    let done = tokio::process::Command::new(TESTBED)
      .args(["classify", "--json", line])
      .stdin(Stdio::null())
      .stderr(Stdio::null())
      .kill_on_drop(true)
      .output()
      .await
      .inspect_err(|why| eprintln!("job-guard: {TESTBED}: {why}"))
      .ok()?;

    // Light is 0 and heavy 1; anything else is words it could not read.
    match done.status.code() {
      Some(0 | 1) => serde_json::from_slice(&done.stdout).ok(),
      _ => None,
    }
  }

  async fn config_dirs(&self) -> Vec<String> {
    let mut dirs = vec![machine_config_dir(&self.machine)];

    if let Some(db) = self.db().await {
      match accounts(db).await {
        Ok(held) => dirs.extend(held.iter().map(|account| account.records_dir(&self.machine))),
        Err(why) => eprintln!("job-guard: {}", why.error),
      }
    }

    dirs.iter().map(|dir| dir.to_string_lossy().into_owned()).collect()
  }

  async fn opening(&self) -> Option<i64> {
    let run = self.run.as_deref()?;
    let row = one(
      self.db().await?,
      r#"SELECT "id" FROM "live_claude_openings" WHERE "run" = $1 ORDER BY "id" DESC LIMIT 1"#,
      vec![run.into()],
    )
    .await
    .ok()??;

    row.try_get::<i64>("", "id").ok()
  }

  async fn record(&self, opening: i64, what: &str) {
    let Some(db) = self.db().await else { return };

    if let Err(why) = run(
      db,
      r#"INSERT INTO "live_claude_runs" ("opening_id", "what", "ran_at") VALUES ($1, $2, $3)"#,
      vec![opening.into(), what.into(), crate::now_ms().into()],
    )
    .await
    {
      eprintln!("job-guard: {why}");
    }
  }
}
