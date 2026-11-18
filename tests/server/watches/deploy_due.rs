//! A repository's batch deployment come due (docs/agent-control.md, 「A repository's batch deployment come due is
//! told」), read on a clock the test sets: told to the orderer of the item whose commit has waited longest once it
//! has waited the due minutes and not before, the moment it began kept past a later change of its item; not told
//! again for the same set, told again at once as a commit joins it; the owner's row setting the minutes; and
//! withdrawn once none of its commits waits, taken off its item or live as its repository's deployment reads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agency_server::outbox::owed;
use agency_server::queue::settings;
use agency_server::watches::deploy_due::{DUE_OWED, MINUTES_KEY, told};
use policy::job_notices::JOB_NOTICES;
use serde_json::{Value, json};

use crate::support::git::{at, git};
use crate::support::peers::{Peers, item, stand};
use crate::support::scratch::Scratch;

const T0: i64 = 1_700_000_000_000;
const MINUTE: i64 = 60_000;

fn due() -> i64 {
  JOB_NOTICES.deploy_due_minutes * MINUTE
}

/// An item whose close in `repository` left `commit` waiting, last changed at `changed`.
fn waiting(id: i64, room: &str, repository: &str, commit: &str, changed: i64) -> Value {
  item(
    id,
    "leased",
    json!({ "scope": room, "changedAt": changed, "labels": {
      "close-phase": "waiting-batch", "close-commit": commit,
      format!("close-phase:{repository}"): "waiting-batch", format!("close-commit:{repository}"): commit } }),
  )
}

fn due_told(peers: &Peers) -> Vec<Value> {
  let held = peers.lock().unwrap();

  held
    .instructed()
    .into_iter()
    .filter(|told| told["kind"] == "repository-deploy-due:web")
    .collect()
}

#[tokio::test]
async fn a_repository_is_told_once_its_oldest_commit_has_waited_the_due_minutes_and_not_before() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;
  let none = BTreeMap::new();
  let home = Path::new("/nonexistent");

  {
    let mut held = peers.lock().unwrap();

    held.items.insert(81, waiting(81, "-700", "web", "aaaa1111aaaa1111", T0 + 10 * MINUTE));
    held.items.insert(82, waiting(82, "-800", "web", "bbbb2222bbbb2222", T0));
    held
      .items
      .insert(83, waiting(83, "-700", "drive", "cccc3333cccc3333", T0 + 60 * MINUTE));
    held.items.insert(
      84,
      item(
        84,
        "done",
        json!({ "labels": {
      "close-phase:web": "waiting-batch", "close-commit:web": "dddd4444dddd4444" } }),
      ),
    );
  }

  told(db, &vars, &none, home, T0 + due() - 1).await.unwrap();
  assert!(
    peers.lock().unwrap().instructed().is_empty(),
    "a minute short of the mark tells nothing"
  );

  // A later change of the oldest item moves nothing: its commit waited since it was first read.
  peers.lock().unwrap().items.get_mut(&82).unwrap()["changedAt"] = json!(T0 + due());
  told(db, &vars, &none, home, T0 + due()).await.unwrap();
  told(db, &vars, &none, home, T0 + due() + MINUTE).await.unwrap();

  let told = peers.lock().unwrap().instructed();

  assert_eq!(told.len(), 1, "once, and nothing of drive's younger commit: {told:#?}");
  assert_eq!(told[0]["kind"], "repository-deploy-due:web");
  assert_eq!(told[0]["room"], json!(-800), "the room of the item whose commit waited longest");
  assert_eq!(told[0]["events"], json!(["repository-deploy-due:#82", "repository-deploy-due:#81"]));

  let said = told[0]["said"]["order"].as_str().unwrap();

  for part in [
    "web",
    "3 h 00 min",
    "bbbb2222bbbb",
    "#82 (item 82)",
    "aaaa1111aaaa",
    "#81 (item 81), waiting 2 h 50 min",
  ] {
    assert!(said.contains(part), "{part} in {said}");
  }

  assert!(!said.contains("dddd4444"), "an ended item's commit waits no more: {said}");
}

