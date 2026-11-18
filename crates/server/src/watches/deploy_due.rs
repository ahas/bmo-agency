//! A repository's batch deployment come due, told to its orderer (docs/agent-control.md, 「A repository's batch
//! deployment come due is told」): the items under a close read every minute, each repository's commits that wait
//! for its batch deployment gathered with the moment each began to wait, and the orderer of the item whose commit
//! has waited longest told once that commit has waited the due minutes. The notice is kept for the set of commits
//! it named, owed afresh as a commit joins that set or a deployment of the repository ends, and withdrawn once none
//! of its commits waits.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use policy::job_notices::JOB_NOTICES;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::job_notices;
use crate::outbox::{self, Keep, Notice, Receiver};
use crate::queue::orderer::{JobService, Orderer};
use crate::queue::settings;
use crate::watches::closes;
use crate::watches::machine_turn::{live, local_checkouts};

/// The prefix of the key the notice is owed under, its repository after it.
pub const DUE_OWED: &str = "repository-deploy-due:";
/// The settings row each repository's waiting commits stand in, as JSON.
pub const SINCE_KEY: &str = "deploy_due_since";
/// The owner's own settings row of the due minutes.
pub const MINUTES_KEY: &str = "deploy_due_minutes";

/// One commit an item's close left waiting for a repository's batch deployment, and since when it has waited, in
/// epoch milliseconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waiting {
  pub commit: String,
  pub since: i64,
}

/// What is kept between reads: each repository's waiting commits, by the item whose close left each.
pub type Kept = BTreeMap<String, BTreeMap<i64, Waiting>>;

/// One waiting commit as the notice names it: the item as the job service answers it, and its wait.
#[derive(Clone, Debug)]
pub struct Commit {
  pub item: Value,
  pub waiting: Waiting,
}

impl Commit {
  fn id(&self) -> i64 {
    self.item["id"].as_i64().unwrap_or_default()
  }

  // What the notice's mark holds of it: the item and its commit.
  fn mark(&self) -> String {
    format!("{}={}", self.id(), self.waiting.commit)
  }
}

/// What a repository's notice calls for at a read.
#[derive(Debug, PartialEq, Eq)]
pub enum Due {
  /// Told: owed afresh and its oldest commit due.
  Tell,
  /// Kept as it was told, or nothing owed yet.
  Stand,
  /// Let go, its copy withdrawn: none of the commits it named waits.
  LetGo,
}

/// What a repository's notice calls for, its waiting `commits` read at `now`, `told` the mark of the notice kept
/// for it, where one is: told where it is owed afresh — none kept, or a commit outside the set it named waiting —
/// and the oldest commit has waited `due_ms`; let go where none of the commits it named waits and it is not told.
pub fn due(commits: &[Commit], told: Option<&str>, now: i64, due_ms: i64) -> Due {
  let named: BTreeSet<&str> = told.into_iter().flat_map(|mark| mark.split(',')).collect();
  let marks: Vec<String> = commits.iter().map(Commit::mark).collect();
  let afresh = told.is_none() || marks.iter().any(|mark| !named.contains(mark.as_str()));
  let oldest = commits.iter().map(|commit| commit.waiting.since).min();

  if afresh && oldest.is_some_and(|since| now - since >= due_ms) {
    return Due::Tell;
  }

  match told.is_some() && marks.iter().all(|mark| !named.contains(mark.as_str())) {
    true => Due::LetGo,
    false => Due::Stand,
  }
}

// The repository a close's commit waits for: the close's own, else the item's assignee on an item that carries no
// repository's pair.
fn repository_of(close: &closes::Close, item: &Value) -> Option<String> {
  close.repository.clone().or_else(|| item["assigneeName"].as_str().map(str::to_owned))
}

