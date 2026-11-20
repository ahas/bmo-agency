//! The deploy queue flushed every hour (docs/agent-control.md, 「The deploy queue is flushed every hour」), read on a
//! clock the test sets: nothing started before the flush minutes have passed since the last flush, and then every
//! repository with a commit waiting put live at its main's commit, related work still open or not; the owner's row
//! setting the minutes; a commit its repository's deployment holds waiting no more; and a main its last run failed
//! at left for its mend.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use agency_server::queue::settings;
use agency_server::watches::deploy_flush::{Deployments, LAST_KEY, MINUTES_KEY, flush};
use policy::job_notices::JOB_NOTICES;
use serde_json::{Value, json};

use crate::support::git::{at, git};
use crate::support::peers::{item, stand};
use crate::support::scratch::Scratch;

const T0: i64 = 1_700_000_000_000;
const MINUTE: i64 = 60_000;

/// Deployments as a test sets them: main's commit for each checkout, and every deployment started, with its checkout,
/// its commit and its reason.
#[derive(Default)]
struct StandIn {
  mains: BTreeMap<PathBuf, String>,
  started: Mutex<Vec<(PathBuf, String, String)>>,
}

impl StandIn {
  fn started(&self) -> Vec<(PathBuf, String, String)> {
    std::mem::take(&mut *self.started.lock().unwrap())
  }
}

impl Deployments for StandIn {
  async fn main_commit(&self, checkout: &Path) -> Result<String, String> {
    self.mains.get(checkout).cloned().ok_or_else(|| "no remote".to_owned())
  }

  async fn start(&self, checkout: &Path, commit: &str, why: &str) -> Result<String, String> {
    self
      .started
      .lock()
      .unwrap()
      .push((checkout.to_path_buf(), commit.to_owned(), why.to_owned()));

    Ok("bmo-deploy-1".to_owned())
  }
}

/// An item whose close in `repository` left `commit` waiting.
fn waiting(id: i64, repository: &str, commit: &str) -> Value {
  item(
    id,
    "leased",
    json!({ "scope": "-700", "labels": {
      "close-phase": "waiting-batch", "close-commit": commit,
      format!("close-phase:{repository}"): "waiting-batch", format!("close-commit:{repository}"): commit } }),
  )
}

fn interval() -> i64 {
  JOB_NOTICES.deploy_flush_minutes * MINUTE
}

#[tokio::test]
async fn every_repository_with_a_waiting_commit_is_put_live_once_the_flush_minutes_have_passed_and_not_before()
 {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;
  let web = tempfile::tempdir().unwrap();
  let drive = tempfile::tempdir().unwrap();
  let checkouts: BTreeMap<String, PathBuf> = [
    ("web".to_owned(), web.path().to_path_buf()),
    ("drive".to_owned(), drive.path().to_path_buf()),
  ]
  .into();
  let deployments = StandIn {
    mains: [
      (web.path().to_path_buf(), "web-main".to_owned()),
      (drive.path().to_path_buf(), "drive-main".to_owned()),
    ]
    .into(),
    ..StandIn::default()
  };
  let home = Path::new("/nonexistent");

  {
    let mut held = peers.lock().unwrap();

    held.items.insert(82, waiting(82, "web", "bbbb2222"));
    held.items.insert(81, waiting(81, "web", "aaaa1111"));
    held.items.insert(83, waiting(83, "drive", "cccc3333"));
    held.items.insert(84, {
      let mut done = waiting(84, "web", "dddd4444");

      done["state"] = json!("done");
      done
    });
    // Related work still open in web holds no repository back.
    held.items.insert(85, item(85, "open", json!({ "assigneeName": "web" })));
  }

  flush(db, &vars, &checkouts, home, T0, &deployments).await.unwrap();
  assert!(deployments.started().is_empty(), "the first read only starts the clock");
  assert_eq!(settings::get(db, LAST_KEY).await.unwrap(), Some(T0.to_string()));

  flush(db, &vars, &checkouts, home, T0 + interval() - 1, &deployments).await.unwrap();
  assert!(deployments.started().is_empty(), "a moment short of the mark starts nothing");

  flush(db, &vars, &checkouts, home, T0 + interval(), &deployments).await.unwrap();

  let flushed = deployments.started();

  assert_eq!(
    flushed,
    [
      (
        drive.path().to_path_buf(),
        "drive-main".to_owned(),
        "Hourly flush of the deploy queue, carrying #83".to_owned()
      ),
      (
        web.path().to_path_buf(),
        "web-main".to_owned(),
        "Hourly flush of the deploy queue, carrying #81, #82".to_owned()
      ),
    ],
    "every repository with a commit waiting, at its main, the done item's commit left out"
  );

  flush(db, &vars, &checkouts, home, T0 + interval() + MINUTE, &deployments)
    .await
    .unwrap();
  assert!(deployments.started().is_empty(), "the clock starts again from the flush");

  settings::set(db, MINUTES_KEY, "15").await.unwrap();
  flush(db, &vars, &checkouts, home, T0 + interval() + 15 * MINUTE, &deployments)
    .await
    .unwrap();
  assert_eq!(deployments.started().len(), 2, "the owner's row sets the minutes, read at each tick");
}

