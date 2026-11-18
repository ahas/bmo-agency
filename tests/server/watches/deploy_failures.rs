//! Deployments that ended without going live (docs/agent-control.md, 「A deployment that did not go live is
//! told」): a progress record read as its run's judgement; a failed run told once, to the orderer of each waiting
//! item whose commit it holds, one instruction a room, and to the owner's private room where it carries none; a
//! first pass telling nothing of what had already ended; and a run whose record a later run wrote over judged by its
//! unit's journal once its unit has ended.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use agency_server::queue::settings;
use agency_server::watches::deploy_failures::{
  Failure, Judgement, Root, STATE_KEY, State, Units, event_line, from_event, judged, last_said, watched,
};
use policy::job_notices::JOB_NOTICES;
use serde_json::{Value, json};

use crate::support::peers::{item, stand};
use crate::support::scratch::Scratch;

/// Units as a test sets them: those still running, the `deploy-failed` payload each one's journal holds, and the
/// lines each one's journal ends with.
#[derive(Default)]
struct StandIn {
  running: Mutex<BTreeSet<String>>,
  failed: BTreeMap<String, Value>,
  lines: Vec<String>,
}

impl Units for StandIn {
  async fn active(&self, unit: &str) -> Option<bool> {
    Some(self.running.lock().unwrap().contains(unit))
  }

  async fn failed_event(&self, unit: &str) -> Option<Value> {
    self.failed.get(unit).cloned()
  }

  async fn last_lines(&self, _unit: &str, count: usize) -> Vec<String> {
    self.lines.iter().take(count).cloned().collect()
  }
}

fn git(path: &Path, args: &[&str]) -> String {
  let done = Command::new("git")
    .arg("-C")
    .arg(path)
    .args(args)
    .env("GIT_AUTHOR_NAME", "tester")
    .env("GIT_AUTHOR_EMAIL", "tester@example.com")
    .env("GIT_COMMITTER_NAME", "tester")
    .env("GIT_COMMITTER_EMAIL", "tester@example.com")
    .output()
    .unwrap();

  assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));

  String::from_utf8_lossy(&done.stdout).trim().to_owned()
}

/// A checkout holding two commits, the first a close handed over and the second the batch's head.
fn checkout(path: &Path) -> (String, String) {
  git(path, &["init", "-q", "-b", "main"]);
  git(path, &["commit", "-q", "--allow-empty", "-m", "the mend"]);

  let handed = git(path, &["rev-parse", "HEAD"]);

  git(path, &["commit", "-q", "--allow-empty", "-m", "the batch's head"]);

  (handed, git(path, &["rev-parse", "HEAD"]))
}

fn record(unit: &str, commit: &str, ending: Value) -> Value {
  json!({
    "unit": unit, "pid": 1, "commit": commit, "repository": "web", "startedAt": 1,
    "plan": ["check", "backup"], "stages": [{ "stage": "check", "at": 2 }], "ahead": [], "running": null,
    "rolledBack": false, "refused": false, "ending": ending,
  })
}

fn write(root: &Path, record: &Value) {
  std::fs::write(root.join("deploy-progress.json"), record.to_string()).unwrap();
}

fn waiting(id: i64, room: &str, commit: &str) -> Value {
  item(
    id,
    "leased",
    json!({ "scope": room, "labels": { "close-phase": "waiting-batch", "close-commit": commit } }),
  )
}

fn order(instruction: &Value) -> &str {
  instruction["said"]["order"].as_str().unwrap()
}