// Each repository's waiting commits among `items`, those live as `checkouts` read them left out, each since the
// moment `kept` holds for it, else the item's last change, never later than `now`; oldest first.
async fn gathered(
  items: Vec<Value>,
  kept: &Kept,
  checkouts: &BTreeMap<String, PathBuf>,
  home: &Path,
  now: i64,
) -> BTreeMap<String, Vec<Commit>> {
  let mut waiting: BTreeMap<String, Vec<Commit>> = BTreeMap::new();

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

      let since = kept
        .get(&repository)
        .and_then(|commits| commits.get(&id))
        .filter(|kept| kept.commit == commit)
        .map_or_else(|| item["changedAt"].as_i64().unwrap_or(now).min(now), |kept| kept.since);

      waiting.entry(repository).or_default().push(Commit {
        item: item.clone(),
        waiting: Waiting {
          commit: commit.to_owned(),
          since,
        },
      });
    }
  }

  for commits in waiting.values_mut() {
    commits.sort_by_key(|commit| (commit.waiting.since, commit.id()));
  }

  waiting
}

// The due minutes as milliseconds: the owner's own row, else the policy's.
async fn due_ms(db: &DatabaseConnection) -> Result<i64, String> {
  let minutes = settings::get(db, MINUTES_KEY)
    .await
    .map_err(|why| why.detail)?
    .and_then(|value| value.trim().parse::<i64>().ok())
    .filter(|minutes| *minutes > 0)
    .unwrap_or(JOB_NOTICES.deploy_due_minutes);

  Ok(minutes * 60_000)
}

/// One read at `now`: the items under a close read, each repository's waiting commits kept with their moments, and
/// each repository's notice told, kept or let go as `due` answers; nothing where the deployment names no job
/// service or no receiver. `checkouts` names the checkout each repository's deployment is read in.
pub async fn told(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  checkouts: &BTreeMap<String, PathBuf>,
  home: &Path,
  now: i64,
) -> Result<(), String> {
  let (Some(service), Some(_)) = (JobService::of(vars), Receiver::of(vars)) else {
    return Ok(());
  };
  let kept: Kept = settings::get(db, SINCE_KEY)
    .await
    .map_err(|why| why.detail)?
    .and_then(|kept| serde_json::from_str(&kept).ok())
    .unwrap_or_default();
  let waiting = gathered(closes::items(&service).await?, &kept, checkouts, home, now).await;
  let keeping: Kept = waiting
    .iter()
    .map(|(repository, commits)| {
      let by_item = commits.iter().map(|commit| (commit.id(), commit.waiting.clone())).collect();

      (repository.clone(), by_item)
    })
    .collect();
  let written = serde_json::to_string(&keeping).map_err(|why| why.to_string())?;

  settings::set(db, SINCE_KEY, &written).await.map_err(|why| why.detail)?;

  let due_ms = due_ms(db).await?;
  let mut owed = outbox::owed(db, DUE_OWED).await?;
  let repositories: BTreeSet<String> = waiting.keys().chain(owed.keys()).cloned().collect();

  for repository in repositories {
    let commits = waiting.get(&repository).map(Vec::as_slice).unwrap_or_default();
    let told = owed.remove(&repository);

    match due(commits, told.as_ref().map(|told| told.mark.as_str()), now, due_ms) {
      Due::Tell => {
        let orderer = service.orderer_of(&commits[0].item).await.unwrap_or(Orderer {
          room: None,
          requester: None,
        });
        let body = job_notices::deploy_due(&repository, commits, now, due_ms, &orderer);
        let mark = commits.iter().map(Commit::mark).collect::<Vec<_>>().join(",");

        outbox::owe(db, vars, Notice::instruction(body, Keep::Kept).marked(mark)).await?;
        println!("deploy due: {repository}'s batch deployment is due, owed to its orderer");
      }
      Due::LetGo => {
        if let Some(told) = told {
          outbox::let_go_as_read(db, vars, &format!("{DUE_OWED}{repository}"), &told).await?;
        }
      }
      Due::Stand => {}
    }
  }

  Ok(())
}

/// A deployment of `repository` ended: its notice let go, its copy withdrawn, so the next read owes it afresh.
pub async fn rearmed(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  repository: &str,
) -> Result<(), String> {
  outbox::let_go(db, vars, &format!("{DUE_OWED}{repository}")).await
}

/// One read of this machine's repositories at the present moment; a test's ground reads nothing.
pub async fn pass(db: &DatabaseConnection, vars: &BTreeMap<String, String>, home: &Path) {
  if crate::job_end::test_ground(vars) {
    return;
  }

  let checkouts = local_checkouts(db).await;

  if let Err(why) = told(db, vars, &checkouts, home, crate::now_ms()).await {
    eprintln!("deploy due: the read did not end: {why}");
  }
}
