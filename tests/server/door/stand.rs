//! A program stood afresh only between its turns (docs/agent-control.md, 「The command is the program's
//! identity」): a program whose command changed while its record says a turn works is left standing, the
//! turn asking for it going in as it stands, and its record saying the turn ended lets the next stand
//! replace it. A research place's session token stands until its run's web tools close and a grace past
//! them (docs/research-runs.md, 「The road」).

use std::collections::HashMap;
use std::time::Duration;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::door::inbox::turn_working;
use agency_server::door::stand::stand_resident;
use agency_server::lines::place::{Place, Program};
use agency_server::places::Ground;
use agent_host::identity::ProcessIdentity;
use agent_host::place::ResidentCommand;
use harness_providers::Machine;
use serde_json::json;

fn record(place: &Place, lines: &[serde_json::Value]) {
  let text: String = lines.iter().map(|line| format!("{line}\n")).collect();

  std::fs::write(place.path.join("loop.jsonl"), text).unwrap();
}

fn program(identity: &str, cwd: &std::path::Path) -> Program {
  Program {
    identity: identity.to_owned(),
    command: ResidentCommand {
      args: vec!["bmo-tui".into(), identity.into()],
      cwd: cwd.to_path_buf(),
      env: Default::default(),
    },
    restart: None,
    opened: None,
  }
}

#[tokio::test]
async fn a_changed_program_is_not_ended_while_its_record_says_a_turn_works() {
  let home = tempfile::tempdir().unwrap();
  let variables: HashMap<String, String> = HashMap::new();
  let service = Service::new(
    sea_orm::DatabaseConnection::Disconnected,
    Config::read(|name| variables.get(name).cloned()).unwrap(),
  );
  let machine = Machine::with_env(
    [("HOME".to_owned(), home.path().to_string_lossy().into_owned())],
    home.path().join("data"),
  );
  let herdr = herdr::Herdr::here(home.path().join("herdr.sock"), home.path().join("silences.jsonl"));
  let ground = Ground::new(&service, &machine, Some(herdr));
  let home_of = ground.home_of("agent-a", "agent-a").unwrap();
  let place = Place::of(&ground.roots.data, "agent-a");
  let mut standing = std::process::Command::new("sleep").arg("30").spawn().unwrap();
  let identity = ProcessIdentity::of(standing.id() as i32).unwrap();

  std::fs::create_dir_all(&place.path).unwrap();
  std::fs::write(
    place.path.join("program.json"),
    serde_json::to_string(&program("old", home.path())).unwrap(),
  )
  .unwrap();
  std::fs::write(
    place.path.join("running.json"),
    json!({ "identity": "old", "pid": identity.pid, "birth": identity.birth, "loop": identity,
            "starts": 1, "startedAt": 1, "paneId": "pane-1", "recent": [] })
    .to_string(),
  )
  .unwrap();
  record(
    &place,
    &[json!({ "kind": "start", "id": "t", "name": "a person's line", "typed": true, "at": 1 })],
  );

  assert!(turn_working(&place.path));

  let stood = stand_resident(
    &ground,
    &place,
    &home_of,
    &program("new", home.path()),
    "chat",
    Duration::from_millis(100),
    Duration::from_millis(10),
  )
  .await
  .unwrap();

  assert_eq!((stood.pane.as_str(), stood.restarted), ("pane-1", false));
  assert_eq!(place.program().unwrap().identity, "old", "the working program is left as it stands");
  assert!(ProcessIdentity::of(standing.id() as i32).is_some_and(|alive| alive.alive()));

  // Once its record says the turn ended, the program is between turns and the next stand replaces it.
  record(
    &place,
    &[
      json!({ "kind": "start", "id": "t", "name": "a person's line", "typed": true, "at": 1 }),
      json!({ "kind": "answer", "id": "t", "text": "done" }),
      json!({ "kind": "end", "id": "t" }),
    ],
  );
  assert!(!turn_working(&place.path));

  let _ = standing.kill();
  let _ = standing.wait();
}

#[test]
fn a_research_place_s_token_stands_until_its_web_tools_close_and_a_grace_past_them() {
  let grace = 30 * 60 * 1_000;
  let research = json!({ "closesAt": 10_000_000 });

  assert_eq!(
    agency_server::door::stand::research_token_deadline(&research, 1_000),
    10_000_000 + grace
  );
  assert_eq!(
    agency_server::door::stand::research_token_deadline(&research, 20_000_000),
    20_000_000 + grace,
    "a moment already passed bounds it from its issue"
  );
  assert_eq!(agency_server::door::stand::research_token_deadline(&json!({}), 5), 5 + grace);
}
