//! The deploy queue flushed every hour (docs/agent-control.md, 「The deploy queue is flushed every hour」): the clock
//! read every minute, and once the flush minutes have passed since the last flush, every repository of this
//! machine with a commit waiting for its batch deployment put live at its main's commit, related work still open or
//! not. The deployments' outcomes are the deployment watch's to tell.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use policy::fill_marks;
use policy::job_notices::JOB_NOTICES;
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::queue::orderer::JobService;
use crate::queue::settings;
use crate::watches::closes;
use crate::watches::machine_turn::{deploy_root, live, read_json};

/// The settings row the moment of the last flush stands in, in epoch milliseconds.
pub const LAST_KEY: &str = "deploy_flush_last";
/// The owner's own settings row of the flush minutes.
pub const MINUTES_KEY: &str = "deploy_flush_minutes";

/// The testbed's binary, whose `deploy` verb puts a commit live.
const TESTBED: &str = "bmo-testbed";
/// The progress record under a deploy root (the testbed's docs/deploy.md, 「The places」).
const PROGRESS: &str = "deploy-progress.json";
/// The branch a close pushes its commits onto.
const MAIN: &str = "refs/heads/main";

/// How a flush reaches a repository's deployment, as a test stands in for it.
pub trait Deployments: Sync {
  /// The commit the main branch of the repository in `checkout` stands on at its remote.
  fn main_commit(&self, checkout: &Path) -> impl Future<Output = Result<String, String>> + Send;
  /// A deployment of `commit` started in `checkout` for the reason `why`; what the verb said where it started.
  fn start(
    &self,
    checkout: &Path,
    commit: &str,
    why: &str,
  ) -> impl Future<Output = Result<String, String>> + Send;
}

/// The deployments as git and the testbed's `deploy` verb answer them on this machine.
pub struct Testbed;

impl Deployments for Testbed {
  async fn main_commit(&self, checkout: &Path) -> Result<String, String> {
    let output = tokio::process::Command::new("git")
      .args(["ls-remote", "origin", MAIN])
      .current_dir(checkout)
      .stdin(std::process::Stdio::null())
      .output()
      .await
      .map_err(|why| format!("git ls-remote did not run: {why}"))?;
    let said = String::from_utf8_lossy(&output.stdout);

    said
      .split_whitespace()
      .next()
      .filter(|_| output.status.success())
      .map(str::to_owned)
      .ok_or_else(|| {
        format!("the remote names no {MAIN}: {}", String::from_utf8_lossy(&output.stderr).trim())
      })
  }

  async fn start(&self, checkout: &Path, commit: &str, why: &str) -> Result<String, String> {
    // The verb refuses a caller inside a harness or a deployment's unit, so neither mark is handed on.
    let output = tokio::process::Command::new(TESTBED)
      .args(["deploy", "--commit", commit, "--message", why])
      .current_dir(checkout)
      .env_remove("CLAUDE_PID")
      .env_remove("BMO_DEPLOY_UNIT")
      .stdin(std::process::Stdio::null())
      .output()
      .await
      .map_err(|why| format!("{TESTBED} deploy did not run: {why}"))?;
    let said = |bytes: &[u8]| String::from_utf8_lossy(bytes).trim().to_owned();

    match output.status.success() {
      true => Ok(said(&output.stdout)),
      false => Err(format!("{TESTBED} deploy ended {}: {}", output.status, said(&output.stderr))),
    }
  }
}

/// The reason a flushed deployment of a repository is made with, `items` the items whose commits wait for it.
pub fn why(items: &[i64]) -> String {
  let listed = items
    .iter()
    .map(|item| fill_marks(&JOB_NOTICES.deploy_flush_item, &[("item", &item.to_string())]))
    .collect::<Vec<_>>()
    .join(", ");

  fill_marks(&JOB_NOTICES.deploy_flush_why, &[("items", &listed)])
}

/// Whether a deployment's reason, as the deployment record names it, is the flush's.
pub fn flushed(reason: &str) -> bool {
  let words = JOB_NOTICES.deploy_flush_why.split("{items}").next().unwrap_or_default();

  !words.is_empty() && reason.starts_with(words)
}

// The repository a close's commit waits for: the close's own, else the item's assignee on an item that carries no
// repository's pair.
fn repository_of(close: &closes::Close, item: &Value) -> Option<String> {
  close.repository.clone().or_else(|| item["assigneeName"].as_str().map(str::to_owned))
}

