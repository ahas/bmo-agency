//! The job workers as the store's agents table holds them, the built-in standing whether its row
//! has been written yet or not, and a base's support workers following its settings
//! (docs/worker-slots.md). A resting worker is no worker of the list.

use std::path::Path;

use policy::workers::WORKERS;
use sea_orm::{ConnectionTrait, DatabaseConnection, QueryResult, TransactionTrait, Value};
use serde::Serialize;
use store::{all, one, run};

use crate::events::{Bus, Event};
use crate::peers::node::NodeService;
use crate::{Refusal, now_ms};

/// A worker removed with its sessions (docs/api.md, 「Events」): a consumer lets go what it lent it, such as
/// a checkout lease.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerRemoved {
  pub worker: String,
}

impl Event for WorkerRemoved {
  const NAME: &'static str = "worker-removed";
}

/// The worker every agent built on the job base has; the store's rows default to it.
pub const BUILTIN_WORKER: &str = "worker";

/// The icon a worker's row is written with.
const WORKER_ICON: &str = "i-material-symbols-construction";

/// One worker as the queue reads it.
#[derive(Clone, Debug)]
pub struct Worker {
  pub name: String,
  pub label: String,
  pub cwd: String,
  pub builtin: bool,
  pub support_count: i64,
  pub support_keepalive: i64,
  pub support_of: Option<String>,
  pub support_slot: Option<i64>,
  /// The work node it stands on, none for this machine.
  pub node: Option<String>,
}

impl Worker {
  fn of(row: &QueryResult, source: &Path) -> Result<Self, sea_orm::DbErr> {
    let name: String = row.try_get("", "name")?;

    Ok(Self {
      label: row.try_get::<Option<String>>("", "label")?.unwrap_or_else(|| name.clone()),
      cwd: row
        .try_get::<Option<String>>("", "cwd")?
        .unwrap_or_else(|| source.to_string_lossy().into_owned()),
      builtin: row.try_get::<i64>("", "builtin")? != 0,
      support_count: row.try_get("", "support_count")?,
      support_keepalive: row.try_get("", "support_keepalive")?,
      support_of: row.try_get("", "support_of")?,
      support_slot: row.try_get("", "support_slot")?,
      node: row.try_get::<Option<String>>("", "node")?.filter(|node| !node.is_empty()),
      name,
    })
  }

  /// Whether it is a support worker rather than a base.
  pub fn is_support(&self) -> bool {
    self.support_of.as_deref().is_some_and(|base| !base.is_empty())
  }

  /// Whether it is a base's checkout on a node rather than a numbered worktree here (docs/work-nodes.md,
  /// 「A checkout on a node」).
  pub fn is_node_checkout(&self) -> bool {
    self.node.is_some() && self.support_slot.is_none()
  }
}

/// Whether a worker of that name stands: the built-in always, any other while its row is a worker's
/// that is not resting.
pub async fn is_worker(db: &DatabaseConnection, name: &str) -> Result<bool, Refusal> {
  if name == BUILTIN_WORKER {
    return Ok(true);
  }

  let row = one(
    db,
    r#"SELECT 1 AS "one" FROM "agents" WHERE "name" = $1 AND "kind" = 'worker' AND "resting_since" IS NULL"#,
    vec![name.into()],
  )
  .await?;

  Ok(row.is_some())
}

