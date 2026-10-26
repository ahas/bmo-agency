//! A research run cut under its pane or by agency's own restart (docs/research-runs.md, 「One run, step by
//! step」): closed in the ledger as cut, the room handed what it had written or one line that the work was
//! cut off, and a server's start sweeping what the process before it left under way.

use std::sync::{Arc, Mutex};

use agency_server::places::Ground;
use agency_server::research::{
  CUT, IN_FLIGHT_KEY, InFlight, KeptRun, Placed, RUN_PLACE, Swept, cut_instruction, sweep_cut,
  written_so_far,
};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{StatusCode, Uri};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::scratch::Scratch;

// A session's record whose last answer is `text`.
fn record_with(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
  let path = dir.join("session.jsonl");
  let lines = [
    json!({ "type": "user", "message": { "role": "user", "content": "the task" } }),
    json!({
      "type": "assistant",
      "message": { "id": "m1", "role": "assistant", "model": "a-model", "content": [{ "type": "text", "text": text }] }
    }),
  ];

  std::fs::write(&path, lines.iter().map(|line| format!("{line}\n")).collect::<String>()).unwrap();
  path
}

#[test]
fn a_cut_run_hands_back_what_it_wrote_or_says_in_one_line_that_the_work_was_cut_off() {
  let dir = tempfile::tempdir().unwrap();
  let record = record_with(dir.path(), "ANSWERED: no :: half way\nSUMMARY:\n1. a finding");

  assert_eq!(
    written_so_far(&record).as_deref(),
    Some("ANSWERED: no :: half way\nSUMMARY:\n1. a finding")
  );
  assert_eq!(written_so_far(&dir.path().join("none.jsonl")), None);

  let handed = cut_instruction(7, "search:basic", Some("the rate"), Some(5), Some("a finding"));
  let order = handed["said"]["order"].as_str().unwrap();

  assert!(order.contains("search:basic") && order.contains("\"the rate\"") && order.contains("a finding"));
  assert!(order.contains("unfinished"));
  assert_eq!(handed["room"], 5);
  assert_eq!(handed["kind"], "research-ended:7");

  let nothing = cut_instruction(8, "browse", None, None, None);
  let said = nothing["said"]["order"].as_str().unwrap();

  assert!(said.contains("in one line that the work was cut off"), "{said}");
  assert_eq!(nothing["room"], Value::Null);
}

type Calls = Arc<Mutex<Vec<(String, Value)>>>;

// A ground over the scratch store whose receiver is the stand-in at `address`, its data under `home`.
fn ground(scratch: &Scratch, home: &std::path::Path, address: &str) -> Arc<Ground> {
  let variables: std::collections::HashMap<String, String> = std::collections::HashMap::new();
  let service = agency_server::Service::new(
    scratch.db().clone(),
    agency_server::config::Config::read(|name| variables.get(name).cloned()).unwrap(),
  );
  let machine = harness_providers::Machine::with_env(
    [
      ("HOME".to_owned(), home.to_string_lossy().into_owned()),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
      ("BMO_RECEIVER_URL".to_owned(), address.to_owned()),
      ("BMO_RECEIVER_TOKEN".to_owned(), "receiver-token".to_owned()),
      ("BMO_MCP_HOST_SOCKET".to_owned(), home.join("host.sock").to_string_lossy().into_owned()),
      ("BMO_MCP_HOST_ISSUER_TOKEN".to_owned(), "issuer".to_owned()),
    ],
    home.join("data"),
  );

  Arc::new(Ground::new(&service, &machine, None))
}

// The tool host's stand-in on its socket under `home`, keeping every session it is asked to take back.
fn tool_host(home: &std::path::Path) -> Arc<Mutex<Vec<String>>> {
  let revoked: Arc<Mutex<Vec<String>>> = Arc::default();
  let listener = tokio::net::UnixListener::bind(home.join("host.sock")).unwrap();
  let kept = revoked.clone();
  let app = axum::Router::new().route(
    "/tool-sessions/{token}",
    axum::routing::delete(move |axum::extract::Path(token): axum::extract::Path<String>| {
      let kept = kept.clone();

      async move {
        kept.lock().unwrap().push(token);
        StatusCode::NO_CONTENT
      }
    }),
  );

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  revoked
}

// A place's program record, carrying the session token it was issued.
fn program_with(place: &std::path::Path, token: &str) {
  let program = agency_server::lines::place::Program {
    identity: "a-program".into(),
    command: agent_host::place::ResidentCommand {
      args: vec!["bmo-tui".into()],
      cwd: place.to_path_buf(),
      env: [("BMO_MCP_TOKEN".to_owned(), token.to_owned())].into(),
    },
    restart: None,
    opened: None,
  };

  std::fs::write(place.join("program.json"), serde_json::to_vec(&program).unwrap()).unwrap();
}