/// A checkout of `web` holding two commits, its testbed declaration naming `root` as its deploy root.
fn checkout(path: &Path, root: &Path) -> (String, String) {
  git(&["init", "-q", "-b", "main", &at(path)]);
  git(&["-C", &at(path), "commit", "-q", "--allow-empty", "-m", "the first"]);

  let first = git(&["-C", &at(path), "rev-parse", "HEAD"]);

  git(&["-C", &at(path), "commit", "-q", "--allow-empty", "-m", "the second"]);
  std::fs::write(
    path.join("testbed.toml"),
    format!("name = \"web\"\n\n[deploy]\nroot = \"{}\"\n", at(root)),
  )
  .unwrap();

  (first, git(&["-C", &at(path), "rev-parse", "HEAD"]))
}

#[tokio::test]
async fn a_live_commit_waits_no_more_and_a_main_its_last_run_failed_at_waits_for_its_mend() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;
  let place = tempfile::tempdir().unwrap();
  let root = tempfile::tempdir().unwrap();
  let (first, second) = checkout(place.path(), root.path());
  let checkouts: BTreeMap<String, PathBuf> = [("web".to_owned(), place.path().to_path_buf())].into();
  let mut deployments = StandIn {
    mains: [(place.path().to_path_buf(), second.clone())].into(),
    ..StandIn::default()
  };
  let home = Path::new("/nonexistent");
  let mut hour = 0;
  let mut flushed = async |deployments: &StandIn| {
    hour += 1;
    flush(db, &vars, &checkouts, home, T0 + hour * interval(), deployments).await.unwrap();

    deployments.started()
  };

  settings::set(db, LAST_KEY, &T0.to_string()).await.unwrap();
  std::fs::write(root.path().join("deployed.json"), json!({ "commit": second }).to_string()).unwrap();
  peers.lock().unwrap().items.insert(81, waiting(81, "web", &first));
  assert!(flushed(&deployments).await.is_empty(), "a commit the live one holds waits no more");

  std::fs::write(root.path().join("deployed.json"), json!({ "commit": first }).to_string()).unwrap();
  peers.lock().unwrap().items.insert(82, waiting(82, "web", &second));
  std::fs::write(
    root.path().join("deploy-progress.json"),
    json!({ "unit": "bmo-deploy-3", "commit": second, "refused": false, "ending": { "outcome": "failed" } })
      .to_string(),
  )
  .unwrap();
  assert!(flushed(&deployments).await.is_empty(), "main is what its last run failed at");

  deployments.mains.insert(place.path().to_path_buf(), "a-mend".to_owned());
  assert_eq!(
    flushed(&deployments).await,
    [(
      place.path().to_path_buf(),
      "a-mend".to_owned(),
      "Hourly flush of the deploy queue, carrying #82".to_owned()
    )],
    "a main moved past the failure is flushed"
  );
}