#[test]
fn a_progress_record_is_judged_by_its_outcome_and_a_journal_line_by_its_event() {
  let failed = judged(
    &record("u", "abc", json!({ "outcome": "stopped", "why": "the gate failed" })),
    "web",
    true,
  );

  assert_eq!(
    failed,
    Judgement::Failed(Failure {
      repository: "web".to_owned(),
      commit: "abc".to_owned(),
      unit: "u".to_owned(),
      outcome: "stopped".to_owned(),
      stage: Some("check".to_owned()),
      reason: Some("the gate failed".to_owned()),
    })
  );
  assert_eq!(judged(&record("u", "abc", json!({ "outcome": "live" })), "web", true), Judgement::Fine);

  let mut refused = record("u", "abc", json!({ "outcome": "stopped" }));

  refused["refused"] = json!(true);
  assert_eq!(judged(&refused, "web", true), Judgement::Fine, "a refusal broke nothing");
  assert_eq!(judged(&record("u", "abc", Value::Null), "web", false), Judgement::Running);

  let Judgement::Failed(cut) = judged(&record("u", "abc", Value::Null), "web", true) else {
    panic!("a unit ended with no outcome written is cut off");
  };

  assert_eq!(cut.outcome, JOB_NOTICES.deploy_cut_off);

  let line = r#"event: deploy-failed {"repository":"web","commit":"abc","detail":{"commit":"abc","stage":"restart","reason":"no answer","rolled_back":true,"journal":"j"}}"#;
  let (name, payload) = event_line(line).unwrap();
  let told = from_event(&payload, "u", "fallback");

  assert_eq!(name, "deploy-failed");
  assert_eq!((told.outcome.as_str(), told.stage.as_deref()), ("rolled-back", Some("restart")));
  assert_eq!(event_line("deploying abc: a subject"), None);

  let said = format!("gate: a\ngate: b\nnothing went live\n{line}\n");

  assert_eq!(last_said(&said, 2), ["gate: b", "nothing went live"], "the event line is left out");
}

#[tokio::test]
async fn a_failed_deployment_is_told_once_to_each_room_of_the_items_it_carries() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;
  let place = tempfile::tempdir().unwrap();
  let (handed, head) = checkout(place.path());
  let roots = [Root {
    path: place.path().to_path_buf(),
    checkout: place.path().to_path_buf(),
    repository: "web".to_owned(),
  }];
  let units = StandIn {
    lines: vec!["static check: cargo format FAILED".to_owned()],
    ..StandIn::default()
  };

  {
    let mut held = peers.lock().unwrap();

    held.items.insert(71, waiting(71, "-700", &handed));
    held.items.insert(72, waiting(72, "-800", &head));
    // A commit of another repository, which this checkout does not hold.
    held.items.insert(73, waiting(73, "-700", "0123456789abcdef0123456789abcdef01234567"));
    // Work closed in two repositories: its pair of this repository is read, whatever the summing pair names.
    held.items.insert(
      74,
      item(
        74,
        "leased",
        json!({ "scope": "-700", "labels": {
          "close-phase": "waiting-batch", "close-commit": "0123456789abcdef0123456789abcdef01234567",
          "close-phase:web": "waiting-batch", "close-commit:web": handed,
          "close-phase:drive": "waiting-batch", "close-commit:drive": "0123456789abcdef0123456789abcdef01234567" } }),
      ),
    );
    // Another repository's pair, whatever commit it names, is no commit of this repository's.
    held.items.insert(
      75,
      item(
        75,
        "leased",
        json!({ "scope": "-700", "labels": { "close-phase:drive": "waiting-batch", "close-commit:drive": handed } }),
      ),
    );
  }

  // The first pass tells nothing of a run that had already ended.
  write(
    place.path(),
    &record("bmo-deploy-1", &head, json!({ "outcome": "failed", "why": "old" })),
  );
  watched(db, &vars, &roots, &units).await.unwrap();
  assert!(peers.lock().unwrap().instructed().is_empty());

  units.running.lock().unwrap().insert("bmo-deploy-2".to_owned());
  write(place.path(), &record("bmo-deploy-2", &head, Value::Null));
  watched(db, &vars, &roots, &units).await.unwrap();
  assert!(peers.lock().unwrap().instructed().is_empty(), "a run under way tells nothing");

  units.running.lock().unwrap().clear();
  write(
    place.path(),
    &record("bmo-deploy-2", &head, json!({ "outcome": "stopped", "why": "the gate failed" })),
  );
  watched(db, &vars, &roots, &units).await.unwrap();
  watched(db, &vars, &roots, &units).await.unwrap();

  let told = peers.lock().unwrap().instructed();
  let rooms: Vec<Value> = told.iter().map(|told| told["room"].clone()).collect();

  assert_eq!(rooms, [json!(-800), json!(-700)], "one instruction a room, once: {told:#?}");
  assert_eq!(told[1]["kind"], "deploy-failed:bmo-deploy-2");
  assert_eq!(told[1]["events"], json!(["deploy-failed:#71", "deploy-failed:#74"]));

  let said = order(&told[1]);

  for part in [
    "web",
    &head[..12],
    "bmo-deploy-2",
    "stopped",
    "check",
    "the gate failed",
    "#71 (item 71)",
  ] {
    assert!(said.contains(part), "{part} in {said}");
  }

  assert!(said.contains("static check: cargo format FAILED"), "the journal's last lines: {said}");
  assert!(!said.contains("#73"), "an item of another repository: {said}");
  assert!(!said.contains("#75"), "an item waiting on another repository alone: {said}");
}

