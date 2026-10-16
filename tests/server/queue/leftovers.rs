//! What a job's end leaves in herdr: only a marked workspace's empty panes close, a gone agent's workspace
//! with nothing kept closes whole, and what the sweep will not close is published once in the policy's words.

use std::collections::HashSet;

use agency_server::queue::leftovers::{HerdrLeftovers, PaneView, Survivor, WorkspaceView, plan};
use policy::fill_marks;
use policy::queue_words::QUEUE_WORDS;

fn pane(id: &str, run: Option<&str>, command: &[&str]) -> PaneView {
  PaneView {
    pane: id.into(),
    run: run.map(str::to_owned),
    command: command.iter().map(|part| (*part).to_owned()).collect(),
  }
}

fn workspace(id: &str, agent: Option<&str>, node: Option<&str>, panes: Vec<PaneView>) -> WorkspaceView {
  WorkspaceView {
    workspace: id.into(),
    label: format!("{id} label"),
    agent: agent.map(str::to_owned),
    node: node.map(str::to_owned),
    panes,
  }
}

#[test]
fn only_a_marked_workspaces_empty_panes_close_and_a_person_s_own_is_told() {
  let standing = HashSet::from(["worker".to_owned()]);
  let resting = HashSet::from(["sleeper".to_owned()]);
  let planned = plan(
    &[
      // A standing agent's home: its shell beside its head goes.
      workspace(
        "w1",
        Some("worker"),
        None,
        vec![pane("p1", None, &["bmo-head-worker"]), pane("p2", None, &[])],
      ),
      // A gone agent's workspace with nothing kept goes whole.
      workspace("w2", Some("gone"), None, vec![pane("p3", None, &[])]),
      // A gone agent's workspace with a program left is told, its empty pane closed.
      workspace("w3", Some("gone"), None, vec![pane("p4", None, &["vim"]), pane("p5", None, &[])]),
      // A resting worker's goes whole once no run is in it.
      workspace("w4", Some("sleeper"), None, vec![pane("p6", None, &["bmo-head-sleeper"])]),
      // An unmarked one carrying nothing may be a person's: told, never closed.
      workspace("w5", None, None, vec![pane("p7", None, &[])]),
      // A run anywhere keeps its pane.
      workspace("w6", Some("gone"), None, vec![pane("p8", Some("run-1"), &[])]),
    ],
    &standing,
    &resting,
  );

  assert_eq!(planned.close_panes, ["p2", "p5"]);
  assert_eq!(planned.close_workspaces, ["w2", "w4"]);
  assert_eq!(
    planned
      .survivors
      .iter()
      .map(|one| (one.workspace.as_str(), one.panes, one.marked))
      .collect::<Vec<_>>(),
    [("w3", 1, true), ("w5", 1, false)]
  );
}

#[test]
fn what_the_sweep_would_not_close_is_said_in_the_policys_words() {
  let survivors = [
    Survivor {
      workspace: "w3".into(),
      label: "notes".into(),
      panes: 1,
      marked: true,
    },
    Survivor {
      workspace: "w5".into(),
      label: String::new(),
      panes: 2,
      marked: false,
    },
  ];
  let told = HerdrLeftovers::of(&survivors);
  let words = &QUEUE_WORDS.leftovers;
  let first = fill_marks(&words.workspace, &[("label", "notes"), ("panes", "1")]);
  let second = fill_marks(
    &words.unmarked,
    &[("workspace", &fill_marks(&words.workspace, &[("label", "w5"), ("panes", "2")]))],
  );

  assert_eq!(told.panes, 3);
  assert_eq!(told.workspaces.len(), 2);
  assert_eq!(
    told.account,
    fill_marks(&words.account, &[("panes", "3"), ("named", &format!("{first}, {second}"))])
  );
}
