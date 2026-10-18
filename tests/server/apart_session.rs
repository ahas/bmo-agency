//! A thrown-away session over the session contract (docs/api.md, 「Sessions」): started on Claude Code alone
//! under a number of its own, its state its scope's provider, model and folder at the first generation,
//! named in the list until it ends, refusing a fold and a turn that names no work parts, and gone once
//! ended; its turn placed in the standing branch checkout its folder is, under the co-work task's label, or
//! in a pane of its own; and the folder a branch's place is taken in, as the take answers it.

use std::collections::HashMap;
use std::sync::Arc;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::herdr_backend::{HerdrSessions, TurnEnded, apart_id, is_apart};
use agency_server::job_run::claude::turn::{ApartSeat, apart_seat};
use agency_server::order_run::places::slot_cwd;
use agency_server::places::branch::{place_agent, pool_of, slot_dir};
use agency_server::places::{Ground, Places};
use agency_server::seeds::deployment;
use agent_session::{Apart, FaultKind, Scope, SessionId, Sessions, Start, Turn, TurnContext};
use harness_providers::Machine;

use crate::scratch::Scratch;

fn machine(home: &std::path::Path) -> Machine {
  let at = |path: std::path::PathBuf| path.to_string_lossy().into_owned();

  Machine::with_env(
    [
      ("HOME".to_owned(), at(home.to_path_buf())),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
    ],
    home.join("data"),
  )
}

fn ground(scratch: &Scratch, machine: &Machine, herdr: Option<herdr::Herdr>) -> Ground {
  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());

  Ground::new(&service, machine, herdr)
}

fn apart(provider: &str, cwd: &str) -> Apart {
  Apart {
    provider: provider.into(),
    model: "claude-sonnet-5".into(),
    cwd: cwd.into(),
    todo: Some(1816),
    role: Some("scout".into()),
  }
}

fn start(agent: &str, apart: Apart) -> Start {
  Start {
    agent: agent.into(),
    scope: Scope::Apart(apart),
  }
}

#[test]
fn a_thrown_away_sessions_id_names_its_agent_and_its_number() {
  assert_eq!(apart_id("branch-a", 3), SessionId("branch-a@apart:3".into()));
  assert!(is_apart(&apart_id("branch-a", 3)));
  assert!(!is_apart(&SessionId("branch-a@apart".into())));
  assert!(!is_apart(&SessionId("branch-a@worker".into())));
  assert!(!is_apart(&SessionId("branch-a@-7".into())));
}

#[tokio::test]
async fn a_thrown_away_session_stands_from_its_start_to_its_end_and_keeps_nothing_to_fold() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let machine = machine(home.path());

  store::seed::settle(scratch.db(), &deployment(&machine)).await.unwrap();

  let places = Places::new(ground(&scratch, &machine, None), Arc::default());
  let mut ended = places.ground.events.subscribe::<TurnEnded>();
  let sessions = HerdrSessions::new(places);
  let first = sessions.start(start("branch-a", apart("claude-code", "/tmp/run-a"))).await.unwrap();
  let second = sessions.start(start("branch-a", apart("claude-code", "/tmp/run-b"))).await.unwrap();

  assert_eq!(first.id, SessionId("branch-a@apart:1".into()));
  assert_eq!(second.id, SessionId("branch-a@apart:2".into()), "every start is a session of its own");
  assert_eq!(first.scope, Scope::Apart(apart("claude-code", "/tmp/run-a")));

  let listed = sessions.list().await.unwrap();

  assert!(listed.contains(&first) && listed.contains(&second), "{listed:?}");

  let state = sessions.read_state(&first.id, None).await.unwrap();

  assert_eq!(
    (state.provider.as_str(), state.model.as_str(), state.place.as_str(), state.generation),
    ("claude-code", "claude-sonnet-5", "/tmp/run-a", 1)
  );
  assert_eq!((state.context, state.run, state.went_out), (None, None, None));
  assert_eq!(sessions.fold(&first.id).await.unwrap_err().kind, FaultKind::Refused);

  let bare = Turn {
    session: first.id.clone(),
    prompt: "look around".into(),
    context: TurnContext::default(),
  };

  assert_eq!(sessions.send_turn(bare).await.unwrap_err().kind, FaultKind::Refused);
  assert!(ended.ready().is_none(), "a turn refused before it starts publishes no end");

  sessions.end(&first.id).await.unwrap();

  assert_eq!(sessions.read_state(&first.id, None).await.unwrap_err().kind, FaultKind::Unknown);
  assert_eq!(sessions.end(&first.id).await.unwrap_err().kind, FaultKind::Unknown);
  assert!(!sessions.list().await.unwrap().contains(&first));
  assert!(sessions.list().await.unwrap().contains(&second));
  assert_eq!(
    sessions.read_state(&SessionId("branch-a@apart:9".into()), None).await.unwrap_err().kind,
    FaultKind::Unknown,
    "a number never given names no session"
  );
  assert_eq!(
    sessions.start(start("branch-a", apart("codex", "/tmp/run-c"))).await.unwrap_err().kind,
    FaultKind::Refused,
    "a thrown-away session goes out on Claude Code alone"
  );
  assert_eq!(
    sessions.start(start("a@b", apart("claude-code", "/tmp/run-c"))).await.unwrap_err().kind,
    FaultKind::Refused
  );
}

#[tokio::test]
async fn a_thrown_away_turn_goes_into_the_standing_checkout_its_folder_is_or_a_pane_of_its_own() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let machine = machine(home.path());
  let herdr = herdr::Herdr::there(home.path().join("herdr.sock"));
  let with_herdr = ground(&scratch, &machine, Some(herdr));
  let without = ground(&scratch, &machine, None);
  let place = slot_dir(&with_herdr.roots.data, "agency-0123456789", 3);
  let inside = place.join("crates").to_string_lossy().into_owned();

  match apart_seat(&with_herdr, &apart("claude-code", &inside)) {
    Some(ApartSeat::Standing(home)) => {
      assert_eq!(home.agent, place_agent("agency-0123456789", 3));
      assert!(home.label.contains("#1816 scout 03"), "{}", home.label);
      assert!(home.resting.is_some(), "the place rests on its own label once the run ends");
    }
    other => panic!("a standing checkout's folder is its place: {other:?}"),
  }

  assert!(matches!(
    apart_seat(&with_herdr, &apart("claude-code", "/tmp/research-7")),
    Some(ApartSeat::Own(_))
  ));
  assert!(apart_seat(&without, &apart("claude-code", &inside)).is_none());
  assert!(apart_seat(&without, &apart("claude-code", "/tmp/research-7")).is_none());
}

#[tokio::test]
async fn a_branchs_place_is_taken_in_the_folder_of_its_number_under_its_checkouts_pool() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let machine = machine(home.path());
  let ground = ground(&scratch, &machine, None);
  let checkout = home.path().join("checkout").to_string_lossy().into_owned();

  assert_eq!(
    slot_cwd(&ground, &checkout, 2).await,
    slot_dir(&ground.roots.data, &pool_of(&checkout).await, 2)
  );
}
