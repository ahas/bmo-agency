//! What a job's end leaves standing in herdr, swept (docs/agent-control.md, 「One standing pane per
//! agent」), on agency's server. It reads the whole of herdr — this machine's, and each work node's whose
//! link the node service reports up — because what outlives a job is as often a workspace no standing
//! agent owns any more as a shell beside a head.
//!
//! A pane carrying a run, a head, a node's pane or a dev server is kept, whatever else is true of it.
//! Only in a workspace this system marked — an agent's or a node's token on it — is a pane carrying
//! nothing closed, and a gone agent's workspace left with nothing kept closed whole. What the sweep
//! will not close is published as `herdr-leftovers` rather than closed, since an unmarked workspace may
//! be a person's own (docs/api.md, 「Events」).

use std::collections::HashSet;

use herdr::Herdr;
use policy::fill_marks;
use policy::queue_words::QUEUE_WORDS;
use serde::Serialize;
use serde_json::{Value, json};
use store::all;

use super::settings;
use crate::Refusal;
use crate::events::Event;
use crate::places::Ground;
use crate::places::branch::place_agent;
use crate::places::claims::PaneClaims;
use crate::places::home::{AGENT_TOKEN, DEV_VERB, HEAD_MARK, STANDING_MARK};

// The token a node's workspace is reported under, and the mark its pane's loop runs under.
const NODE_TOKEN: &str = "node";
const NODE_MARK: &str = "bmo-node-";

// Where the survivors last told were written, so the same picture is told once.
const TOLD_KEY: &str = "herdr_leftovers_told";

/// One pane as the sweep reads it: its run, or what herdr started it with.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneView {
  pub pane: String,
  pub run: Option<String>,
  pub command: Vec<String>,
}

/// One workspace as the sweep reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceView {
  pub workspace: String,
  pub label: String,
  pub agent: Option<String>,
  pub node: Option<String>,
  pub panes: Vec<PaneView>,
}

/// A workspace the sweep would not close, told rather than closed.
#[derive(Clone, Debug, PartialEq)]
pub struct Survivor {
  pub workspace: String,
  pub label: String,
  pub panes: usize,
  pub marked: bool,
}

/// What to close and what to tell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
  pub close_panes: Vec<String>,
  pub close_workspaces: Vec<String>,
  pub survivors: Vec<Survivor>,
}

/// Whether a pane runs a standing process, told by its command alone.
pub fn carries_standing(command: &[String]) -> bool {
  command.iter().any(|part| {
    part.starts_with(HEAD_MARK)
      || part == DEV_VERB
      || part.starts_with(NODE_MARK)
      || part.starts_with(STANDING_MARK)
  })
}

/// What to close and what to tell, out of the workspaces as they stand, the agents that stand and the
/// workers resting.
pub fn plan(workspaces: &[WorkspaceView], standing: &HashSet<String>, resting: &HashSet<String>) -> Plan {
  let mut plan = Plan::default();

  for workspace in workspaces {
    let kept: Vec<&PaneView> = workspace
      .panes
      .iter()
      .filter(|pane| pane.run.is_some() || !pane.command.is_empty())
      .collect();
    let empty: Vec<&PaneView> = workspace
      .panes
      .iter()
      .filter(|pane| pane.run.is_none() && pane.command.is_empty())
      .collect();
    let marked = workspace.agent.is_some() || workspace.node.is_some();

    if !marked {
      // Nothing of this system's is known to be here: one carrying nothing may be a person's own shell.
      if kept.is_empty() && !empty.is_empty() {
        plan.survivors.push(Survivor {
          workspace: workspace.workspace.clone(),
          label: workspace.label.clone(),
          panes: empty.len(),
          marked: false,
        });
      }

      continue;
    }

    // A resting worker's workspace goes whole, its head and its program with it, once no run is in it.
    if workspace.agent.as_ref().is_some_and(|agent| resting.contains(agent)) {
      if workspace.panes.iter().all(|pane| pane.run.is_none()) {
        plan.close_workspaces.push(workspace.workspace.clone());
      }

      continue;
    }

    // A standing agent's own workspace, and a node's, are the standing passes' to tidy: one with no
    // head yet is about to be given one.
    let standing_home =
      workspace.agent.as_ref().is_some_and(|agent| standing.contains(agent)) || workspace.node.is_some();

    if standing_home {
      if kept.iter().any(|pane| carries_standing(&pane.command)) {
        plan.close_panes.extend(empty.iter().map(|pane| pane.pane.clone()));
      }

      continue;
    }

    if kept.is_empty() {
      plan.close_workspaces.push(workspace.workspace.clone());
      continue;
    }

    plan.close_panes.extend(empty.iter().map(|pane| pane.pane.clone()));

    // A run in it is still at work and says nothing about leftovers.
    if kept.iter().all(|pane| pane.run.is_none()) {
      plan.survivors.push(Survivor {
        workspace: workspace.workspace.clone(),
        label: workspace.label.clone(),
        panes: kept.len(),
        marked: true,
      });
    }
  }

  plan
}