// The receiver's stand-in, answering every call it is made and keeping it.
async fn receiver() -> (String, Calls) {
  let calls: Calls = Arc::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(stand_in).with_state(calls.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  (address, calls)
}

async fn stand_in(State(calls): State<Calls>, uri: Uri, body: Bytes) -> impl IntoResponse {
  calls
    .lock()
    .unwrap()
    .push((uri.path().to_owned(), serde_json::from_slice(&body).unwrap_or(Value::Null)));

  (StatusCode::CREATED, axum::Json(json!({ "id": 1 })))
}

#[tokio::test]
async fn a_server_s_start_closes_the_runs_the_last_process_left_under_way_as_cut_and_tells_their_rooms() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let dir = tempfile::tempdir().unwrap();
  let record = record_with(dir.path(), "what it had found");
  let (address, calls) = receiver().await;
  let ground = ground(&scratch, dir.path(), &address);

  // A run kept under way with its record, one the ledger alone holds open, one already closed, and one
  // started after this process did.
  scratch
    .run(
      r#"INSERT INTO "runs" ("id", "agent", "chat_id", "unit", "depth", "started_at", "finished_at", "outcome")
         VALUES (1, 'search', 5, 'research-1', 0, 100, NULL, NULL),
                (2, 'search', 6, 'research-2', 1, 100, NULL, NULL),
                (3, 'search', 6, 'research-3', 0, 100, 150, 'answered'),
                (4, 'browse', 6, 'research-4', 0, 900, NULL, NULL)"#,
      vec![],
    )
    .await;

  let kept = InFlight {
    key: "search:basic".into(),
    query: "the rate".into(),
    room: Some(5),
    model: "a-model".into(),
    record: Some(record.to_string_lossy().into_owned()),
    placed: None,
  };

  agency_server::queue::settings::set(
    db,
    &format!("{IN_FLIGHT_KEY}1"),
    &serde_json::to_string(&kept).unwrap(),
  )
  .await
  .unwrap();

  assert_eq!(
    sweep_cut(&ground, 500).await.unwrap(),
    Swept {
      cut: 2,
      taken_up: 0
    }
  );

  assert_eq!(scratch_outcome(db, 1).await, Some(CUT.to_owned()));
  assert_eq!(scratch_outcome(db, 2).await, Some(CUT.to_owned()));
  assert_eq!(scratch_outcome(db, 3).await, Some("answered".to_owned()));
  assert_eq!(scratch_outcome(db, 4).await, None, "a run of this process is not swept");
  assert!(
    agency_server::queue::settings::get(db, &format!("{IN_FLIGHT_KEY}1"))
      .await
      .unwrap()
      .is_none(),
    "the kept run is let go"
  );

  let told = calls.lock().unwrap().clone();

  assert_eq!(told.len(), 2, "{told:?}");
  assert!(told.iter().all(|(path, _)| path == "/instructions"));

  let first = told[0].1["said"]["order"].as_str().unwrap();
  let second = told[1].1["said"]["order"].as_str().unwrap();

  assert!(first.contains("what it had found") && first.contains("\"the rate\""), "{first}");
  assert!(second.contains("search:thorough") && second.contains("cut off"), "{second}");
  assert_eq!(told[1].1["room"], 6);

  // A second start finds nothing left.
  assert_eq!(sweep_cut(&ground, 500).await.unwrap(), Swept::default());
}

// A run in places kept as standing on its final round, in the place named, with the first answer it had.
fn placed(ledger: i64, place: &str, round: usize) -> InFlight {
  InFlight {
    key: "search:basic".into(),
    query: "the rate".into(),
    room: Some(5),
    model: "a-model".into(),
    record: None,
    placed: Some(Placed {
      ask: bmo_agency_client::ResearchRunAsk {
        key: "search:basic".into(),
        query: "the rate".into(),
        asker: None,
        options: None,
        room_id: Some(5),
      },
      taken: agency_server::road::Road {
        provider: "deepseek".into(),
        model: "a-model".into(),
        account: None,
      },
      unit: format!("research-{ledger}"),
      prompt: "the task".into(),
      closes_at: 1_000,
      bound_ms: 60_000,
      run: KeptRun {
        model: "a-model".into(),
        effort: String::new(),
        tier: "standard".into(),
        system_prompt: "system.md".into(),
        mcp_config: "mcp.json".into(),
        thinking: false,
        cwd: "agents/search".into(),
      },
      place: place.to_owned(),
      before: 0,
      round: Some(round),
      rounds_began: Some(100),
      answers: vec!["the first answer".into()],
      spent: None,
    }),
  }
}

