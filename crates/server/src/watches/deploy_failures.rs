//! Deployments that ended without going live, told to the orderers of the items they carry (docs/agent-control.md,
//! 「A deployment that did not go live is told」). Every repository's progress record is read every few seconds;
//! a run seen running is remembered by its unit, and every run is judged once: from its record, or, where a later
//! run of the same repository wrote over that record, from its unit's journal.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use policy::job_notices::JOB_NOTICES;
use policy::machine_turn::MACHINE_TURN;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::job_notices;
use crate::outbox::{self, Keep, Notice};
use crate::queue::orderer::{JobService, Orderer};
use crate::queue::settings;
use crate::watches::closes;
use crate::watches::machine_turn::{declared_name, deploy_root, held_by, read_json};

/// The settings row what the watch has read stands in.
pub const STATE_KEY: &str = "deploy_watch";

/// The progress record under a deploy root (the testbed's docs/deploy.md, 「The places」).
const PROGRESS: &str = "deploy-progress.json";
/// The outcome of a run that put its commit live.
const LIVE: &str = "live";
/// The testbed's event a run that did not go live sends, rolled back or not.
pub const FAILED_EVENT: &str = "deploy-failed";

/// One deploy root watched: where it stands, the checkout of its repository git is asked in, and the repository's
/// name where its record names none, as the testbed declaration in that checkout gives it, else as its row does.
#[derive(Clone, Debug)]
pub struct Root {
  pub path: PathBuf,
  pub checkout: PathBuf,
  pub repository: String,
}

/// What the watch keeps between passes: the unit last judged for each deploy root, and each unit seen running and
/// not yet judged, with its deploy root.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
  #[serde(default)]
  pub judged: BTreeMap<String, String>,
  #[serde(default)]
  pub running: BTreeMap<String, String>,
}

/// A deployment that ended without going live, as its notice tells it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
  pub repository: String,
  pub commit: String,
  pub unit: String,
  pub outcome: String,
  pub stage: Option<String>,
  pub reason: Option<String>,
}

/// What a progress record says of its run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Judgement {
  /// It runs still, or whether its unit ended cannot be told.
  Running,
  /// It went live, or was refused.
  Fine,
  /// It ended without going live.
  Failed(Failure),
}

/// How a run's unit is read beside its record: whether it still runs, the payload of the `deploy-failed` event its
/// journal holds, and the last lines of its journal.
pub trait Units: Sync {
  /// Whether the unit still runs; none where it cannot be told.
  fn active(&self, unit: &str) -> impl Future<Output = Option<bool>> + Send;
  /// The payload of the `deploy-failed` event line its journal holds; none where it holds none.
  fn failed_event(&self, unit: &str) -> impl Future<Output = Option<Value>> + Send;
  /// The last `count` lines of its journal, oldest first.
  fn last_lines(&self, unit: &str, count: usize) -> impl Future<Output = Vec<String>> + Send;
}

/// The units as this machine's user manager and journal answer them.
pub struct Journal;

