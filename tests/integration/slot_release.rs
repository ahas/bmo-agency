//! A slot given back under a running turn (docs/role-slots.md, 「The line」): the owner's release gives back the
//! lease at once, puts the stop to the turn running in the role session the lease was bound to, and ends that
//! session only once its turn has ended — never under it — the turn ending `stopped` with the reason
//! `released`.

use crate::integration::common;

use std::sync::Arc;
use std::time::Duration;

use agency_server::herdr_backend::{HerdrSessions, TurnEnded};
use agency_server::order_run::dispatch::{self, Asked, Wanted};
use agency_server::places::{Ground, Places};
use agency_server::queue::running::EXECUTION_CANCEL_PREFIX;
use agent_session::{
  Apart, FaultKind, Mapping, Resume, Scope, Sessions, Start, Turn, TurnContext, WorkParts,
};
use common::Scratch;
use common::herdr::HerdrModel;
use harness_providers::Machine;

use crate::support::grounds::{deepseek_balance, seeded};

// A role's turn's work parts, under the run name given.
fn work(cwd: &str, run: &str) -> WorkParts {
  WorkParts {
    provider: "claude-code".into(),
    model: "claude-sonnet-5".into(),
    mapping: Mapping {
      model: "claude-sonnet-5".into(),
      effort: None,
      tier: None,
      answer_tokens: None,
    },
    resume: Resume::Fresh,
    cwd: cwd.into(),
    name: run.into(),
    title: "a round".into(),
    chat: 0,
    unit: None,
    user: None,
    work_key: "worker".into(),
    hears_words: false,
    variables: Default::default(),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_owner_s_release_under_a_running_turn_cuts_it_and_ends_its_session_only_once_it_has_ended() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let line = seeded(scratch.db(), home.path()).await;

  deepseek_balance(&line, 5.0).await;

  // The slot, let in under its owner.
  let asked = Asked {
    request_key: "research:9".into(),
    owner: "research:9".into(),
    wanted: Wanted {
      role: "worker",
      path: None,
      fixed: None,
    },
    ..Asked::default()
  };
  let held = dispatch::enqueue(&line.db, &line.events, &asked, "search", async |_, _| Ok(()))
    .await
    .unwrap();
  let held = dispatch::acquire(&line, &held).await.unwrap().expect("let in");

  // The sessions, over a herdr of the test's own.
  let config = home.path().join("config");

  std::fs::create_dir_all(config.join("herdr")).unwrap();

  let herdr = HerdrModel::stand(config.join("herdr").join("herdr.sock"));
  let at = |path: &std::path::Path| path.to_string_lossy().into_owned();
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), at(home.path())),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
      ("XDG_CONFIG_HOME".to_owned(), at(&config)),
      ("BMO_DEPLOY_DIR".to_owned(), env!("CARGO_MANIFEST_DIR").to_owned()),
    ],
    home.path().join("data"),
  );
  let service = common::service(&scratch, &[("AGENCY_SOURCE_DIR", at(&home.path().join("source")))]);
  let mut ends = service.events.subscribe::<TurnEnded>();
  let ground = Ground::new(&service, &machine, Some(herdr::Herdr::there(herdr.socket.clone())));
  let sessions = Arc::new(HerdrSessions::new(Places::new(ground, Arc::default())));
  let started = sessions
    .start(Start {
      agent: "worker".into(),
      scope: Scope::Apart(Apart {
        provider: "claude-code".into(),
        model: "claude-sonnet-5".into(),
        cwd: at(home.path()),
        role: Some("worker".into()),
        todo: Some(1),
        ..Apart::default()
      }),
    })
    .await
    .unwrap();

  dispatch::bind_session(&line.db, &line.events, &held.token, &started.id.0)
    .await
    .unwrap();

  // A round's turn running in it.
  let turn = Turn {
    session: started.id.clone(),
    prompt: "work".into(),
    context: TurnContext {
      work: Some(work(&at(home.path()), "research-9-round")),
      ..TurnContext::default()
    },
  };
  let sending = sessions.clone();
  let going = tokio::spawn(async move { sending.send_turn(turn).await });

  for _ in 0..100 {
    if sessions.turn_run(&started.id).is_some() || going.is_finished() {
      break;
    }

    tokio::time::sleep(Duration::from_millis(20)).await;
  }

  if going.is_finished() {
    panic!("the turn ended before it could be cut: {:?}", going.await);
  }

  assert_eq!(
    sessions.turn_run(&started.id).as_deref(),
    Some("research-9-round"),
    "a tile names its run"
  );

  // The owner gives back what it holds: the lease at once, as the owner's, and the session bound to it named.
  let done = dispatch::release_owner(&line.db, &line.events, "research:9").await.unwrap();
  let given = dispatch::request(&line.db, &held.token).await.unwrap().unwrap();

  assert_eq!(done.sessions, vec![started.id.0.clone()]);
  assert_eq!(
    (given.state.as_str(), given.ended_by.as_deref()),
    ("released", Some(dispatch::BY_OWNER))
  );

  // The session is ended under its turn only once the turn has ended: the stop is put to the turn first.
  let cutting = sessions.clone();
  let session = started.id.clone();
  let cut = tokio::spawn(async move { cutting.end_under_turns(&session, "released").await });

  for _ in 0..100 {
    if going.is_finished() {
      break;
    }

    tokio::time::sleep(Duration::from_millis(50)).await;
  }

  if going.is_finished() {
    let end = going.await.unwrap().unwrap();

    assert_eq!(
      (end.ending.as_deref(), end.reason.as_deref()),
      (Some("stopped"), Some("released")),
      "{end:?}"
    );
    cut.await.unwrap().unwrap();
    assert_eq!(ends.ready().map(|ended| ended.end.reason), Some(Some("released".to_owned())));
    assert_eq!(sessions.read_state(&started.id, None).await.unwrap_err().kind, FaultKind::Unknown);
  } else {
    // A turn not yet at its program reads the stop as it gets there; until then its session stands.
    let marker =
      agency_server::queue::settings::get(&line.db, &format!("{EXECUTION_CANCEL_PREFIX}research-9-round"))
        .await
        .unwrap();

    assert!(marker.is_some(), "the stop is put to the running turn");
    assert!(!cut.is_finished(), "the session is not ended under its turn");
    assert!(
      sessions.read_state(&started.id, None).await.is_ok(),
      "the session stands while its turn runs"
    );
  }
}