// A research run's place standing as its own, its program's record holding one turn under the run's name that
// answered `text`.
fn place_with(data: &std::path::Path, name: &str, ledger: i64, text: &str) -> std::path::PathBuf {
  let path = agency_server::lines::place::Place::of(data, name).path;
  let lines = [
    json!({ "kind": "start", "id": "t", "name": format!("research-{ledger}"), "title": "a turn", "at": 1 }),
    json!({ "kind": "answer", "id": "t", "text": text, "model": "[\"deepseek\",\"a-model\"]",
            "usage": { "inputTokens": 10, "outputTokens": 5, "cacheReadTokens": 2 } }),
    json!({ "kind": "end", "id": "t" }),
  ];

  std::fs::create_dir_all(&path).unwrap();
  std::fs::write(path.join(RUN_PLACE), json!({ "place": name, "run": ledger }).to_string()).unwrap();
  std::fs::write(
    path.join("loop.jsonl"),
    lines.iter().map(|line| format!("{line}\n")).collect::<String>(),
  )
  .unwrap();
  path
}

#[tokio::test]
async fn a_run_whose_place_still_stands_is_taken_up_and_finished_and_one_whose_place_is_gone_is_cut() {
  let scratch = Scratch::new().await;
  let db = scratch.db();
  let home = tempfile::tempdir().unwrap();
  let (address, calls) = receiver().await;
  let ground = ground(&scratch, home.path(), &address);
  let revoked = tool_host(home.path());
  let rounds = agency_server::research_draft::process_of(db, "search:basic")
    .await
    .unwrap()
    .unwrap()
    .rounds;
  let last = rounds.len() - 1;

  scratch
    .run(
      r#"INSERT INTO "runs" ("id", "agent", "chat_id", "unit", "depth", "started_at", "finished_at", "outcome")
         VALUES (9, 'search', 5, 'research-9', 0, 100, NULL, NULL),
                (10, 'search', 5, 'research-10', 0, 100, NULL, NULL)"#,
      vec![],
    )
    .await;

  let rounds_place = place_with(&ground.roots.data, "search-9-rounds", 9, "the final answer");
  let first_place = place_with(&ground.roots.data, "search-9", 9, "the first answer");

  program_with(&rounds_place, "rounds-token");
  program_with(&first_place, "first-token");

  for (ledger, place) in [(9, "search-9-rounds"), (10, "search-10-rounds")] {
    agency_server::queue::settings::set(
      db,
      &format!("{IN_FLIGHT_KEY}{ledger}"),
      &serde_json::to_string(&placed(ledger, place, last)).unwrap(),
    )
    .await
    .unwrap();
  }

  assert_eq!(
    sweep_cut(&ground, 500).await.unwrap(),
    Swept {
      cut: 1,
      taken_up: 1
    }
  );
  assert_eq!(scratch_outcome(db, 10).await, Some(CUT.to_owned()), "its place is gone");

  // The run taken up reads its final round's answer out of its place's record, closes as answered with what
  // the turn spent, tells the room the result and ends both its places.
  let mut waited = 0;

  while scratch_outcome(db, 9).await.is_none() && waited < 200 {
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    waited += 1;
  }

  assert_eq!(scratch_outcome(db, 9).await, Some("answered".to_owned()));

  let spent = store::one(db, r#"SELECT "input_tokens" FROM "runs" WHERE "id" = 9"#, vec![])
    .await
    .unwrap()
    .and_then(|row| row.try_get::<Option<i64>>("", "input_tokens").ok().flatten());

  assert_eq!(spent, Some(10));

  let mut told = vec![];

  for _ in 0..200 {
    told = calls.lock().unwrap().clone();

    if told.len() == 2 && !rounds_place.exists() && !first_place.exists() {
      break;
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
  }

  let result = told
    .iter()
    .find(|(_, body)| body["kind"] == "research-ended:9")
    .map(|(_, body)| body["said"]["order"].as_str().unwrap_or_default().to_owned())
    .unwrap_or_default();

  assert!(result.contains("the final answer") && !result.contains("cut off"), "{told:?}");
  assert!(!rounds_place.exists() && !first_place.exists(), "both places are ended with the run");

  let mut taken_back = revoked.lock().unwrap().clone();

  taken_back.sort();
  assert_eq!(
    taken_back,
    ["first-token", "rounds-token"],
    "their session tokens are taken back with them"
  );
  assert!(
    agency_server::queue::settings::get(db, &format!("{IN_FLIGHT_KEY}9"))
      .await
      .unwrap()
      .is_none(),
    "the run under way is let go"
  );
}

async fn scratch_outcome(db: &sea_orm::DatabaseConnection, id: i64) -> Option<String> {
  store::one(db, r#"SELECT "outcome" FROM "runs" WHERE "id" = $1"#, vec![id.into()])
    .await
    .unwrap()
    .and_then(|row| row.try_get::<Option<String>>("", "outcome").ok().flatten())
}
