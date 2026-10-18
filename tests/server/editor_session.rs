//! An editor work key's session over the session contract (docs/api.md, 「Sessions」): started for an editor's
//! key alone, its state the model its key maps to in the brain folder, keeping nothing to fold or let go, named
//! in no list, and a turn that cannot go out ending as a failure that says why; and the key's walk falling to
//! Claude Code where it stands aside, and failing the turn where it had to answer and met no stand-in — on a
//! store of the test's own, with the seeds agency settles.

use std::collections::HashMap;
use std::sync::Arc;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::harness::{OrderEntry, set_provider_order};
use agency_server::editor_session::{Placed, is_editor, walked, workdir_of};
use agency_server::herdr_backend::{HerdrSessions, TurnEnded};
use agency_server::places::{Ground, Places};
use agency_server::run_log::{RunLog, Target};
use agency_server::seeds::deployment;
use agent_session::{
  Delivery, FaultKind, Outcome, RunParts, Scope, SessionId, Sessions, Start, Turn, TurnContext,
};
use harness_providers::Machine;

use crate::scratch::Scratch;

/// A store settled with agency's seeds, a home and a brain folder of the test's own, and the backend on them.
struct Bench {
  _scratch: Scratch,
  home: tempfile::TempDir,
  ground: Ground,
  sessions: HerdrSessions,
  ended: agency_server::events::Subscription<TurnEnded>,
}

impl Bench {
  async fn new() -> Self {
    let scratch = Scratch::new().await;
    let home = tempfile::tempdir().unwrap();
    let at = |path: std::path::PathBuf| path.to_string_lossy().into_owned();
    let machine = Machine::with_env(
      [
        ("HOME".to_owned(), at(home.path().to_path_buf())),
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
      ],
      home.path().join("data"),
    );

    store::seed::settle(scratch.db(), &deployment(&machine)).await.unwrap();

    let variables: HashMap<String, String> =
      HashMap::from([("AGENCY_BRAIN_ROOT".to_owned(), at(home.path().join("brain")))]);
    let service =
      Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());
    let ground = Ground::new(&service, &machine, None);
    let places = Places::new(Ground::new(&service, &machine, None), Arc::default());
    let ended = places.ground.events.subscribe::<TurnEnded>();

    Self {
      _scratch: scratch,
      home,
      ground,
      sessions: HerdrSessions::new(places),
      ended,
    }
  }

  fn ground(&self) -> &Ground {
    &self.ground
  }

  fn brain(&self) -> std::path::PathBuf {
    self.home.path().join("brain")
  }

  // The brain folder made a repository, as far as an editor run asks.
  fn brain_is_a_repository(&self) {
    std::fs::create_dir_all(self.brain().join(".git")).unwrap();
  }
}

fn editor(key: &str) -> Start {
  Start {
    agent: key.to_owned(),
    scope: Scope::Editor,
  }
}

#[test]
fn only_an_editors_work_key_is_one() {
  assert!(is_editor("digest"));
  assert!(is_editor("people-digest"));
  assert!(!is_editor("worker"));
  assert!(!is_editor("nobody"));
}

#[tokio::test]
async fn an_editor_session_is_started_for_an_editors_key_and_refused_for_any_other() {
  let bench = Bench::new().await;
  let session = bench.sessions.start(editor("digest")).await.unwrap();

  assert_eq!(session.id, SessionId("digest@editor".into()));
  assert_eq!(session.scope, Scope::Editor);
  assert_eq!(bench.sessions.start(editor("worker")).await.unwrap_err().kind, FaultKind::Refused);
}

#[tokio::test]
async fn an_editor_session_names_its_model_in_the_brain_and_keeps_nothing() {
  let bench = Bench::new().await;
  let session = SessionId("digest@editor".into());
  let refused = bench.sessions.read_state(&session, None).await.unwrap_err();

  assert_eq!(refused.kind, FaultKind::Refused);
  assert!(refused.message.contains("no repository"), "{}", refused.message);

  bench.brain_is_a_repository();

  let state = bench.sessions.read_state(&session, None).await.unwrap();

  assert_eq!(state.provider, "claude-code");
  assert!(!state.model.is_empty());
  assert_eq!(state.place, bench.brain().to_string_lossy());
  assert_eq!(bench.sessions.fold(&session).await.unwrap_err().kind, FaultKind::Refused);
  bench.sessions.end(&session).await.unwrap();
  assert!(
    bench.sessions.list().await.unwrap().iter().all(|listed| listed.scope != Scope::Editor),
    "an editor's session is never listed"
  );
}

#[tokio::test]
async fn a_turn_whose_brain_is_no_repository_ends_failed_saying_why() {
  let mut bench = Bench::new().await;
  let end = bench
    .sessions
    .send_turn(Turn {
      session: SessionId("people-digest@editor".into()),
      prompt: "sort the people".into(),
      context: TurnContext::default(),
    })
    .await
    .unwrap();
  let ended = bench.ended.ready().expect("the turn's end was published");

  assert_eq!(end, ended.end, "the send answers the end it published");
  assert_eq!(end.delivery, Delivery::Taken);
  assert_eq!((ended.session.as_str(), end.outcome), ("people-digest@editor", Outcome::Failed));
  assert!(end.reason.unwrap().contains("no repository"));
}

// The bench's store with its provider order set: each provider named, on or off.
async fn order(bench: &Bench, order: &[(&str, bool)]) {
  let entries: Vec<OrderEntry> = order
    .iter()
    .map(|(name, enabled)| OrderEntry {
      name: (*name).to_owned(),
      enabled: *enabled,
    })
    .collect();

  set_provider_order(bench._scratch.db(), &entries).await.unwrap();
}