/// The workspaces as a herdr holds them now, each pane with its command; none where herdr did not
/// answer.
pub async fn read_workspaces(herdr: &Herdr) -> Option<Vec<WorkspaceView>> {
  let snapshot = herdr.snapshot().await.ok()?;
  let mut views = vec![];

  for workspace in &snapshot.workspaces {
    let Some(id) = &workspace.workspace_id else { continue };
    let mut panes = vec![];

    for pane in snapshot.panes.iter().filter(|pane| pane.workspace_id.as_ref() == Some(id)) {
      let Some(pane_id) = &pane.pane_id else { continue };
      let run = pane.run().map(str::to_owned);
      // A pane carrying a run is kept on that alone, so its command is not asked for.
      let command = if run.is_some() { vec![] } else { herdr.command_of(pane_id).await };

      panes.push(PaneView {
        pane: pane_id.clone(),
        run,
        command,
      });
    }

    views.push(WorkspaceView {
      workspace: id.clone(),
      label: workspace.label.clone().unwrap_or_default(),
      agent: workspace.token(AGENT_TOKEN).map(str::to_owned),
      node: workspace.token(NODE_TOKEN).map(str::to_owned),
      panes,
    });
  }

  Some(views)
}

/// The agents that stand: the store's own, awake, and each place of the co-work pools.
pub async fn standing_agents(db: &sea_orm::DatabaseConnection) -> Result<HashSet<String>, Refusal> {
  let mut names: HashSet<String> =
    all(db, r#"SELECT "name" FROM "agents" WHERE "resting_since" IS NULL"#, vec![])
      .await?
      .iter()
      .map(|row| row.try_get::<String>("", "name"))
      .collect::<Result<_, _>>()?;

  for row in all(db, r#"SELECT DISTINCT "pool", "slot" FROM "branch_agents""#, vec![]).await? {
    let pool: String = row.try_get("", "pool")?;
    let slot: i64 = row.try_get("", "slot")?;

    names.insert(place_agent(&pool, slot));
  }

  Ok(names)
}

/// The workers resting.
pub async fn resting_workers(db: &sea_orm::DatabaseConnection) -> Result<HashSet<String>, Refusal> {
  Ok(
    all(
      db,
      r#"SELECT "name" FROM "agents" WHERE "kind" = 'worker' AND "resting_since" IS NOT NULL"#,
      vec![],
    )
    .await?
    .iter()
    .map(|row| row.try_get::<String>("", "name"))
    .collect::<Result<_, _>>()?,
  )
}

/// One workspace the sweep would not close, as the leftovers event names it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftWorkspace {
  pub workspace_id: String,
  pub label: String,
  pub panes: usize,
  pub marked: bool,
}

/// What a job's end left open in herdr that the sweep would not close (docs/api.md, 「Events」): how many
/// panes, in which workspaces, and one English sentence of it. Telling the owner is the consumer's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HerdrLeftovers {
  pub panes: usize,
  pub workspaces: Vec<LeftWorkspace>,
  pub account: String,
}

impl Event for HerdrLeftovers {
  const NAME: &'static str = "herdr-leftovers";
}