// The items whose commits wait for each repository among `items`, those live as `checkouts` read them left out,
// each item once, lowest first.
async fn waiting(
  items: Vec<Value>,
  checkouts: &BTreeMap<String, PathBuf>,
  home: &Path,
) -> BTreeMap<String, Vec<i64>> {
  let mut waiting: BTreeMap<String, Vec<i64>> = BTreeMap::new();

  for item in items {
    let Some(id) = item["id"].as_i64() else { continue };

    if matches!(item["state"].as_str(), Some("done" | "dropped" | "merged")) {
      continue;
    }

    for close in closes::of(&item["labels"]) {
      let (Some(commit), Some(repository)) = (close.waiting(), repository_of(&close, &item)) else {
        continue;
      };

      if let Some(checkout) = checkouts.get(&repository)
        && live(checkout, commit, home).await == Some(true)
      {
        continue;
      }

      let ids = waiting.entry(repository).or_default();

      if !ids.contains(&id) {
        ids.push(id);
      }
    }
  }

  for ids in waiting.values_mut() {
    ids.sort_unstable();
  }

  waiting
}

// Whether the last run of the repository in `checkout` was of `commit` and ended without going live: main has
// not moved past what broke it, so a deployment of it would fail again.
fn failed_at(checkout: &Path, home: &Path, commit: &str) -> bool {
  let Some(record) = deploy_root(checkout, home).and_then(|root| read_json(&root.join(PROGRESS))) else {
    return false;
  };
  let outcome = record["ending"]["outcome"].as_str();
  let of_it = record["commit"].as_str().is_some_and(|run| !run.is_empty() && commit.starts_with(run));

  of_it && outcome.is_some_and(|outcome| outcome != "live") && record["refused"] != true
}

// The flush minutes as milliseconds: the owner's own row, else the policy's.
async fn interval_ms(db: &DatabaseConnection) -> Result<i64, String> {
  Ok(minutes(db).await? * 60_000)
}

/// The flush minutes in force: the owner's own settings row where it names a positive whole number, else the
/// policy's.
pub async fn minutes(db: &DatabaseConnection) -> Result<i64, String> {
  Ok(
    settings::get(db, MINUTES_KEY)
      .await
      .map_err(|why| why.detail)?
      .and_then(|value| value.trim().parse::<i64>().ok())
      .filter(|minutes| *minutes > 0)
      .unwrap_or(JOB_NOTICES.deploy_flush_minutes),
  )
}

/// One read of the clock at `now`: where no flush was kept, `now` kept as the last and nothing started; where the
/// flush minutes have passed since the last, the items under a close read, `now` kept as the last, and a deployment
/// of main's commit started through `deployments` for every repository with a commit waiting, unless main's commit
/// is the one its last run failed at. Nothing where the deployment names no job service. `checkouts` names the
/// checkout each repository is read and deployed in; a repository with none here is left to its supervisor.
pub async fn flush(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  checkouts: &BTreeMap<String, PathBuf>,
  home: &Path,
  now: i64,
  deployments: &impl Deployments,
) -> Result<(), String> {
  let Some(service) = JobService::of(vars) else {
    return Ok(());
  };
  let last = settings::get(db, LAST_KEY)
    .await
    .map_err(|why| why.detail)?
    .and_then(|last| last.trim().parse::<i64>().ok());

  match last {
    None => return settings::set(db, LAST_KEY, &now.to_string()).await.map_err(|why| why.detail),
    Some(last) if now - last < interval_ms(db).await? => return Ok(()),
    Some(_) => {}
  }

  let waiting = waiting(closes::items(&service).await?, checkouts, home).await;

  settings::set(db, LAST_KEY, &now.to_string()).await.map_err(|why| why.detail)?;

  for (repository, items) in waiting {
    let Some(checkout) = checkouts.get(&repository) else {
      eprintln!("deploy flush: {repository} has no checkout on this machine, so it is not flushed");
      continue;
    };
    let commit = match deployments.main_commit(checkout).await {
      Ok(commit) => commit,
      Err(why) => {
        eprintln!("deploy flush: {repository}'s main was not read: {why}");
        continue;
      }
    };

    if failed_at(checkout, home, &commit) {
      eprintln!(
        "deploy flush: {repository}'s main is the commit its last run failed at, so it waits for a mend"
      );
      continue;
    }

    match deployments.start(checkout, &commit, &why(&items)).await {
      Ok(said) => println!("deploy flush: {repository} at {commit} started: {said}"),
      Err(why) => eprintln!("deploy flush: {repository} at {commit} was not started: {why}"),
    }
  }

  Ok(())
}

/// One read of this machine's repositories at the present moment; a test's ground reads nothing.
pub async fn pass(db: &DatabaseConnection, vars: &BTreeMap<String, String>, home: &Path) {
  if crate::job_end::test_ground(vars) {
    return;
  }

  let checkouts = crate::watches::machine_turn::local_checkouts(db).await;

  if let Err(why) = flush(db, vars, &checkouts, home, crate::now_ms(), &Testbed).await {
    eprintln!("deploy flush: the read did not end: {why}");
  }
}