// The digest's run placed in the bench's brain folder, which it makes.
async fn placed(bench: &Bench) -> Placed {
  std::fs::create_dir_all(bench.brain()).unwrap();

  Placed::bare(bench.ground(), &bench.brain(), "digest", "turn:digest:1").await.unwrap()
}

fn log(key: &str) -> RunLog {
  RunLog::new(Target {
    agent: key.to_owned(),
    run: format!("turn:{key}:1"),
    chat_id: None,
    job_id: None,
  })
}

#[tokio::test]
async fn an_editor_key_whose_walk_stands_aside_falls_to_claude_code() {
  let bench = Bench::new().await;

  order(
    &bench,
    &[("claude-code", true), ("codex", true), ("openrouter", true), ("registered", true)],
  )
  .await;

  let placed = placed(&bench).await;
  let walked = walked(bench.ground(), "digest", "sort", &placed, &mut log("digest")).await;

  assert!(matches!(walked, Ok(None)), "{walked:?}");
}

#[tokio::test]
async fn a_walk_that_must_answer_and_meets_no_stand_in_fails_the_editors_turn() {
  let mut bench = Bench::new().await;

  bench.brain_is_a_repository();
  order(
    &bench,
    &[("claude-code", false), ("codex", true), ("openrouter", true), ("registered", true)],
  )
  .await;

  let placed = placed(&bench).await;
  let walked = walked(bench.ground(), "digest", "sort", &placed, &mut log("digest")).await;

  assert!(walked.as_ref().is_err_and(|why| why.contains("did not answer")), "{walked:?}");

  let end = bench
    .sessions
    .send_turn(Turn {
      session: SessionId("digest@editor".into()),
      prompt: "sort".into(),
      context: TurnContext::default(),
    })
    .await
    .unwrap();
  let ended = bench.ended.ready().expect("the turn's end was published");

  assert_eq!((end.delivery, ended.end.outcome), (Delivery::Taken, Outcome::Failed));
  assert!(ended.end.reason.unwrap().contains("did not answer"));
}

fn parts(run: &str, workdir: &str) -> RunParts {
  RunParts {
    name: run.to_owned(),
    parent: Some("batch-unit".to_owned()),
    agent: Some("companion".to_owned()),
    chat_id: Some(-7),
    step: Some(2),
    steps: Some(3),
    workdir: Some(workdir.to_owned()),
    desk: Some("desk-1".to_owned()),
    registration: None,
  }
}

#[test]
fn a_workdir_stays_inside_the_brain_folder() {
  assert_eq!(workdir_of(None).unwrap(), None);
  assert_eq!(
    workdir_of(Some(&parts("r", "agents/companion/"))).unwrap(),
    Some("agents/companion".into())
  );
  assert!(workdir_of(Some(&parts("r", "../elsewhere"))).is_err());
  assert!(workdir_of(Some(&parts("r", "/elsewhere"))).is_err());
}

#[tokio::test]
async fn a_run_is_placed_in_its_agents_folder_with_its_desk_room_and_name_and_the_editor_cap() {
  let bench = Bench::new().await;

  std::fs::create_dir_all(bench.brain().join("agents").join("companion")).unwrap();

  let named = parts("bmo-digest:digest:companion:-7:1", "agents/companion");
  let placed = Placed::of(bench.ground(), &bench.brain(), "digest", &named.name, Some(&named))
    .await
    .unwrap();
  let variable = |name: &str| {
    placed
      .variables
      .iter()
      .find(|(named, _)| named == name)
      .map(|(_, value)| value.as_str())
  };

  assert_eq!(placed.cwd, bench.brain().join("agents").join("companion"));
  assert_eq!(variable("BMO_FRAGMENT_DESK"), Some("desk-1"));
  assert_eq!(variable("BMO_TURN_CHAT"), Some("-7"));
  assert_eq!(variable("BMO_RUN_NAME"), Some(named.name.as_str()));
  assert_eq!(placed.parent.as_deref(), Some("batch-unit"));
  assert_eq!(placed.clocked.parent.as_deref(), Some("batch-unit"));
  assert!(placed.clocked.what.contains("companion") && placed.clocked.what.contains("batch 2 of 3"));
  assert_eq!(placed.clocked.bound.as_secs(), policy::run_clock::RUN_CLOCK.editor_cap_seconds);

  let missing = parts("r", "agents/missing");

  assert!(Placed::of(bench.ground(), &bench.brain(), "digest", "r", Some(&missing)).await.is_err());
}

#[tokio::test]
async fn an_editor_turn_goes_out_under_its_callers_run_name_and_a_workdir_leaving_the_brain_is_refused() {
  let mut bench = Bench::new().await;
  let turn = |parts: RunParts| Turn {
    session: SessionId("digest@editor".into()),
    prompt: "sort".into(),
    context: TurnContext {
      run: Some(parts),
      ..TurnContext::default()
    },
  };
  let refused = bench.sessions.send_turn(turn(parts("r", "../elsewhere"))).await.unwrap_err();

  assert_eq!(refused.kind, FaultKind::Refused);
  assert!(bench.ended.ready().is_none());

  bench.sessions.send_turn(turn(parts("named-run", "agents/companion"))).await.unwrap();

  let ended = bench.ended.ready().expect("the turn's end was published");

  assert_eq!((ended.end.run.as_str(), ended.end.outcome), ("named-run", Outcome::Failed));
}