impl HerdrLeftovers {
  /// The event of the survivors a plan left.
  pub fn of(survivors: &[Survivor]) -> Self {
    let words = &QUEUE_WORDS.leftovers;
    let panes: usize = survivors.iter().map(|one| one.panes).sum();
    let named: Vec<String> = survivors
      .iter()
      .map(|one| {
        let label = if one.label.is_empty() { &one.workspace } else { &one.label };
        let workspace =
          fill_marks(&words.workspace, &[("label", label), ("panes", &one.panes.to_string())]);

        if one.marked {
          workspace
        } else {
          fill_marks(&words.unmarked, &[("workspace", &workspace)])
        }
      })
      .collect();

    Self {
      panes,
      workspaces: survivors
        .iter()
        .map(|one| LeftWorkspace {
          workspace_id: one.workspace.clone(),
          label: one.label.clone(),
          panes: one.panes,
          marked: one.marked,
        })
        .collect(),
      account: fill_marks(&words.account, &[("panes", &panes.to_string()), ("named", &named.join(", "))]),
    }
  }
}

/// One sweep of one herdr: what it may close, closed, and what it would not close published once for each
/// picture; none where herdr could not be asked, which closes and publishes nothing.
pub async fn sweep(
  ground: &Ground,
  herdr: &Herdr,
  standing: &HashSet<String>,
  resting: &HashSet<String>,
) -> Result<Option<Plan>, Refusal> {
  let db = &ground.db;
  let Some(workspaces) = read_workspaces(herdr).await else {
    return Ok(None);
  };
  let plan = plan(&workspaces, standing, resting);

  for pane in &plan.close_panes {
    let _ = herdr.call("pane.close", json!({ "pane_id": pane }), None).await;
  }

  for workspace in &plan.close_workspaces {
    let _ = herdr.call("workspace.close", json!({ "workspace_id": workspace }), None).await;
  }

  let told =
    Value::Array(plan.survivors.iter().map(|one| json!([one.workspace, one.panes])).collect()).to_string();

  if plan.survivors.is_empty() {
    settings::forget(db, TOLD_KEY).await?;
  } else if settings::get(db, TOLD_KEY).await?.as_deref() != Some(told.as_str()) {
    settings::set(db, TOLD_KEY, &told).await?;
    ground.events.publish(HerdrLeftovers::of(&plan.survivors));
  }

  Ok(Some(plan))
}

/// What a job's end leaves in herdr swept, once the end has been answered: this machine's herdr, and each
/// ready work node's whose link the node service reports up, on its own socket; then the pinned workers'
/// programs stood again, waiting a while for the checkout's lock the ended runner still holds.
pub async fn after_job_end(ground: &Ground) {
  if let Err(refusal) = sweep_everywhere(ground).await {
    eprintln!("agency: leftover sweep: {}", refusal.detail);
  }

  crate::places::standby::restand_all(ground, Some(crate::places::standby::AFTER_JOB_LOCK_WAIT)).await;
}

/// What a job's end leaves in herdr swept on this machine's herdr, where one stands, and on each ready work
/// node's whose link the node service reports up. Every agent standing counts, a pane a consumer claims, a
/// gate place and a research run's own place among them (docs/api.md, 「Hooks」, the pane claims;
/// docs/agent-control.md, 「A gate's places」; docs/research-runs.md, 「The road」).
pub async fn sweep_everywhere(ground: &Ground) -> Result<(), Refusal> {
  let db = &ground.db;
  let mut standing = standing_agents(db).await?;
  let resting = resting_workers(db).await?;

  standing.extend(ground.hooks.ask::<PaneClaims>(()).await.into_iter().map(|claim| claim.agent));
  standing.extend(crate::places::gate_places::recorded(&ground.roots.data).into_keys());
  standing.extend(crate::research::run_places(&ground.roots.data));

  if let Some(herdr) = &ground.herdr {
    sweep(ground, herdr, &standing, &resting).await?;
  }

  for row in all(
    db,
    r#"SELECT "node" FROM "node_workplaces" WHERE "state" = 'ready' ORDER BY "node""#,
    vec![],
  )
  .await?
  {
    let node: String = row.try_get("", "node")?;

    // A node service that cannot be asked reads the link as down, as a link not heard from lately is.
    if ground.peers.node.link_up(&node).await.unwrap_or(false) {
      let herdr = Herdr::there(ground.roots.data.join("nodes").join(&node).join("herdr.sock"));

      sweep(ground, &herdr, &standing, &resting).await?;
    }
  }

  Ok(())
}