async fn printed(program: &str, args: &[&str]) -> Option<(bool, String)> {
  let output = tokio::process::Command::new(program)
    .args(args)
    .stdin(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .output()
    .await
    .ok()?;

  Some((output.status.success(), String::from_utf8_lossy(&output.stdout).into_owned()))
}

impl Units for Journal {
  async fn active(&self, unit: &str) -> Option<bool> {
    let (_, said) = printed("systemctl", &["--user", "is-active", unit]).await?;

    match said.trim() {
      "active" | "activating" | "deactivating" | "reloading" | "refreshing" => Some(true),
      "inactive" | "failed" => Some(false),
      _ => None,
    }
  }

  async fn failed_event(&self, unit: &str) -> Option<Value> {
    let grep = format!("^event: {FAILED_EVENT} ");
    let (_, said) =
      printed("journalctl", &["--user", &own_lines(unit), "-o", "cat", "--no-pager", "--grep", &grep])
        .await?;

    said
      .lines()
      .rev()
      .find_map(|line| event_line(line).filter(|(name, _)| name == FAILED_EVENT))
      .map(|(_, payload)| payload)
  }

  async fn last_lines(&self, unit: &str, count: usize) -> Vec<String> {
    // A few more than asked for, since an event line, which the order tells in its own words, is left out.
    let read = (count + EVENT_LINES).to_string();

    match printed("journalctl", &["--user", &own_lines(unit), "-n", &read, "-o", "cat", "--no-pager"]).await
    {
      Some((true, said)) => last_said(&said, count),
      _ => vec![],
    }
  }
}

/// How many event lines a run's journal may end with: the outcome, and a rollback's beside it.
const EVENT_LINES: usize = 2;

/// The journal's match for the lines the unit's own processes wrote, leaving out the user manager's lines about
/// the unit.
fn own_lines(unit: &str) -> String {
  let unit = unit.strip_suffix(".service").unwrap_or(unit);

  format!("_SYSTEMD_USER_UNIT={unit}.service")
}

/// The last `count` lines of a run's journal, `said` oldest first, its event lines left out.
pub fn last_said(said: &str, count: usize) -> Vec<String> {
  let kept: Vec<&str> = said.lines().filter(|line| event_line(line).is_none()).collect();

  kept[kept.len().saturating_sub(count)..].iter().map(|line| (*line).to_owned()).collect()
}

/// The event a line of the testbed's standard error carries where no hook took it: `event: <name> <payload>`, the
/// payload one JSON object (the testbed's docs/boundary.md, 「Events」).
pub fn event_line(line: &str) -> Option<(String, Value)> {
  let (name, payload) = line.trim().strip_prefix("event: ")?.split_once(' ')?;

  Some((name.to_owned(), serde_json::from_str(payload).ok()?))
}

fn text(value: &Value) -> Option<String> {
  value.as_str().map(str::trim).filter(|text| !text.is_empty()).map(str::to_owned)
}

/// What a progress record says of its run, `gone` whether its unit has ended where no outcome is written yet: a run
/// that went live or was refused is fine; one that ended `failed`, `rolled-back` or `stopped` failed, in the last
/// stage it entered; and one whose unit ended with no outcome written was cut off.
pub fn judged(record: &Value, repository: &str, gone: bool) -> Judgement {
  let ending = &record["ending"];
  let failure = |outcome: String, reason: Option<String>| Failure {
    repository: text(&record["repository"]).unwrap_or_else(|| repository.to_owned()),
    commit: text(&record["commit"]).unwrap_or_default(),
    unit: text(&record["unit"]).unwrap_or_default(),
    outcome,
    stage: record["stages"]
      .as_array()
      .and_then(|stages| stages.last())
      .and_then(|last| text(&last["stage"])),
    reason,
  };

  if !ending.is_object() {
    return match gone {
      true => Judgement::Failed(failure(JOB_NOTICES.deploy_cut_off.clone(), None)),
      false => Judgement::Running,
    };
  }

  let outcome = text(&ending["outcome"]).unwrap_or_default();

  if outcome == LIVE || record["refused"] == true {
    return Judgement::Fine;
  }

  Judgement::Failed(failure(outcome, text(&ending["why"])))
}

/// The failure a `deploy-failed` event's payload tells, for a run whose record was written over.
pub fn from_event(payload: &Value, unit: &str, repository: &str) -> Failure {
  let detail = &payload["detail"];

  Failure {
    repository: text(&payload["repository"]).unwrap_or_else(|| repository.to_owned()),
    commit: text(&detail["commit"]).or_else(|| text(&payload["commit"])).unwrap_or_default(),
    unit: unit.to_owned(),
    outcome: if detail["rolled_back"] == true { "rolled-back" } else { "failed" }.to_owned(),
    stage: text(&detail["stage"]),
    reason: text(&detail["reason"]),
  }
}

/// The items under a close waiting for `repository`'s batch deployment whose commit waiting in it `commit` is or
/// holds, as git answers in `checkout`.
async fn carried(
  service: &JobService,
  repository: &str,
  checkout: &Path,
  commit: &str,
) -> Result<Vec<Value>, String> {
  let answer = reqwest::Client::new()
    .get(format!("{}/items", service.url))
    .query(&[("label", MACHINE_TURN.close_label.as_str()), ("limit", "500")])
    .bearer_auth(&service.token)
    .send()
    .await
    .map_err(|why| why.to_string())?;

  if !answer.status().is_success() {
    return Err(format!("the job service answered {} to /items", answer.status().as_u16()));
  }

  let page: Value = answer.json().await.map_err(|why| why.to_string())?;
  let mut carried = vec![];

  for item in page["items"].as_array().into_iter().flatten() {
    let closes = closes::of(&item["labels"]);
    let Some(handed) = closes::waiting_in(&closes, repository).filter(|_| item["state"] != "done") else {
      continue;
    };

    if held_by(checkout, handed, commit).await == Some(true) {
      carried.push(item.clone());
    }
  }

  Ok(carried)
}

/// A failed run owed to the orderers of the items it carries, one instruction a room, the owner's private room
/// where it carries none or the job service does not answer.
async fn tell(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  root: &Root,
  failure: &Failure,
  units: &impl Units,
) -> Result<(), String> {
  let service = JobService::of(vars);
  let items = match &service {
    Some(service) => carried(service, &failure.repository, &root.checkout, &failure.commit)
      .await
      .unwrap_or_else(|why| {
        eprintln!("deploy watch: the items {} carries were not read: {why}", failure.unit);

        vec![]
      }),
    None => vec![],
  };
  let mut rooms: BTreeMap<Option<i64>, Vec<Value>> = BTreeMap::new();

  for item in items {
    let room = match &service {
      Some(service) => service.orderer_of(&item).await.map(|orderer| orderer.room).unwrap_or_default(),
      None => None,
    };

    rooms.entry(room).or_default().push(item);
  }

  if rooms.is_empty() {
    rooms.insert(None, vec![]);
  }

  let journal = units.last_lines(&failure.unit, JOB_NOTICES.deploy_journal_lines).await;
  let several = rooms.len() > 1;

  for (room, items) in rooms {
    let orderer = Orderer {
      room,
      requester: None,
    };
    let notice =
      Notice::instruction(job_notices::deploy_failed(failure, &items, &journal, &orderer), Keep::Once);
    let notice = match several {
      true => {
        let key =
          format!("{}@{}", notice.key, room.map_or_else(|| "owner".to_owned(), |room| room.to_string()));

        notice.keyed(key)
      }
      false => notice,
    };

    outbox::owe(db, vars, notice).await?;
  }

  println!(
    "deploy watch: {} of {} ended {} without going live, owed",
    failure.unit, failure.repository, failure.outcome
  );

  Ok(())
}

/// One pass over `roots`, each run judged once and every failed one told; the first pass, with no state kept, tells
/// nothing of what has already ended. A notice the store would not take leaves its run unjudged for the next pass.
pub async fn watched(
  db: &DatabaseConnection,
  vars: &BTreeMap<String, String>,
  roots: &[Root],
  units: &impl Units,
) -> Result<(), String> {
  let kept = settings::get(db, STATE_KEY).await.map_err(|why| why.detail)?;
  let telling = kept.is_some();
  let mut state: State = kept.and_then(|kept| serde_json::from_str(&kept).ok()).unwrap_or_default();

  for root in roots {
    let key = root.path.to_string_lossy().into_owned();
    let Some(record) = read_json(&root.path.join(PROGRESS)) else { continue };
    let Some(unit) = text(&record["unit"]) else { continue };

    if state.judged.get(&key) == Some(&unit) {
      state.running.remove(&unit);
      continue;
    }

    let gone = record["ending"].is_object() || units.active(&unit).await == Some(false);

    match judged(&record, &root.repository, gone) {
      Judgement::Running => {
        state.running.insert(unit, key);
        continue;
      }
      Judgement::Fine => {}
      Judgement::Failed(failure) if telling => {
        if let Err(why) = tell(db, vars, root, &failure, units).await {
          eprintln!("deploy watch: {unit} was not owed, so it is read again: {why}");
          continue;
        }
      }
      Judgement::Failed(_) => {}
    }

    state.running.remove(&unit);
    state.judged.insert(key, unit);
  }

  // A run remembered while it ran whose record now names another: judged by its journal once its unit has ended.
  for (unit, key) in state.running.clone() {
    if units.active(&unit).await != Some(false) {
      continue;
    }

    let root = roots.iter().find(|root| root.path.to_string_lossy() == key.as_str());

    if let (Some(root), Some(payload)) = (root, units.failed_event(&unit).await) {
      let failure = from_event(&payload, &unit, &root.repository);

      if let Err(why) = tell(db, vars, root, &failure, units).await {
        eprintln!("deploy watch: {unit} was not owed, so it is read again: {why}");
        continue;
      }
    }

    state.running.remove(&unit);
  }

  let written = serde_json::to_string(&state).map_err(|why| why.to_string())?;

  settings::set(db, STATE_KEY, &written).await.map_err(|why| why.detail)
}

/// The deploy roots of the repositories agency holds on this machine, each once, as the testbed declaration in the
/// repository's checkout names it.
async fn roots(db: &DatabaseConnection, home: &Path) -> Result<Vec<Root>, String> {
  let mut roots: BTreeMap<PathBuf, Root> = BTreeMap::new();

  for repository in crate::repositories::every(db).await.map_err(|why| why.detail)? {
    if repository.node.is_some() {
      continue;
    }

    let checkout = PathBuf::from(&repository.path);

    if let Some(path) = deploy_root(&checkout, home) {
      let name = declared_name(&checkout).unwrap_or(repository.name);

      roots.entry(path.clone()).or_insert(Root {
        path,
        checkout,
        repository: name,
      });
    }
  }

  Ok(roots.into_values().collect())
}

/// One pass of the deployment watch over this machine's deploy roots; a test's ground reads nothing.
pub async fn pass(db: &DatabaseConnection, vars: &BTreeMap<String, String>, home: &Path) {
  if crate::job_end::test_ground(vars) {
    return;
  }

  let roots = match roots(db, home).await {
    Ok(roots) => roots,
    Err(why) => return eprintln!("deploy watch: the repositories were not read: {why}"),
  };

  if let Err(why) = watched(db, vars, &roots, &Journal).await {
    eprintln!("deploy watch: the pass did not end: {why}");
  }
}