/// Every worker, each base followed by its support workers in their order; the built-in stands first
/// whether its row has been written or not, working in the source root where it has none.
pub async fn workers(db: &DatabaseConnection, source: &Path) -> Result<Vec<Worker>, Refusal> {
  let rows = all(
    db,
    r#"SELECT a.* FROM "agents" a WHERE a."kind" = 'worker' AND a."resting_since" IS NULL
       ORDER BY COALESCE((SELECT b."builtin" FROM "agents" b WHERE b."name" = a."support_of"), a."builtin") DESC,
         COALESCE(a."support_of", a."name"), COALESCE(a."support_slot", 1)"#,
    vec![],
  )
  .await?;
  let mut workers: Vec<Worker> =
    rows.iter().map(|row| Worker::of(row, source)).collect::<Result<_, _>>()?;

  if !workers.iter().any(|worker| worker.name == BUILTIN_WORKER) {
    workers.insert(
      0,
      Worker {
        name: BUILTIN_WORKER.into(),
        label: BUILTIN_WORKER.into(),
        cwd: source.to_string_lossy().into_owned(),
        builtin: true,
        support_count: 0,
        support_keepalive: 0,
        support_of: None,
        support_slot: None,
        node: None,
      },
    );
  }

  Ok(workers)
}

/// One worker's row as it stands, resting or not.
pub async fn worker_row(
  db: &DatabaseConnection,
  name: &str,
  source: &Path,
) -> Result<Option<Worker>, Refusal> {
  let row = one(
    db,
    r#"SELECT * FROM "agents" WHERE "name" = $1 AND "kind" = 'worker'"#,
    vec![name.into()],
  )
  .await?;

  Ok(row.as_ref().map(|row| Worker::of(row, source)).transpose()?)
}