#[tokio::test]
async fn a_run_whose_record_was_written_over_is_told_from_its_journal_in_the_owners_room() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;
  let place = tempfile::tempdir().unwrap();
  let roots = [Root {
    path: place.path().to_path_buf(),
    checkout: place.path().to_path_buf(),
    repository: "web".to_owned(),
  }];
  let failed = json!({ "repository": "web", "commit": "feedface",
    "detail": { "commit": "feedface", "stage": "check", "reason": "the gate failed", "rolled_back": false } });
  let units = StandIn {
    failed: [("bmo-deploy-5".to_owned(), failed)].into(),
    ..StandIn::default()
  };

  units
    .running
    .lock()
    .unwrap()
    .extend(["bmo-deploy-5".to_owned(), "bmo-deploy-6".to_owned()]);
  settings::set(db, STATE_KEY, "{}").await.unwrap();
  write(place.path(), &record("bmo-deploy-5", "feedface", Value::Null));
  watched(db, &vars, &roots, &units).await.unwrap();

  // A second run of the repository writes over the record while the first still runs, which then fails.
  write(place.path(), &record("bmo-deploy-6", "feedface", Value::Null));
  watched(db, &vars, &roots, &units).await.unwrap();
  units.running.lock().unwrap().remove("bmo-deploy-5");
  watched(db, &vars, &roots, &units).await.unwrap();
  watched(db, &vars, &roots, &units).await.unwrap();

  let told = peers.lock().unwrap().instructed();

  assert_eq!(told.len(), 1, "{told:#?}");
  assert_eq!(told[0]["room"], Value::Null, "carrying no item, it goes to the owner's private room");
  assert_eq!(told[0]["events"], json!(["deploy-failed"]));
  assert!(order(&told[0]).contains(&JOB_NOTICES.deploy_failed_no_items), "{}", order(&told[0]));

  let kept: State = serde_json::from_str(&settings::get(db, STATE_KEY).await.unwrap().unwrap()).unwrap();

  assert_eq!(kept.running.keys().collect::<Vec<_>>(), ["bmo-deploy-6"], "the second still runs");
}

#[tokio::test]
async fn a_run_of_the_repository_ending_live_or_failed_owes_its_due_notice_afresh() {
  use agency_server::watches::deploy_due::told;

  let scratch = Scratch::new().await;
  let db = scratch.db();
  let (peers, vars) = stand().await;
  let place = tempfile::tempdir().unwrap();
  let (handed, head) = checkout(place.path());
  let roots = [Root {
    path: place.path().to_path_buf(),
    checkout: place.path().to_path_buf(),
    repository: "web".to_owned(),
  }];
  let units = StandIn::default();
  let none = BTreeMap::new();
  let due = JOB_NOTICES.deploy_due_minutes * 60_000;
  let due_told = || {
    let held = peers.lock().unwrap();

    held
      .instructed()
      .into_iter()
      .filter(|told| told["kind"] == "repository-deploy-due:web")
      .count()
  };

  settings::set(db, STATE_KEY, "{}").await.unwrap();
  peers.lock().unwrap().items.insert(71, {
    let mut waiting = waiting(71, "-700", &handed);

    waiting["changedAt"] = json!(0);
    waiting["labels"]["close-phase:web"] = json!("waiting-batch");
    waiting["labels"]["close-commit:web"] = json!(handed);
    waiting
  });
  told(db, &vars, &none, Path::new("/nonexistent"), due).await.unwrap();
  told(db, &vars, &none, Path::new("/nonexistent"), due + 1).await.unwrap();
  assert_eq!(due_told(), 1);

  for (unit, outcome) in [("bmo-deploy-7", "live"), ("bmo-deploy-8", "failed")] {
    write(
      place.path(),
      &record(unit, &head, json!({ "outcome": outcome, "why": "the gate failed" })),
    );

    let ended = watched(db, &vars, &roots, &units).await.unwrap();

    assert_eq!(ended.into_iter().collect::<Vec<_>>(), ["web"], "{unit}");
    told(db, &vars, &none, Path::new("/nonexistent"), due + 2).await.unwrap();
  }

  assert_eq!(due_told(), 3, "told again after each run's end, its commit still past the mark");
  assert_eq!(peers.lock().unwrap().withdrawn(), ["1", "2"], "each copy withdrawn as its run ended");
}