#[tokio::test]
async fn the_owners_row_sets_the_due_minutes() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;

  settings::set(db, MINUTES_KEY, "30").await.unwrap();
  peers.lock().unwrap().items.insert(81, waiting(81, "-700", "web", "aaaa1111", T0));
  told(db, &vars, &BTreeMap::new(), Path::new("/nonexistent"), T0 + 30 * MINUTE)
    .await
    .unwrap();

  assert_eq!(due_told(&peers).len(), 1);
}

#[tokio::test]
async fn a_told_notice_is_told_again_as_a_commit_joins_and_withdrawn_once_none_of_its_commits_waits() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;
  let none = BTreeMap::new();
  let home = Path::new("/nonexistent");

  peers.lock().unwrap().items.insert(81, waiting(81, "-700", "web", "aaaa1111", T0));
  told(db, &vars, &none, home, T0 + due()).await.unwrap();

  // A commit joins the set that was told: told again at once, the oldest being past the mark.
  peers
    .lock()
    .unwrap()
    .items
    .insert(85, waiting(85, "-800", "web", "eeee5555", T0 + due()));
  told(db, &vars, &none, home, T0 + due() + MINUTE).await.unwrap();
  told(db, &vars, &none, home, T0 + due() + 2 * MINUTE).await.unwrap();

  let told_so_far = due_told(&peers);

  assert_eq!(told_so_far.len(), 2, "{told_so_far:#?}");
  assert_eq!(
    told_so_far[1]["events"],
    json!(["repository-deploy-due:#81", "repository-deploy-due:#85"])
  );
  assert_eq!(told_so_far[1]["room"], json!(-700));

  // The batch went live and both pairs came off.
  {
    let mut held = peers.lock().unwrap();

    for id in [81, 85] {
      held.items.insert(id, item(id, "done", json!({})));
    }
  }

  told(db, &vars, &none, home, T0 + due() + 3 * MINUTE).await.unwrap();

  assert!(owed(db, DUE_OWED).await.unwrap().is_empty());
  assert_eq!(peers.lock().unwrap().withdrawn().last().map(String::as_str), Some("2"));
  assert_eq!(due_told(&peers).len(), 2);
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

fn deployed(root: &Path, commit: &str) {
  std::fs::write(root.join("deployed.json"), json!({ "commit": commit }).to_string()).unwrap();
}

#[tokio::test]
async fn a_commit_its_repositorys_deployment_holds_is_live_whatever_its_labels_read() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;
  let place = tempfile::tempdir().unwrap();
  let root = tempfile::tempdir().unwrap();
  let (first, second) = checkout(place.path(), root.path());
  let checkouts: BTreeMap<String, PathBuf> = [("web".to_owned(), place.path().to_path_buf())].into();
  let home = Path::new("/nonexistent");

  deployed(root.path(), &first);

  {
    let mut held = peers.lock().unwrap();

    held.items.insert(81, waiting(81, "-700", "web", &first, T0));
    held.items.insert(82, waiting(82, "-800", "web", &second, T0 + 60 * MINUTE));
  }

  told(db, &vars, &checkouts, home, T0 + due()).await.unwrap();
  assert!(due_told(&peers).is_empty(), "the live commit's wait is no wait");

  told(db, &vars, &checkouts, home, T0 + 60 * MINUTE + due()).await.unwrap();

  let told_so_far = due_told(&peers);

  assert_eq!(told_so_far.len(), 1);
  assert_eq!(told_so_far[0]["events"], json!(["repository-deploy-due:#82"]));

  deployed(root.path(), &second);
  told(db, &vars, &checkouts, home, T0 + 60 * MINUTE + due() + MINUTE).await.unwrap();

  assert!(owed(db, DUE_OWED).await.unwrap().is_empty(), "withdrawn once its commit is live");
  assert_eq!(peers.lock().unwrap().withdrawn(), ["1"]);
}