/// Every name an agent's row stands under, a resting worker's included: the names a new agent may not
/// take.
pub async fn taken_names(db: &DatabaseConnection) -> Result<Vec<String>, Refusal> {
  let rows = all(db, r#"SELECT "name" FROM "agents" ORDER BY "name""#, vec![]).await?;

  Ok(rows.iter().map(|row| row.try_get("", "name")).collect::<Result<_, _>>()?)
}

async fn named_agent(db: &impl ConnectionTrait, name: &str) -> Result<bool, Refusal> {
  Ok(
    one(db, r#"SELECT 1 AS "one" FROM "agents" WHERE "name" = $1"#, vec![name.into()])
      .await?
      .is_some(),
  )
}

/// A worker the owner added, marked as the owner's rather than the machine's, so the bot's next start
/// neither writes over it nor claims it.
pub struct NewWorker<'a> {
  pub name: &'a str,
  pub label: &'a str,
  pub cwd: &'a str,
  pub preamble: Option<&'a str>,
  pub dev_command: Option<&'a str>,
}

pub async fn add_worker(db: &DatabaseConnection, worker: &NewWorker<'_>) -> Result<(), Refusal> {
  run(
    db,
    r#"INSERT INTO "agents" ("name", "kind", "role", "label", "cwd", "icon", "builtin", "last_run_at", "preamble", "dev_command")
       VALUES ($1, 'worker', 'worker', $2, $3, $4, 0, $5, $6, $7)"#,
    vec![
      worker.name.into(),
      worker.label.into(),
      worker.cwd.into(),
      WORKER_ICON.into(),
      (now_ms() / 1000).into(),
      worker.preamble.into(),
      worker.dev_command.into(),
    ],
  )
  .await?;

  Ok(())
}

/// What the owner asked to change about a worker; a field left none is left alone.
#[derive(Default)]
pub struct Changes {
  pub label: Option<String>,
  pub cwd: Option<String>,
  pub preamble: Option<Option<String>>,
  pub dev_command: Option<Option<String>>,
  pub downgrade_heal: Option<bool>,
  pub keepalive_stopped: Option<bool>,
  pub support_count: Option<i64>,
  pub support_keepalive: Option<i64>,
  /// The provider a worker is pinned to and the model with it; none for each takes the pin off.
  pub pinned: Option<(Option<String>, Option<String>)>,
  /// The name herdr alone shows it under; none gives herdr the label back.
  pub herdr_label: Option<Option<String>>,
  /// The work node it stands on; none stands it on this machine.
  pub node: Option<Option<String>>,
}

impl Changes {
  /// Whether it changes anything but the downgrade heal, which alone a support worker keeps its own.
  pub fn beyond_heal(&self) -> bool {
    self.label.is_some()
      || self.cwd.is_some()
      || self.preamble.is_some()
      || self.dev_command.is_some()
      || self.keepalive_stopped.is_some()
      || self.support_count.is_some()
      || self.support_keepalive.is_some()
      || self.pinned.is_some()
      || self.herdr_label.is_some()
      || self.node.is_some()
  }
}

/// Only what was named moves; the rest of the row is the record. Whether a worker's row changed.
pub async fn update_worker(
  db: &impl ConnectionTrait,
  name: &str,
  changes: &Changes,
) -> Result<bool, Refusal> {
  let mut sets: Vec<String> = vec![];
  let mut values: Vec<Value> = vec![];
  let mut set = |column: &str, value: Value| {
    values.push(value);
    sets.push(format!(r#""{column}" = ${}"#, values.len()));
  };

  if let Some(label) = &changes.label {
    set("label", label.clone().into());
  }

  if let Some(cwd) = &changes.cwd {
    set("cwd", cwd.clone().into());
  }

  if let Some(preamble) = &changes.preamble {
    set("preamble", preamble.clone().into());
  }

  if let Some(dev_command) = &changes.dev_command {
    set("dev_command", dev_command.clone().into());
  }

  if let Some(heal) = changes.downgrade_heal {
    set("downgrade_heal", i64::from(heal).into());
  }

  if let Some(stopped) = changes.keepalive_stopped {
    set("keepalive_stopped", i64::from(stopped).into());
  }

  if let Some(count) = changes.support_count {
    set("support_count", count.into());
  }

  if let Some(keepalive) = changes.support_keepalive {
    set("support_keepalive", keepalive.into());
  }

  if let Some((provider, model)) = &changes.pinned {
    let model = if provider.is_none() { None } else { model.clone() };

    set("pinned_provider", provider.clone().into());
    set("pinned_model", model.into());
  }

  if let Some(herdr_label) = &changes.herdr_label {
    set("herdr_label", herdr_label.clone().into());
  }

  if let Some(node) = &changes.node {
    set("node", node.clone().into());
  }

  if sets.is_empty() {
    return Ok(false);
  }

  values.push(name.into());

  let sql = format!(
    r#"UPDATE "agents" SET {} WHERE "name" = ${} AND "kind" = 'worker'"#,
    sets.join(", "),
    values.len()
  );

  Ok(run(db, &sql, values).await? > 0)
}

/// A worker the owner added, taken away with its sessions, and `worker-removed` published so what it was
/// lent goes too; the built-in never is. Whether there was one.
pub async fn remove_worker(db: &DatabaseConnection, events: &Bus, name: &str) -> Result<bool, Refusal> {
  let unit = db.begin().await?;
  let gone = run(
    &unit,
    r#"DELETE FROM "agents" WHERE "name" = $1 AND "kind" = 'worker' AND "builtin" = 0"#,
    vec![name.into()],
  )
  .await?
    > 0;

  if gone {
    run(&unit, r#"DELETE FROM "worker_sessions" WHERE "worker" = $1"#, vec![name.into()]).await?;
  }

  unit.commit().await?;

  if gone {
    events.publish(WorkerRemoved {
      worker: name.to_owned(),
    });
  }

  Ok(gone)
}

/// A support worker's name.
pub fn support_name(base: &str, slot: i64) -> String {
  format!("{base}-w{slot}")
}

/// A support worker's directory, beside the base's checkout.
fn support_path(base_cwd: &str, slot: i64) -> String {
  format!("{}-w{slot}", super::paths::resolve(base_cwd).display())
}

/// What a support worker is called on a screen.
fn support_label(base_label: &str, slot: i64) -> String {
  format!("{base_label}#{slot}")
}

/// What a base's checkout on a node is called on a screen.
pub fn node_checkout_label(base_label: &str, node_label: &str) -> String {
  format!("{base_label}@{node_label}")
}

// Whether a support worker is within its base's count, which is what makes it usable. A checkout on a
// node is counted by no number and stands until it is removed.
fn support_in_use(worker: &Worker, base: &Worker) -> bool {
  if worker.is_node_checkout() {
    return true;
  }

  let slot = worker.support_slot.unwrap_or(0);

  slot >= WORKERS.first_support_slot && slot < WORKERS.first_support_slot + base.support_count
}

/// The support rows one settling wrote and took away, by name.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Settled {
  pub added: Vec<String>,
  pub removed: Vec<String>,
}

/// A base's support workers brought to its settings: written where the count asks for more, their
/// labels and places following the base's, a checkout on a node called by the base and the node, and
/// removed where the count no longer reaches them once nothing holds them. A support worker the busy
/// test names is left standing.
pub async fn settle_support(
  db: &DatabaseConnection,
  nodes: &dyn NodeService,
  events: &Bus,
  base: &str,
  source: &Path,
  busy: impl Fn(&str) -> bool,
) -> Result<Settled, Refusal> {
  let mut settled = Settled::default();
  let Some(row) = worker_row(db, base, source).await? else {
    return Ok(settled);
  };

  if row.is_support() {
    return Ok(settled);
  }

  let held: Vec<Worker> = workers(db, source)
    .await?
    .into_iter()
    .filter(|worker| worker.support_of.as_deref() == Some(base))
    .collect();

  for slot in WORKERS.first_support_slot..WORKERS.first_support_slot + row.support_count {
    let name = support_name(base, slot);
    let wanted_label = support_label(&row.label, slot);
    let wanted_cwd = support_path(&row.cwd, slot);

    if let Some(existing) = held.iter().find(|worker| worker.support_slot == Some(slot)) {
      if existing.label != wanted_label || existing.cwd != wanted_cwd {
        let changes = Changes {
          label: Some(wanted_label),
          cwd: Some(wanted_cwd),
          ..Changes::default()
        };

        update_worker(db, &existing.name, &changes).await?;
      }

      continue;
    }

    if named_agent(db, &name).await? {
      eprintln!("support worker {name}: the name is another agent's, so {base} goes without it");
      continue;
    }

    run(
      db,
      r#"INSERT INTO "agents" ("name", "kind", "role", "label", "cwd", "icon", "builtin", "last_run_at", "support_of", "support_slot")
         VALUES ($1, 'worker', 'worker', $2, $3, $4, 0, $5, $6, $7)"#,
      vec![
        name.clone().into(),
        wanted_label.into(),
        wanted_cwd.into(),
        WORKER_ICON.into(),
        (now_ms() / 1000).into(),
        base.into(),
        slot.into(),
      ],
    )
    .await?;
    settled.added.push(name);
  }

  let checkouts: Vec<&Worker> = held.iter().filter(|worker| worker.is_node_checkout()).collect();
  // The node service keeps the nodes and the labels they are shown under; one not asked has none.
  let node_rows = if checkouts.is_empty() { vec![] } else { nodes.node_rows().await? };

  for worker in checkouts {
    let node = worker.node.as_deref().unwrap_or_default();
    let node_label = node_rows.iter().find(|row| row.node == node).and_then(|row| row.label.clone());
    let wanted = node_checkout_label(&row.label, node_label.as_deref().unwrap_or(node));

    if worker.label != wanted {
      let changes = Changes {
        label: Some(wanted),
        ..Changes::default()
      };

      update_worker(db, &worker.name, &changes).await?;
    }
  }

  for worker in held.iter().filter(|worker| !support_in_use(worker, &row) && !busy(&worker.name)) {
    if remove_support_worker(db, events, &worker.name).await? {
      settled.removed.push(worker.name.clone());
    }
  }

  Ok(settled)
}

/// A support worker taken away with its sessions, and `worker-removed` published so the checkouts it was
/// lent go too; whether there was one.
pub async fn remove_support_worker(
  db: &DatabaseConnection,
  events: &Bus,
  name: &str,
) -> Result<bool, Refusal> {
  let unit = db.begin().await?;
  let gone = run(
    &unit,
    r#"DELETE FROM "agents" WHERE "name" = $1 AND "kind" = 'worker' AND "support_of" IS NOT NULL"#,
    vec![name.into()],
  )
  .await?
    > 0;

  if gone {
    run(&unit, r#"DELETE FROM "worker_sessions" WHERE "worker" = $1"#, vec![name.into()]).await?;
  }

  unit.commit().await?;

  if gone {
    events.publish(WorkerRemoved {
      worker: name.to_owned(),
    });
  }

  Ok(gone)
}

/// A base's checkout on a node, a support worker of its own placed there and numbered by no slot
/// (docs/work-nodes.md, 「A checkout on a node」).
pub struct NodeCheckout<'a> {
  pub name: &'a str,
  pub label: &'a str,
  pub cwd: &'a str,
  pub base: &'a str,
  pub node: &'a str,
}

pub async fn add_node_checkout(
  db: &DatabaseConnection,
  checkout: &NodeCheckout<'_>,
) -> Result<(), Refusal> {
  run(
    db,
    r#"INSERT INTO "agents" ("name", "kind", "role", "label", "cwd", "icon", "builtin", "last_run_at", "support_of", "node")
       VALUES ($1, 'worker', 'worker', $2, $3, $4, 0, $5, $6, $7)"#,
    vec![
      checkout.name.into(),
      checkout.label.into(),
      checkout.cwd.into(),
      WORKER_ICON.into(),
      (now_ms() / 1000).into(),
      checkout.base.into(),
      checkout.node.into(),
    ],
  )
  .await?;

  Ok(())
}

/// A worker the owner added set to rest, or woken; whether there was such a worker. The built-in and a
/// support worker never rest, and a worker already resting keeps the moment it began.
pub async fn set_resting(db: &DatabaseConnection, name: &str, resting: bool) -> Result<bool, Refusal> {
  let (sql, values): (&str, Vec<Value>) = if resting {
    (
      r#"UPDATE "agents" SET "resting_since" = COALESCE("resting_since", $1)
         WHERE "name" = $2 AND "kind" = 'worker' AND "builtin" = 0 AND "support_of" IS NULL"#,
      vec![(now_ms() / 1000).into(), name.into()],
    )
  } else {
    (
      r#"UPDATE "agents" SET "resting_since" = NULL
         WHERE "name" = $1 AND "kind" = 'worker' AND "builtin" = 0 AND "support_of" IS NULL"#,
      vec![name.into()],
    )
  };

  Ok(run(db, sql, values).await? > 0)
}

/// One resting worker: what it is called and since when it rests.
#[derive(Debug, PartialEq, Eq)]
pub struct Resting {
  pub name: String,
  pub label: Option<String>,
  pub since: i64,
}

/// The resting workers, by name.
pub async fn resting_workers(db: &DatabaseConnection) -> Result<Vec<Resting>, Refusal> {
  let rows = all(
    db,
    r#"SELECT "name", "label", "resting_since" FROM "agents"
       WHERE "kind" = 'worker' AND "resting_since" IS NOT NULL ORDER BY "name""#,
    vec![],
  )
  .await?;

  Ok(
    rows
      .iter()
      .map(|row| {
        Ok(Resting {
          name: row.try_get("", "name")?,
          label: row.try_get("", "label")?,
          since: row.try_get("", "resting_since")?,
        })
      })
      .collect::<Result<_, sea_orm::DbErr>>()?,
  )
}

/// One worker's keepalive turned off until its next job wakes it; whether there was such a worker.
pub async fn pause_keepalive(db: &DatabaseConnection, name: &str) -> Result<bool, Refusal> {
  Ok(
    run(
      db,
      r#"UPDATE "agents" SET "keepalive_paused_at" = $1 WHERE "name" = $2 AND "kind" = 'worker'"#,
      vec![(now_ms() / 1000).into(), name.into()],
    )
    .await?
      > 0,
  )
}
