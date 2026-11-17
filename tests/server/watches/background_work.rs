//! A supervisor's background work after the turn that launched it (docs/agent-control.md, 「Background work on a
//! line that carries it apart」): a task its notice ended is told once to the room it was launched in, as an order
//! for its supervisor opening a turn there; a task whose program is gone, or that a stop ended, is told once as
//! lost; a task with no room, and one still running under its program, is told nowhere.

use agency_server::door::place_run::{BackgroundLine, Launch, background_file};
use agency_server::door::record::task_notice;
use agency_server::lines::place::place_of;
use agency_server::watches::background_work::{Watch, kept_file, pass};
use agent_host::identity::ProcessIdentity;
use serde_json::{Value, json};

use crate::support::peers::stand;
use crate::support::scratch::Scratch;

const AGENT: &str = "bmo";

// A place whose program is this test's own process, started at `started_at`, and whose background record holds
// `lines`.
fn place_with(data: &std::path::Path, started_at: i64, lines: &[BackgroundLine]) -> std::path::PathBuf {
  let place = place_of(data, AGENT);
  let identity = ProcessIdentity::own();

  std::fs::create_dir_all(&place).unwrap();
  std::fs::write(
    place.join("running.json"),
    json!({ "identity": "one", "pid": identity.pid, "birth": identity.birth, "loop": identity,
            "starts": 1, "startedAt": started_at, "recent": [] })
    .to_string(),
  )
  .unwrap();
  std::fs::write(
    background_file(&place),
    lines
      .iter()
      .map(|line| format!("{}\n", serde_json::to_string(line).unwrap()))
      .collect::<String>(),
  )
  .unwrap();

  place
}

fn launched(task: &str, kind: &str, room: Option<&str>, at: i64) -> BackgroundLine {
  BackgroundLine::Launched(Launch {
    at,
    task: task.to_owned(),
    kind: kind.to_owned(),
    description: format!("the work of {task}"),
    room: room.map(str::to_owned),
    agent: room.map(|_| AGENT.to_owned()),
    run: room.map(|room| format!("chat:{room}:1")),
    ..Launch::default()
  })
}

fn noticed(task: &str, at: i64) -> BackgroundLine {
  let notice = task_notice(&format!(
    "<task-notification>\n<task-id>{task}</task-id>\n<output-file>/tmp/tasks/{task}.output</output-file>\n\
     <status>completed</status>\n<result>Six sets made.</result>\n</task-notification>"
  ))
  .unwrap();

  BackgroundLine::Notice { at, notice }
}

fn order(body: &Value) -> &str {
  body["said"]["order"].as_str().unwrap_or_default()
}

/// a task its notice ended is told once to the room it was launched in, as an order for the supervisor that
/// launched it, opening a turn there and carrying what it handed back
#[tokio::test]
async fn a_finished_notice_is_told_once_to_the_room_its_work_was_launched_in() {
  let scratch = Scratch::new().await;
  let (peers, vars) = stand().await;
  let data = tempfile::tempdir().unwrap();
  let place = place_with(
    data.path(),
    1_000,
    &[launched("a60", "subagent", Some("-500"), 2_000), noticed("a60", 3_000)],
  );
  let mut watch = Watch::default();

  pass(scratch.db(), &vars, data.path(), &mut watch).await;
  pass(scratch.db(), &vars, data.path(), &mut watch).await;

  let told = peers.lock().unwrap().instructed();

  assert_eq!(told.len(), 1, "told once: {told:?}");
  assert_eq!(told[0]["room"], -500);
  assert_eq!(told[0]["agentName"], AGENT);
  assert_eq!(told[0]["sort"], "background");
  assert_eq!(told[0]["kind"], "background-ended:a60");
  assert_eq!(told[0]["cause"], "reply");
  assert!(
    order(&told[0]).contains("has ended (completed): the subagent \"the work of a60\""),
    "{}",
    order(&told[0])
  );
  assert!(order(&told[0]).contains("Six sets made.") && order(&told[0]).contains("/tmp/tasks/a60.output"));
  assert!(std::fs::read_to_string(kept_file(&place)).unwrap().contains("a60"));
}

/// a task whose program was stood afresh after it was launched is told once as lost, one a stop ended is told as
/// stopped, and a task with no room, or one still running under the program that launched it, is told nowhere
#[tokio::test]
async fn work_lost_or_stopped_is_told_once_and_work_with_no_room_or_still_running_nowhere() {
  let scratch = Scratch::new().await;
  let (peers, vars) = stand().await;
  let data = tempfile::tempdir().unwrap();

  place_with(
    data.path(),
    5_000,
    &[
      launched("a61", "subagent", Some("-500"), 2_000),
      launched("b62", "shell", Some("-600"), 2_000),
      BackgroundLine::Stopped {
        at: 2_500,
        task: "b62".to_owned(),
      },
      launched("b63", "shell", None, 2_000),
      noticed("b63", 3_000),
      launched("b64", "shell", Some("-500"), 6_000),
    ],
  );

  let mut watch = Watch::default();

  pass(scratch.db(), &vars, data.path(), &mut watch).await;
  pass(scratch.db(), &vars, data.path(), &mut watch).await;

  let told = peers.lock().unwrap().instructed();
  let kinds: Vec<&str> = told.iter().map(|body| body["kind"].as_str().unwrap()).collect();

  assert_eq!(kinds, ["background-lost:a61", "background-lost:b62"], "{told:?}");
  assert!(
    order(&told[0]).contains("Your program was ended or stood afresh"),
    "{}",
    order(&told[0])
  );
  assert!(order(&told[1]).contains("A stop of your turn ended it."), "{}", order(&told[1]));
  assert_eq!(told[1]["room"], -600);
}
