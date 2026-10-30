//! A thrown-away session over the session contract (docs/api.md, 「Sessions」): started on Claude Code alone
//! under a number of its own, its state its scope's provider, model and folder at the first generation,
//! named in the list until it ends, refusing a fold and a turn that names no work parts, and gone once
//! ended; its turn placed in the standing branch checkout its folder is, under the co-work task's label, or
//! in a pane of its own; the folder a branch's place is taken in, as the take answers it; and a session
//! naming its prompt document, which goes out on that runner line whole in a pane of its own.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use agency_server::Service;
use agency_server::config::Config;
use agency_server::control::session_ops::scope_asked;
use agency_server::herdr_backend::{HerdrSessions, TurnEnded, apart_id, is_apart};
use agency_server::job_run::claude::turn::{ApartSeat, apart_seat, document_command};
use agency_server::lines::Trees;
use agency_server::lines::runner::{branch_instructions, search_instructions};
use agency_server::lines::token::research_record;
use agency_server::order_run::places::slot_cwd;
use agency_server::places::branch::{place_agent, pool_of, slot_dir};
use agency_server::places::{Ground, Places};
use agency_server::research_draft::process_of;
use agency_server::seeds::deployment;
use agent_session::{
  Apart, FaultKind, GateAsk, Mapping, Resume, Scope, SessionId, Sessions, Start, Turn, TurnContext,
  WorkParts,
};
use harness_providers::Machine;
use serde_json::json;

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
    ..Apart::default()
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

  // A turn carrying a gate's ask goes out on the gate road even in a session apart: its refusal is the
  // gate's own, that a gate's turn names its system prompt, not the apart turn's that names no work.
  let gate = Turn {
    session: first.id.clone(),
    prompt: "classify".into(),
    context: TurnContext {
      gate: Some(GateAsk::default()),
      ..TurnContext::default()
    },
  };
  let refused = sessions.send_turn(gate).await.unwrap_err();

  assert_eq!(refused.kind, FaultKind::Refused);
  assert!(
    refused.message.contains("a gate's turn names its system prompt"),
    "an apart turn carrying a gate's ask takes the gate road: {}",
    refused.message
  );

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

// The site tools' catalogue the stand-in tool host lists for the web namespace.
const LISTED: &str = "- naver_blog — A blog post. · Naver Blog (blog.naver.com)";

// A tool host standing on `socket` that lists one web tool, with that catalogue line, to the issuer `token`.
fn web_host(socket: &std::path::Path, token: &'static str) {
  let listener = tokio::net::UnixListener::bind(socket).unwrap();
  let app = axum::Router::new().route(
    "/namespaces/web/tools",
    axum::routing::get(move |headers: axum::http::HeaderMap| async move {
      let bearer = headers.get("authorization").and_then(|value| value.to_str().ok());

      if bearer != Some(format!("Bearer {token}").as_str()) {
        return (axum::http::StatusCode::UNAUTHORIZED, axum::Json(json!({ "code": "unauthorized" })));
      }

      (
        axum::http::StatusCode::OK,
        axum::Json(json!({
          "items": [{
            "namespace": "web", "name": "naver_blog", "title": null, "description": "A blog post.",
            "input": { "type": "object" }, "output": null, "icon": null, "capabilities": [], "grant": null,
            "hints": { "readOnly": true, "destructive": false, "idempotent": null, "openWorld": true },
            "listing": { "line": LISTED, "order": 1 }, "system": false, "manual": null, "reachedBy": "call",
          }],
          "more": false,
        })),
      )
    }),
  );

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
}

// A machine whose tool host is the socket named, its code read from this checkout.
fn hosted(home: &std::path::Path, socket: &std::path::Path) -> Machine {
  let at = |path: &std::path::Path| path.to_string_lossy().into_owned();

  Machine::with_env(
    [
      ("HOME".to_owned(), at(home)),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
      ("BMO_DEPLOY_DIR".to_owned(), env!("CARGO_MANIFEST_DIR").to_owned()),
      ("BMO_MCP_HOST_SOCKET".to_owned(), at(socket)),
      ("BMO_MCP_HOST_ISSUER_TOKEN".to_owned(), "issuer".to_owned()),
    ],
    home.join("data"),
  )
}

// A turn's work parts on the work key a research tool sends, its variables the run's record variables.
fn work(variables: &[(&str, &str)]) -> WorkParts {
  WorkParts {
    provider: "claude-code".into(),
    model: "claude-sonnet-5".into(),
    mapping: Mapping {
      model: "claude-sonnet-5".into(),
      effort: Some("medium".into()),
      tier: Some("standard".into()),
      answer_tokens: None,
    },
    resume: Resume::Fresh,
    cwd: String::new(),
    name: "research-7-branch-1".into(),
    title: "a branch".into(),
    chat: 0,
    unit: None,
    user: None,
    work_key: "search".into(),
    hears_words: false,
    variables: variables
      .iter()
      .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
      .collect(),
  }
}

fn on_document(document: Option<&str>, cwd: &str) -> Apart {
  Apart {
    provider: "claude-code".into(),
    model: "claude-sonnet-5".into(),
    cwd: cwd.into(),
    prompt_document: document.map(str::to_owned),
    ..Apart::default()
  }
}

#[tokio::test]
async fn a_session_naming_the_branch_document_goes_out_on_the_branch_line_under_the_run_s_record() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let socket = home.path().join("mcp-host.sock");

  web_host(&socket, "issuer");

  let machine = hosted(home.path(), &socket);
  let ground = ground(&scratch, &machine, None);
  let trees = Trees::of(&ground);
  let apart = on_document(Some("search-branch"), "");
  let variables = work(&[
    ("BMO_RESEARCH_RUN", "7"),
    ("BMO_RESEARCH_BRANCH_OF", "7"),
    ("BMO_RESEARCH_BRANCHES", "2"),
    ("BMO_RESEARCH_BRANCH_PAGES", "4"),
    ("BMO_RESEARCH_SESSIONS_AT", "http://127.0.0.1:9"),
  ]);
  let (run, command) = document_command(&ground, &apart, "search-branch", &variables, None, "a-session")
    .await
    .unwrap()
    .expect("the branch line is composed where the web listing can be had");

  assert_eq!(
    (run.line, run.model.as_str(), run.effort.as_str()),
    ("search", "claude-sonnet-5", "medium")
  );
  assert!(run.system_prompt.ends_with("search-branch.md"), "{}", run.system_prompt.display());

  let system = std::fs::read_to_string(&run.system_prompt).unwrap();

  assert!(system.contains(&branch_instructions(&trees, LISTED).unwrap()), "{system}");
  assert!(!system.contains(&search_instructions(&trees, LISTED).unwrap()), "a branch is no search");

  let config = std::fs::read_to_string(&run.mcp_config).unwrap();

  assert!(config.contains("/mcp/web"), "{config}");
  assert_eq!(
    command.cwd,
    trees.agents.join("search").to_string_lossy(),
    "no folder named is the line's own"
  );
  assert!(
    command.args.windows(2).any(|pair| pair == ["--session-id", "a-session"]),
    "{:?}",
    command.args
  );

  let env: BTreeMap<String, String> = command
    .env
    .iter()
    .filter_map(|(name, value)| Some((name.clone(), value.clone()?)))
    .collect();
  let (record, withheld) = research_record(&env);
  let record = record.expect("the record carries the run's research variables");

  assert_eq!(
    (&record["run"], &record["branchOf"], &record["pages"]),
    (&json!(7), &json!(7), &json!(4))
  );
  assert_eq!(record["branches"], 0, "a branch opens no branch");
  assert!(record.get("sessionsAt").is_none(), "a branch names no session service");
  assert!(withheld.contains(&"web.branches"));

  // The work key a research tool sends has no draft process, so no round follows the branch's answer.
  assert!(process_of(scratch.db(), "search").await.unwrap().is_none());
}

#[tokio::test]
async fn a_session_naming_no_runner_s_document_is_refused_with_the_names_that_are() {
  let refused = scope_asked(&json!({
    "agentName": "explorer", "scope": "apart", "model": "claude-sonnet-5", "promptDocumentName": "worker",
  }))
  .unwrap_err();
  let said = format!("{refused:?}");

  for name in ["search", "search-branch", "browse"] {
    assert!(said.contains(name), "{said}");
  }

  let read = scope_asked(&json!({
    "agentName": "explorer", "scope": "apart", "model": "claude-sonnet-5",
    "promptDocumentName": "search-branch", "boundMs": 600_000,
  }))
  .unwrap();

  assert_eq!(
    read,
    Scope::Apart(Apart {
      bound_ms: Some(600_000),
      ..on_document(Some("search-branch"), "")
    }),
    "a session on a runner line naming no folder leaves it to the line"
  );
  assert_eq!(
    scope_asked(&json!({ "agentName": "a", "scope": "apart", "model": "claude-sonnet-5" })).unwrap(),
    Scope::Apart(on_document(None, ".")),
    "a session naming no document keeps the worker line's folder"
  );

  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let socket = home.path().join("nobody.sock");
  let ground = ground(&scratch, &hosted(home.path(), &socket), None);

  assert!(
    document_command(&ground, &on_document(Some("worker"), ""), "worker", &work(&[]), None, "s")
      .await
      .is_err()
  );
}

#[tokio::test]
async fn a_session_naming_its_document_takes_a_pane_of_its_own_and_its_line_s_folder() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let machine = machine(home.path());
  let herdr = herdr::Herdr::there(home.path().join("herdr.sock"));
  let with_herdr = ground(&scratch, &machine, Some(herdr));
  let place = slot_dir(&with_herdr.roots.data, "agency-0123456789", 3);
  let inside = place.join("crates").to_string_lossy().into_owned();

  assert!(matches!(
    apart_seat(&with_herdr, &on_document(Some("search-branch"), &inside)),
    Some(ApartSeat::Own(_))
  ));
  assert!(
    matches!(apart_seat(&with_herdr, &apart("claude-code", &inside)), Some(ApartSeat::Standing(_))),
    "a session naming none still goes into the standing checkout its folder is"
  );

  store::seed::settle(scratch.db(), &deployment(&machine)).await.unwrap();

  let places = Places::new(ground(&scratch, &machine, None), Arc::default());
  let agents = Trees::of(&places.ground).agents;
  let sessions = HerdrSessions::new(places);
  let started = sessions.start(start("explorer", on_document(Some("browse"), ""))).await.unwrap();

  assert_eq!(
    started.scope,
    Scope::Apart(on_document(Some("browse"), &agents.join("browse").to_string_lossy()))
  );
}
